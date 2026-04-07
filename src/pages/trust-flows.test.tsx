import type { ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'

const { invoke, isTauri } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../app/shell-data-context'
import { I18nContext, type I18nContextValue } from '../lib/i18n/context'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from '../lib/i18n'
import { backend, backendTestHarness } from '../lib/backend'
import type {
  AppConfig,
  AppSnapshot,
  AuditRunDetail,
  DashboardSnapshot,
} from '../lib/types'
import { AuditPage } from './audit'
import { ImportPage } from './import'
import { SchedulePage } from './schedule'
import { SecurityPage } from './security'

const config: AppConfig = {
  initialized: false,
  archiveMode: 'Encrypted',
  preferredLanguage: 'system',
  dueAfterHours: 72,
  scheduleCheckIntervalHours: 6,
  checkpointDays: 90,
  captureFavicons: true,
  selectedProfileIds: ['chrome:Default'],
  gitEnabled: true,
  rememberDatabaseKeyInKeyring: false,
  appAutostart: false,
  remoteBackup: {
    enabled: false,
    bucket: '',
    region: 'us-east-1',
    endpoint: null,
    prefix: 'pathkeep',
    pathStyle: true,
    uploadAfterBackup: false,
    credentialsSaved: false,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    lastError: null,
  },
  ai: {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt:
      'You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

function createI18nValue(language: ResolvedLanguage): I18nContextValue {
  return {
    language,
    preference: language,
    setLanguagePreference: vi.fn(),
    t: createTranslator(language),
    ns: (namespace) => createNamespaceTranslator(language, namespace),
  }
}

function createShellValue(
  snapshot: AppSnapshot,
  dashboard: DashboardSnapshot | null = null,
): ShellDataContextValue {
  return {
    buildInfo: null,
    snapshot,
    dashboard,
    loading: false,
    busyAction: null,
    error: null,
    notice: null,
    refreshKey: 0,
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    saveConfig: vi.fn().mockResolvedValue(snapshot),
    initializeArchive: vi.fn().mockResolvedValue(snapshot),
    runBackup: vi.fn().mockResolvedValue({
      dueSkipped: false,
      run: null,
      profiles: [],
      warnings: [],
      remoteBackup: null,
    }),
    clearNotice: vi.fn(),
  }
}

function renderTrustPage(
  ui: ReactNode,
  {
    dashboard = null,
    language = 'en' as ResolvedLanguage,
    route = '/',
    snapshot,
  }: {
    dashboard?: DashboardSnapshot | null
    language?: ResolvedLanguage
    route?: string
    snapshot: AppSnapshot
  },
) {
  return render(
    <MemoryRouter initialEntries={[route]}>
      <I18nContext.Provider value={createI18nValue(language)}>
        <ShellDataContext.Provider
          value={createShellValue(snapshot, dashboard)}
        >
          {ui}
        </ShellDataContext.Provider>
      </I18nContext.Provider>
    </MemoryRouter>,
  )
}

async function seedInitializedSnapshot() {
  await backend.initializeArchive(config, 'vault-passphrase')
  const snapshot = await backend.getAppSnapshot()
  const dashboard = await backend.loadDashboardSnapshot()
  return { snapshot, dashboard }
}

describe('trust flows', () => {
  beforeEach(() => {
    isTauri.mockReturnValue(false)
    invoke.mockReset()
    backendTestHarness.reset()
  })

  test('covers import preview, execute, revert, and doctor review in a translated locale', async () => {
    const user = userEvent.setup()
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true)
    const { snapshot } = await seedInitializedSnapshot()

    renderTrustPage(<ImportPage />, {
      language: 'zh-CN',
      route: '/import',
      snapshot,
    })

    await user.type(
      screen.getByPlaceholderText('/path/to/takeout.zip'),
      '/tmp/takeout',
    )
    await user.click(screen.getByRole('button', { name: '扫描来源 →' }))

    expect(await screen.findByText('步骤 3：预览导入')).toBeVisible()
    expect(await screen.findByText('PathKeep trust UX notes')).toBeVisible()

    await user.click(screen.getByRole('button', { name: '确认导入 →' }))
    expect(await screen.findByText('步骤 5：导入完成')).toBeVisible()
    expect((await screen.findAllByText('已导入')).length).toBeGreaterThan(0)

    await waitFor(() =>
      expect(screen.getByRole('button', { name: '回滚批次' })).toBeEnabled(),
    )
    await user.click(screen.getByRole('button', { name: '回滚批次' }))
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '恢复批次' })).toBeEnabled(),
    )

    await user.click(screen.getByRole('button', { name: '运行检查' }))
    expect(
      await screen.findByRole('heading', { name: '需要关注' }),
    ).toBeVisible()

    confirmSpy.mockRestore()
  })

  test('renders Windows scheduler guidance and keeps PME tabs keyboard reachable', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const previewSpy = vi.spyOn(backend, 'previewSchedule').mockResolvedValue({
      platform: 'windows',
      label: 'dev.codex.pathkeep.backup',
      executablePath: 'C:/Program Files/PathKeep/pathkeep.exe',
      generatedFiles: [
        {
          relativePath: 'schedule/pathkeep-backup.xml',
          absolutePath:
            'C:/Users/test/AppData/Local/PathKeep/schedule/pathkeep-backup.xml',
          purpose: 'Task Scheduler XML',
          contents:
            '<Task><Settings><StartWhenAvailable>true</StartWhenAvailable></Settings></Task>',
        },
      ],
      manualSteps: ['Review the XML before import.'],
      applyCommands: [['schtasks', '/Create', '/XML', 'pathkeep-backup.xml']],
      rollbackCommands: [
        ['schtasks', '/Delete', '/TN', 'dev.codex.pathkeep.backup', '/F'],
      ],
      applySupported: false,
    })
    const statusSpy = vi.spyOn(backend, 'scheduleStatus').mockResolvedValue({
      platform: 'windows',
      label: 'dev.codex.pathkeep.backup',
      dueAfterHours: 48,
      checkIntervalHours: 6,
      applySupported: false,
      installState: 'manual-review',
      detectedFiles: [],
      manualSteps: ['Import the XML in Task Scheduler.'],
      auditPath: null,
      lastSuccessfulBackupAt: null,
      warnings: ['Review StartWhenAvailable before trusting the schedule.'],
    })

    renderTrustPage(<SchedulePage />, {
      language: 'zh-TW',
      route: '/schedule',
      snapshot,
    })

    expect(
      await screen.findByRole('heading', { name: 'Windows 工作排程器' }),
    ).toBeVisible()
    expect(await screen.findByText('每 48 小時一次')).toBeVisible()
    expect(await screen.findByText('每 6 小時檢查一次')).toBeVisible()

    await user.tab()
    expect(screen.getByRole('button', { name: '預覽' })).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: '手動' })).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: '執行' })).toHaveFocus()

    previewSpy.mockRestore()
    statusSpy.mockRestore()
  })

  test('renders rekey preview in Traditional Chinese without English mode fallbacks', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()

    renderTrustPage(<SecurityPage />, {
      language: 'zh-TW',
      route: '/security',
      snapshot,
    })

    expect(await screen.findByText('封存目前為 加密')).toBeVisible()

    await user.selectOptions(
      screen.getByLabelText('目標模式'),
      screen.getByRole('option', { name: '明文' }),
    )
    await user.click(screen.getByRole('button', { name: '預覽重新加密' }))

    expect(await screen.findByText('加密 → 明文')).toBeVisible()
  })

  test('filters audit runs and shows delta against the previous visible run', async () => {
    const user = userEvent.setup()
    const snapshot = await backend.getAppSnapshot()
    snapshot.config.initialized = true
    snapshot.recentRuns = [
      {
        id: 11,
        startedAt: '2026-04-07T10:00:00.000Z',
        finishedAt: '2026-04-07T10:05:00.000Z',
        status: 'success',
        runType: 'import',
        trigger: 'manual',
        profileScope: ['takeout::browser-history'],
        manifestHash: 'hash-11',
        profilesProcessed: 2,
        newVisits: 12,
        newUrls: 7,
        newDownloads: 3,
      },
      {
        id: 10,
        startedAt: '2026-04-06T10:00:00.000Z',
        finishedAt: '2026-04-06T10:05:00.000Z',
        status: 'success',
        runType: 'backup',
        trigger: 'schedule',
        profileScope: ['chrome:Default'],
        manifestHash: 'hash-10',
        profilesProcessed: 1,
        newVisits: 8,
        newUrls: 5,
        newDownloads: 2,
      },
      {
        id: 9,
        startedAt: '2026-04-05T10:00:00.000Z',
        finishedAt: '2026-04-05T10:05:00.000Z',
        status: 'success',
        runType: 'doctor',
        trigger: 'manual',
        profileScope: [],
        manifestHash: 'hash-9',
        profilesProcessed: 1,
        newVisits: 6,
        newUrls: 4,
        newDownloads: 1,
      },
    ]

    const detailMap: Record<number, AuditRunDetail> = {
      11: {
        run: snapshot.recentRuns[0],
        trigger: 'manual',
        timezone: 'America/Phoenix',
        dueOnly: false,
        profileScope: ['takeout::browser-history'],
        warnings: [],
        errorMessage: null,
        stats: {},
        manifestPath: '/tmp/run-11.json',
        manifestHash: 'hash-11',
        artifacts: [
          {
            kind: 'manifest',
            path: '/tmp/run-11.json',
            createdAt: '2026-04-07T10:05:00.000Z',
          },
        ],
      },
      10: {
        run: snapshot.recentRuns[1],
        trigger: 'schedule',
        timezone: 'America/Phoenix',
        dueOnly: false,
        profileScope: ['chrome:Default'],
        warnings: ['Schedule drift detected'],
        errorMessage: null,
        stats: {},
        manifestPath: '/tmp/run-10.json',
        manifestHash: 'hash-10',
        artifacts: [
          {
            kind: 'manifest',
            path: '/tmp/run-10.json',
            createdAt: '2026-04-06T10:05:00.000Z',
          },
          {
            kind: 'snapshot',
            path: '/tmp/run-10.snapshot',
            createdAt: '2026-04-06T10:05:00.000Z',
          },
        ],
      },
      9: {
        run: snapshot.recentRuns[2],
        trigger: 'manual',
        timezone: 'America/Phoenix',
        dueOnly: false,
        profileScope: [],
        warnings: [],
        errorMessage: null,
        stats: {},
        manifestPath: '/tmp/run-9.json',
        manifestHash: 'hash-9',
        artifacts: [
          {
            kind: 'snapshot',
            path: '/tmp/run-9.snapshot',
            createdAt: '2026-04-05T10:05:00.000Z',
          },
        ],
      },
    }

    const loadAuditRunDetailSpy = vi
      .spyOn(backend, 'loadAuditRunDetail')
      .mockImplementation((runId: number) => Promise.resolve(detailMap[runId]))

    renderTrustPage(<AuditPage />, {
      route: '/audit?run=11',
      snapshot,
    })

    expect(await screen.findByText('FILTERS')).toBeVisible()
    expect(
      await screen.findByRole('option', { name: 'snapshot' }),
    ).toBeVisible()
    await user.selectOptions(
      screen.getByLabelText('Source scope'),
      screen.getByRole('option', { name: 'Google Takeout' }),
    )
    expect(screen.getByRole('button', { name: /#11/ })).toBeVisible()
    expect(
      screen.queryByRole('button', { name: /#10/ }),
    ).not.toBeInTheDocument()
    await user.selectOptions(
      screen.getByLabelText('Source scope'),
      screen.getByRole('option', { name: 'All sources' }),
    )
    await user.selectOptions(
      screen.getByLabelText('Run type'),
      screen.getByRole('option', { name: 'Backup' }),
    )
    expect(screen.getByRole('button', { name: /#10/ })).toBeVisible()
    expect(
      screen.queryByRole('button', { name: /#11/ }),
    ).not.toBeInTheDocument()
    await user.selectOptions(
      screen.getByLabelText('Run type'),
      screen.getByRole('option', { name: 'All run types' }),
    )
    await user.selectOptions(
      screen.getByLabelText('Artifact type'),
      screen.getByRole('option', { name: 'snapshot' }),
    )

    await waitFor(() =>
      expect(
        screen.queryByRole('button', { name: /#11/ }),
      ).not.toBeInTheDocument(),
    )
    expect(screen.getByRole('button', { name: /#10/ })).toBeVisible()
    expect(await screen.findByText('Compared to run #9')).toBeVisible()
    expect(screen.getByText('+2')).toBeVisible()

    loadAuditRunDetailSpy.mockRestore()
  })
})
