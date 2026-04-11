import type { ReactNode } from 'react'
import { render, screen, waitFor, within } from '@testing-library/react'
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
import { backend } from '../lib/backend-client'
import { backendTestHarness } from '../lib/backend'
import { platformLabelKey } from '../lib/platform-guidance'
import { securityModeKey } from '../lib/trust-review'
import type {
  AppConfig,
  AppSnapshot,
  AuditRunDetail,
  DashboardSnapshot,
  ImportBatchDetail,
  ImportBatchOverview,
} from '../lib/types'
import { AuditPage } from './audit'
import { ImportPage } from './import'
import { SchedulePage } from './schedule'
import { SecurityPage } from './security'
import { SettingsPage } from './settings'

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
  appLock: {
    enabled: false,
    idleTimeoutMinutes: 5,
    biometricEnabled: false,
    passcodeEnabled: true,
    passcodeConfigured: false,
    recoveryHint: null,
  },
  analytics: {
    enabled: false,
    consentGrantedAt: null,
  },
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
  enrichment: {
    plugins: [
      {
        id: 'readable-content-refetch',
        enabled: true,
        version: 'm4-v1',
      },
    ],
  },
  deterministic: {
    modules: [
      { id: 'query-groups', enabled: true, version: 'm5b-v1' },
      { id: 'threads', enabled: true, version: 'm5b-v1' },
      { id: 'reference-pages', enabled: true, version: 'm5b-v1' },
      { id: 'source-effectiveness', enabled: true, version: 'm5b-v1' },
      { id: 'template-summaries', enabled: true, version: 'm5b-v1' },
    ],
  },
  ai: {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    enrichmentEnabled: true,
    enrichmentPlugins: [
      { pluginId: 'title-normalization', enabled: true },
      { pluginId: 'readable-content-refetch', enabled: true },
    ],
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
  const namespaceCache = new Map<string, ReturnType<typeof createTranslator>>()

  return {
    language,
    preference: language,
    setLanguagePreference: vi.fn(),
    t: createTranslator(language),
    ns: (namespace) => {
      const cached = namespaceCache.get(namespace)
      if (cached) {
        return cached
      }

      const translator = createNamespaceTranslator(language, namespace)
      namespaceCache.set(namespace, translator)
      return translator
    },
  }
}

function createShellValue(
  snapshot: AppSnapshot,
  dashboard: DashboardSnapshot | null = null,
): ShellDataContextValue {
  return {
    buildInfo: null,
    appLockStatus: snapshot.appLockStatus,
    snapshot,
    dashboard,
    loading: false,
    busyAction: null,
    busyOverlay: null,
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
    setAppLockPasscode: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    clearAppLockPasscode: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    lockAppSession: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    unlockAppSession: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    clearNotice: vi.fn(),
  }
}

function expectHtmlElement(node: Element | null): HTMLElement {
  expect(node).toBeInstanceOf(HTMLElement)
  return node as HTMLElement
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
    vi.restoreAllMocks()
    isTauri.mockReturnValue(false)
    invoke.mockReset()
    backendTestHarness.reset()
  })

  test('covers import preview, execute, revert, and doctor review in a translated locale', async () => {
    const user = userEvent.setup()
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true)
    const { snapshot } = await seedInitializedSnapshot()
    const importT = createNamespaceTranslator('zh-CN', 'import')
    const zhCnT = createTranslator('zh-CN')

    renderTrustPage(<ImportPage />, {
      language: 'zh-CN',
      route: '/import',
      snapshot,
    })

    await user.type(
      screen.getByPlaceholderText('/path/to/takeout.zip'),
      '/tmp/takeout',
    )
    await user.click(
      screen.getByRole('button', { name: importT('scanSource') }),
    )

    expect(await screen.findByText(importT('previewTitle'))).toBeVisible()
    expect(await screen.findByText('PathKeep trust UX notes')).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: importT('confirmImport') }),
    )
    expect(await screen.findByText(importT('completeTitle'))).toBeVisible()
    expect(
      (await screen.findAllByText(importT('imported'))).length,
    ).toBeGreaterThan(0)

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: importT('revertBatch') }),
      ).toBeEnabled(),
    )
    await user.click(
      screen.getByRole('button', { name: importT('revertBatch') }),
    )
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: importT('restoreBatch') }),
      ).toBeEnabled(),
    )

    await user.click(
      screen.getByRole('button', { name: importT('runHealthCheckAction') }),
    )
    expect(
      await screen.findByRole('heading', {
        name: new RegExp(zhCnT('common.statusNeedsAttention')),
      }),
    ).toBeVisible()

    confirmSpy.mockRestore()
  })

  test('keeps the workflow collapsed by default and prioritizes detected browser profiles over manual paths', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const importT = createNamespaceTranslator('en', 'import')

    renderTrustPage(<ImportPage />, {
      language: 'en',
      route: '/import',
      snapshot,
    })

    expect(screen.getByText(importT('workflowCollapsedHint'))).toBeVisible()
    expect(
      screen.queryByText(importT('workflowPreviewTitle')),
    ).not.toBeInTheDocument()

    await user.click(
      screen.getByRole('button', { name: importT('showWorkflow') }),
    )
    expect(
      await screen.findByText(importT('workflowPreviewTitle')),
    ).toBeVisible()

    await user.click(screen.getByRole('button', { name: /Browser Direct/i }))

    expect(
      await screen.findByText(importT('detectedBrowserProfiles')),
    ).toBeVisible()
    expect(
      screen.getByRole('button', { name: /Google Chrome · Primary/i }),
    ).toBeVisible()
    expect(
      screen.queryByPlaceholderText('/path/to/History'),
    ).not.toBeInTheDocument()

    await user.click(
      screen.getByRole('button', { name: importT('showManualPath') }),
    )
    expect(screen.getByPlaceholderText('/path/to/History')).toBeVisible()
  })

  test('clears stale batch detail when the newly selected preview fails', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const batches: ImportBatchOverview[] = [
      {
        id: 1,
        sourceKind: 'takeout',
        sourcePath: '/tmp/takeout-a',
        profileId: 'takeout::browser-history',
        createdAt: '2026-04-10T10:00:00.000Z',
        importedAt: '2026-04-10T10:05:00.000Z',
        revertedAt: null,
        status: 'imported',
        candidateItems: 1,
        importedItems: 1,
        duplicateItems: 0,
        visibleItems: 1,
        auditPath: '/tmp/import-audit-a.json',
        gitCommit: null,
      },
      {
        id: 2,
        sourceKind: 'takeout',
        sourcePath: '/tmp/takeout-b',
        profileId: 'takeout::browser-history',
        createdAt: '2026-04-10T11:00:00.000Z',
        importedAt: '2026-04-10T11:05:00.000Z',
        revertedAt: null,
        status: 'imported',
        candidateItems: 1,
        importedItems: 1,
        duplicateItems: 0,
        visibleItems: 1,
        auditPath: '/tmp/import-audit-b.json',
        gitCommit: null,
      },
    ]
    snapshot.recentImportBatches = batches

    const previewByBatch: Record<number, ImportBatchDetail> = {
      1: {
        batch: batches[0],
        previewEntries: [
          {
            sourcePath: '/tmp/takeout-a',
            url: 'https://example.com/first',
            title: 'First batch entry',
            visitedAt: '2026-04-10T10:04:00.000Z',
            sourceVisitId: 1,
            status: 'imported',
          },
        ],
        recognizedFiles: [],
        quarantinedFiles: [],
        notes: [],
      },
    }
    vi.spyOn(backend, 'previewImportBatch').mockImplementation((batchId) => {
      if (batchId in previewByBatch) {
        return Promise.resolve(previewByBatch[batchId])
      }
      return Promise.reject(new Error('Batch detail unavailable'))
    })

    renderTrustPage(<ImportPage />, {
      language: 'en',
      route: '/import?batch=1',
      snapshot,
    })

    expect(await screen.findByText('https://example.com/first')).toBeVisible()

    await user.click(
      screen.getByRole('button', {
        name: /Batch #2/,
      }),
    )

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent(
        'Batch detail unavailable',
      ),
    )
    expect(
      screen.queryByText('https://example.com/first'),
    ).not.toBeInTheDocument()
  })

  test('renders Windows scheduler guidance and keeps PME tabs keyboard reachable', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('zh-TW', 'schedule')
    const zhTwT = createTranslator('zh-TW')
    const previewSpy = vi.spyOn(backend, 'previewSchedule').mockResolvedValue({
      platform: 'windows',
      label: 'com.yi-ting.pathkeep.backup',
      executablePath: 'C:/Program Files/PathKeep/pathkeep.exe',
      generatedFiles: [
        {
          relativePath: 'schedule/com.yi-ting.pathkeep.task.xml',
          absolutePath:
            'C:/Users/test/AppData/Local/com.yi-ting.pathkeep/schedule/com.yi-ting.pathkeep.task.xml',
          purpose: 'Task Scheduler XML',
          contents:
            '<Task><Settings><StartWhenAvailable>true</StartWhenAvailable></Settings></Task>',
        },
      ],
      manualSteps: ['Review the XML before import.'],
      applyCommands: [
        ['schtasks', '/Create', '/XML', 'com.yi-ting.pathkeep.task.xml'],
      ],
      rollbackCommands: [
        ['schtasks', '/Delete', '/TN', 'com.yi-ting.pathkeep.backup', '/F'],
      ],
      applySupported: false,
    })
    const statusSpy = vi.spyOn(backend, 'scheduleStatus').mockResolvedValue({
      platform: 'windows',
      label: 'com.yi-ting.pathkeep.backup',
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

    const workflowPanel = expectHtmlElement(
      (await screen.findByText(scheduleT('pmeTitle'))).closest('.panel'),
    )

    expect(
      await screen.findByRole('heading', {
        name: zhTwT(platformLabelKey('windows')),
      }),
    ).toBeVisible()
    expect(
      await screen.findByText(scheduleT('intervalValue', { hours: 48 })),
    ).toBeVisible()
    expect(
      await screen.findByText(scheduleT('verificationValue', { hours: 6 })),
    ).toBeVisible()

    const previewTab = within(workflowPanel).getByRole('button', {
      name: zhTwT('common.previewTab'),
    })
    const manualTab = within(workflowPanel).getByRole('button', {
      name: zhTwT('common.manualTab'),
    })
    const executeTab = within(workflowPanel).getByRole('button', {
      name: zhTwT('common.executeTab'),
    })
    const verifyTab = within(workflowPanel).getByRole('button', {
      name: zhTwT('common.verifyTab'),
    })

    previewTab.focus()
    expect(previewTab).toHaveFocus()
    await user.tab()
    expect(manualTab).toHaveFocus()
    await user.tab()
    expect(executeTab).toHaveFocus()
    await user.tab()
    expect(verifyTab).toHaveFocus()

    await user.click(verifyTab)
    expect(
      (
        await within(workflowPanel).findAllByText(
          'Review StartWhenAvailable before trusting the schedule.',
        )
      )[0],
    ).toBeVisible()

    previewSpy.mockRestore()
    statusSpy.mockRestore()
  })

  test('shows apply and remove controls when the schedule supports direct execution', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const scheduleT = createNamespaceTranslator('en', 'schedule')
    const enT = createTranslator('en')

    backendTestHarness.seedSchedule(
      {
        platform: 'macos',
        label: 'com.yi-ting.pathkeep.backup',
        executablePath: '/Applications/PathKeep.app',
        generatedFiles: [
          {
            relativePath: 'schedule/com.yi-ting.pathkeep.backup.plist',
            absolutePath:
              '/Users/test/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
            purpose: 'LaunchAgent plist',
            contents:
              '<?xml version="1.0"?><plist><dict><key>Label</key><string>com.yi-ting.pathkeep.backup</string></dict></plist>',
          },
        ],
        manualSteps: ['Review the LaunchAgent install.'],
        applyCommands: [['launchctl', 'bootstrap']],
        rollbackCommands: [['launchctl', 'bootout']],
        applySupported: true,
      },
      {
        platform: 'macos',
        label: 'com.yi-ting.pathkeep.backup',
        dueAfterHours: 72,
        checkIntervalHours: 6,
        applySupported: true,
        installState: 'installed',
        detectedFiles: [
          '~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
        ],
        manualSteps: [
          'Remove the LaunchAgent if you no longer want automation.',
        ],
        auditPath: null,
        lastSuccessfulBackupAt: null,
        warnings: [],
      },
    )

    renderTrustPage(<SchedulePage />, {
      language: 'en',
      route: '/schedule',
      snapshot,
    })

    const workflowPanel = expectHtmlElement(
      (await screen.findByText(scheduleT('pmeTitle'))).closest('.panel'),
    )

    await user.click(
      within(workflowPanel).getByRole('button', {
        name: enT('common.executeTab'),
      }),
    )

    expect(await screen.findByText('launchctl bootstrap')).toBeVisible()
    expect(
      screen.getByRole('button', { name: scheduleT('applySchedule') }),
    ).toBeEnabled()
    expect(
      screen.getByRole('button', { name: scheduleT('removeSchedule') }),
    ).toBeEnabled()
  })

  test('renders rekey preview in Traditional Chinese without English mode fallbacks', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const securityT = createNamespaceTranslator('zh-TW', 'security')
    const zhTwT = createTranslator('zh-TW')

    renderTrustPage(<SecurityPage />, {
      language: 'zh-TW',
      route: '/security',
      snapshot,
    })

    expect(
      await screen.findByText(
        zhTwT('security.archiveIs', {
          mode: zhTwT(securityModeKey('encrypted')),
        }),
      ),
    ).toBeVisible()

    await user.selectOptions(
      screen.getByLabelText(securityT('targetMode')),
      screen.getByRole('option', { name: '明文' }),
    )
    await user.click(
      screen.getByRole('button', { name: securityT('previewRekey') }),
    )

    expect(await screen.findByText('加密 → 明文')).toBeVisible()
  })

  test('shows the latest rekey review path and audit shortcut on the security page', async () => {
    await seedInitializedSnapshot()
    await backend.rekeyArchive({ newMode: 'Plaintext', newKey: null })
    const snapshot = await backend.getAppSnapshot()

    renderTrustPage(<SecurityPage />, {
      language: 'en',
      route: '/security',
      snapshot,
    })

    expect(await screen.findByText(/archive-before-rekey/)).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Open last rekey review' }),
    ).toHaveAttribute('href', expect.stringContaining('/audit?run='))
  })

  test('renders the retention prune panel in settings and executes the selected cleanup', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedInitializedSnapshot()
    const previewSpy = vi
      .spyOn(backend, 'previewRetentionPrune')
      .mockResolvedValue({
        buckets: [
          {
            id: 'snapshots',
            bytes: 2048,
            itemCount: 2,
            paths: [snapshot.directories.rawSnapshotsDir],
          },
          {
            id: 'exports',
            bytes: 0,
            itemCount: 0,
            paths: [snapshot.directories.exportsDir],
          },
        ],
        warnings: ['Snapshots stay local until you explicitly prune them.'],
      })
    const runSpy = vi.spyOn(backend, 'runRetentionPrune').mockResolvedValue({
      runId: 44,
      deletedBytes: 2048,
      deletedFiles: 2,
      buckets: [
        {
          id: 'snapshots',
          bytes: 2048,
          itemCount: 2,
          paths: [snapshot.directories.rawSnapshotsDir],
        },
      ],
      warnings: [],
    })
    const refreshSpy = vi.fn().mockResolvedValue(undefined)

    render(
      <MemoryRouter initialEntries={['/settings']}>
        <I18nContext.Provider value={createI18nValue('en')}>
          <ShellDataContext.Provider
            value={{
              ...createShellValue(snapshot, dashboard),
              refreshAppData: refreshSpy,
            }}
          >
            <SettingsPage />
          </ShellDataContext.Provider>
        </I18nContext.Provider>
      </MemoryRouter>,
    )

    expect(await screen.findByText('RETENTION & CLEANUP')).toBeVisible()
    expect(await screen.findByText(/Snapshots stay local/)).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Prune selected' }))

    await waitFor(() =>
      expect(runSpy).toHaveBeenCalledWith({ bucketIds: ['snapshots'] }),
    )
    expect(
      await screen.findByRole('link', { name: 'Open prune review' }),
    ).toHaveAttribute('href', '/audit?run=44')

    previewSpy.mockRestore()
    runSpy.mockRestore()
  })

  test('filters audit runs and shows delta against the previous visible run', async () => {
    const user = userEvent.setup()
    const auditT = createNamespaceTranslator('en', 'audit')
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

    await user.click(
      screen.getByRole('button', { name: auditT('artifactsTab') }),
    )
    expect(await screen.findByText(/\/tmp\/run-10\.snapshot/)).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: auditT('warningsTab') }),
    )
    expect(await screen.findByText('Schedule drift detected')).toBeVisible()

    loadAuditRunDetailSpy.mockRestore()
  })

  test('keeps successful audit detail cache entries when one run detail fails', async () => {
    const auditT = createNamespaceTranslator('en', 'audit')
    const snapshot = await backend.getAppSnapshot()
    snapshot.config.initialized = true
    snapshot.recentRuns = [
      {
        id: 21,
        startedAt: '2026-04-07T10:00:00.000Z',
        finishedAt: '2026-04-07T10:05:00.000Z',
        status: 'success',
        runType: 'import',
        trigger: 'manual',
        profileScope: ['takeout::browser-history'],
        manifestHash: 'hash-21',
        profilesProcessed: 1,
        newVisits: 3,
        newUrls: 2,
        newDownloads: 0,
      },
      {
        id: 20,
        startedAt: '2026-04-06T10:00:00.000Z',
        finishedAt: '2026-04-06T10:05:00.000Z',
        status: 'success',
        runType: 'backup',
        trigger: 'manual',
        profileScope: ['chrome:Default'],
        manifestHash: 'hash-20',
        profilesProcessed: 1,
        newVisits: 4,
        newUrls: 3,
        newDownloads: 0,
      },
    ]

    const detailMap: Record<number, AuditRunDetail> = {
      21: {
        run: snapshot.recentRuns[0],
        trigger: 'manual',
        timezone: 'America/Phoenix',
        dueOnly: false,
        profileScope: ['takeout::browser-history'],
        warnings: [],
        errorMessage: null,
        stats: {},
        manifestPath: '/tmp/run-21.json',
        manifestHash: 'hash-21',
        artifacts: [
          {
            kind: 'manifest',
            path: '/tmp/run-21.json',
            createdAt: '2026-04-07T10:05:00.000Z',
          },
        ],
      },
    }

    vi.spyOn(backend, 'loadAuditRunDetail').mockImplementation((runId) => {
      const detail = detailMap[runId]
      if (detail) {
        return Promise.resolve(detail)
      }
      return Promise.reject(new Error(`Run ${runId} detail unavailable`))
    })

    renderTrustPage(<AuditPage />, {
      language: 'en',
      route: '/audit?run=21',
      snapshot,
    })

    await waitFor(() =>
      expect(
        within(screen.getByLabelText(auditT('filterArtifactType'))).getByRole(
          'option',
          { name: 'manifest' },
        ),
      ).toBeInTheDocument(),
    )
  })
})
