/**
 * This test file protects the shipped behavior of the Intelligence Surfaces.test.tsx front-end surface.
 *
 * Why this file exists:
 * - These assertions keep route-level trust, loading, and degraded-state promises from quietly regressing.
 * - If a design or product contract changes, the corresponding test should move with it instead of letting the route drift.
 *
 * Main declarations:
 * - `createI18nValue`
 * - `createShellValue`
 * - `renderSurface`
 * - `ScopeSwitcher`
 * - `seedArchiveState`
 * - `enableAi`
 *
 * Source-of-truth notes:
 * - Route behavior is defined jointly by `docs/design/screens-and-nav.md`, `docs/design/ux-principles.md`, and the relevant feature docs.
 * - Tests should verify real user-facing promises such as deep links, scoped callouts, loading grammar, and repair entry points.
 */

import type { ReactNode } from 'react'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../app/shell-data-context'
import { backend } from '../lib/backend-client'
import { backendTestHarness } from '../lib/backend'
import { I18nContext, type I18nContextValue } from '../lib/i18n/context'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from '../lib/i18n'
import * as coreIntelligenceApi from '../lib/core-intelligence/api'
import { ProfileScopeProvider } from '../lib/profile-scope'
import type {
  AiProviderConnectionTestReport,
  AiQueueStatus,
  AppConfig,
  AppSnapshot,
  DashboardSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../lib/types'
import { AssistantPage } from './assistant'
import { DashboardPage } from './dashboard'
import { ExplorerPage } from './explorer'
import { DomainDeepDiveRoutePage, IntelligencePage } from './intelligence'
import { JobsPage } from './jobs'
import { SettingsPage } from './settings'

const baseConfig: AppConfig = {
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
        version: 'diagnostic',
      },
    ],
  },
  deterministic: {
    modules: [
      { id: 'query-groups', enabled: true, version: 'diagnostic' },
      { id: 'threads', enabled: true, version: 'diagnostic' },
      { id: 'reference-pages', enabled: true, version: 'diagnostic' },
      { id: 'source-effectiveness', enabled: true, version: 'diagnostic' },
      { id: 'template-summaries', enabled: true, version: 'diagnostic' },
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
      'You are an audit-first history research assistant. Use the available browser history evidence before answering.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

/**
 * Creates i18n value.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
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

/**
 * Creates shell value.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
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
    refreshKey: 1,
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

/**
 * Explains how render surface works.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function renderSurface(
  ui: ReactNode,
  {
    dashboard = null,
    language = 'en' as ResolvedLanguage,
    route = '/',
    shellValue,
    snapshot,
  }: {
    dashboard?: DashboardSnapshot | null
    language?: ResolvedLanguage
    route?: string
    shellValue?: ShellDataContextValue
    snapshot: AppSnapshot
  },
) {
  return render(
    <MemoryRouter initialEntries={[route]}>
      <I18nContext.Provider value={createI18nValue(language)}>
        <ProfileScopeProvider>
          <ShellDataContext.Provider
            value={shellValue ?? createShellValue(snapshot, dashboard)}
          >
            {ui}
          </ShellDataContext.Provider>
        </ProfileScopeProvider>
      </I18nContext.Provider>
    </MemoryRouter>,
  )
}

/**
 * Explains how seed archive state works.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
async function seedArchiveState() {
  await backend.initializeArchive(baseConfig, 'vault-passphrase')
  await backend.runBackupNow(false)

  const snapshot = structuredClone(await backend.getAppSnapshot())
  const dashboard = structuredClone(await backend.loadDashboardSnapshot())

  return { snapshot, dashboard }
}

/**
 * Explains how enable ai works.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function enableAi(snapshot: AppSnapshot) {
  snapshot.config.ai = {
    ...snapshot.config.ai,
    enabled: true,
    assistantEnabled: true,
    semanticIndexEnabled: true,
    llmProviderId: 'llm-local',
    embeddingProviderId: 'embed-local',
    llmProviders: [
      {
        id: 'llm-local',
        name: 'Local LLM',
        purpose: 'llm',
        requestFormat: 'openai',
        enabled: true,
        baseUrl: 'http://localhost:11434',
        apiKeySaved: false,
        defaultModel: 'qwen3:8b',
        modelCatalog: [],
        temperature: 0.2,
        maxTokens: 1200,
        dimensions: null,
        notes: null,
      },
    ],
    embeddingProviders: [
      {
        id: 'embed-local',
        name: 'Local Embedding',
        purpose: 'embedding',
        requestFormat: 'openai',
        enabled: true,
        baseUrl: 'http://localhost:11434',
        apiKeySaved: false,
        defaultModel: 'nomic-embed-text',
        modelCatalog: [],
        temperature: null,
        maxTokens: null,
        dimensions: 768,
        notes: null,
      },
    ],
  }
  snapshot.aiStatus = {
    ...snapshot.aiStatus,
    enabled: true,
    assistantEnabled: true,
    ready: true,
    state: 'ready',
    indexedItems: 128,
    llmProviderId: 'llm-local',
    embeddingProviderId: 'embed-local',
    queuedJobs: 1,
    runningJobs: 1,
  }
}

describe('intelligence surfaces', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    backendTestHarness.reset()
    window.localStorage.clear()
  })

  test('renders localized dashboard intelligence and trust callouts', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('zh-TW', 'dashboard')
    const commonT = createNamespaceTranslator('zh-TW', 'common')

    enableAi(snapshot)
    snapshot.config.rememberDatabaseKeyInKeyring = true
    snapshot.keyringStatus.storedSecret = false
    snapshot.browserProfiles.push({
      profileId: 'safari:Personal',
      profileName: 'Personal',
      browserFamily: 'safari',
      browserName: 'Safari',
      userName: 'tim',
      profilePath: '/Users/tim/Library/Safari',
      historyPath: '/Users/tim/Library/Safari/History.db',
      faviconsPath: null,
      historyExists: false,
      browserVersion: null,
      historyFileName: 'History.db',
      historyBytes: 18 * 1024 * 1024,
      faviconsBytes: 0,
      supportingBytes: 2 * 1024 * 1024,
      retentionBoundary: {
        kind: 'macos-safari',
        localDays: 365,
      },
    })
    snapshot.config.selectedProfileIds.push('safari:Personal')

    renderSurface(<DashboardPage />, {
      dashboard,
      language: 'zh-TW',
      route: '/',
      snapshot,
    })

    expect(
      await screen.findByText(dashboardT('intelligenceTitle')),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('semanticSearchAction') }),
    ).toBeVisible()
    expect(
      screen.getAllByRole('link', { name: dashboardT('reviewInsightsAction') }),
    ).toHaveLength(2)
    expect(
      screen.getAllByRole('link', { name: dashboardT('reviewSecurity') }),
    ).toHaveLength(2)
    expect(
      screen.getAllByRole('link', { name: dashboardT('reviewImportBatches') }),
    ).toHaveLength(2)
    expect(
      screen.getByText(commonT('browserRetentionManagedLabel')),
    ).toBeVisible()
    expect(
      screen.getAllByText((content) =>
        content.includes(commonT('browserRetentionArchiveBoundary')),
      ).length,
    ).toBeGreaterThan(0)
  })

  test('routes dashboard archive-key failures toward the security page', async () => {
    const { snapshot } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('en', 'dashboard')

    renderSurface(<DashboardPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        dashboard: null,
        snapshot: null,
        error: 'database key is required for encrypted archives',
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: dashboardT('archiveUnlockRequiredTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByText(dashboardT('archiveUnlockRequiredBody')),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('archiveUnlockAction') }),
    ).toHaveAttribute('href', '/security#unlock-archive')
  })

  test('renders top-sites inside a scroll region so long lists do not stretch the section', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      Array.from({ length: 20 }, (_, index) => ({
        registrableDomain: `example-${index + 1}.com`,
        displayName: `Example ${index + 1}`,
        domainCategory: 'reference',
        visitCount: 100 - index,
        uniqueDays: 20 - Math.floor(index / 2),
        averageDailyVisits: Number((5 - index * 0.1).toFixed(1)),
        uniqueUrls: 10 - Math.floor(index / 3),
      })),
    )

    renderSurface(<IntelligencePage />, {
      route: '/intelligence?profileId=chrome:Default',
      snapshot,
    })

    expect(await screen.findByTestId('intelligence-page')).toBeVisible()

    const topSitesSection = screen
      .getByRole('heading', { name: intelligenceT('topSitesTitle') })
      .closest('section')
    expect(topSitesSection).not.toBeNull()
    if (!(topSitesSection instanceof HTMLElement)) {
      throw new Error('expected top sites section')
    }

    expect(
      topSitesSection.querySelector('.intelligence-section__scroll-region'),
    ).not.toBeNull()
  })

  test('shows a security recovery empty state in settings when the archive needs unlocking', async () => {
    const { snapshot } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const dashboardT = createNamespaceTranslator('en', 'dashboard')

    await backend.clearSessionDatabaseKey()

    renderSurface(<SettingsPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        dashboard: null,
        snapshot: null,
        error: 'database key is required for encrypted archives',
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: settingsT('archiveUnlockTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('reviewSecurity') }),
    ).toHaveAttribute('href', '/security')
  })

  test('renders settings enrichment runtime review and syncs plugin toggles', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const runtimeSnapshot: IntelligenceRuntimeSnapshot = {
      queue: {
        queued: 1,
        running: 0,
        succeeded: 1,
        failed: 1,
        cancelled: 0,
        lastActivityAt: '2026-04-10T16:30:00Z',
      },
      plugins: [
        {
          pluginId: 'title-normalization',
          sourceKind: 'local',
          enabled: true,
          storedRecords: 42,
          queuedJobs: 0,
          runningJobs: 0,
          failedJobs: 0,
          lastCompletedAt: '2026-04-10T16:20:00Z',
          lastError: null,
        },
        {
          pluginId: 'readable-content-refetch',
          sourceKind: 'network',
          enabled: true,
          storedRecords: 8,
          queuedJobs: 1,
          runningJobs: 0,
          failedJobs: 1,
          lastCompletedAt: '2026-04-10T15:40:00Z',
          lastError: '429 from upstream host',
        },
      ],
      modules: [
        {
          moduleId: 'query-groups',
          enabled: true,
          version: 'diagnostic',
          status: 'ready',
          dependsOn: [],
          derivedTables: ['insight_bursts', 'insight_query_groups'],
          lastRunId: 12,
          lastBuiltAt: '2026-04-10T16:25:00Z',
          lastInvalidatedAt: null,
          staleReason: null,
          notes: ['Latest deterministic rebuild completed successfully.'],
        },
        {
          moduleId: 'reference-pages',
          enabled: true,
          version: 'diagnostic',
          status: 'stale',
          dependsOn: ['query-groups', 'threads'],
          derivedTables: ['insight_reference_pages'],
          lastRunId: 11,
          lastBuiltAt: '2026-04-09T16:25:00Z',
          lastInvalidatedAt: '2026-04-10T16:28:00Z',
          staleReason:
            'Visibility changed after the last deterministic rebuild.',
          notes: [
            'Manual rebuild required before this deterministic module is fresh again.',
          ],
        },
      ],
      recentJobs: [
        {
          id: 411,
          jobType: 'enrichment-plugin',
          pluginId: 'readable-content-refetch',
          state: 'failed',
          historyId: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Article',
          attempt: 2,
          createdAt: '2026-04-10T15:35:00Z',
          startedAt: '2026-04-10T15:36:00Z',
          finishedAt: '2026-04-10T15:37:00Z',
          updatedAt: '2026-04-10T15:37:00Z',
          heartbeatAt: null,
          progressLabel: null,
          progressDetail: null,
          progressCurrent: null,
          progressTotal: null,
          progressPercent: null,
          lastError: '429 from upstream host',
          retryable: true,
          cancellable: false,
        },
      ],
      notes: [
        'Browser preview mode shows a deterministic queue/runtime fixture.',
      ],
    }

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      runtimeSnapshot,
    )
    const shellValue = createShellValue(snapshot, dashboard)
    shellValue.saveConfig = vi.fn().mockResolvedValue(snapshot)

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'en',
      route: '/settings',
      shellValue,
      snapshot,
    })

    expect(
      await screen.findByText(settingsT('firstPartyRuntimeTitle')),
    ).toBeVisible()
    expect(screen.getByText('Title normalization')).toBeVisible()
    expect(screen.getByText('Query groups')).toBeVisible()
    expect(screen.getByText('Reference pages')).toBeVisible()
    expect(screen.getAllByText('Page content fetcher').length).toBeGreaterThan(
      0,
    )
    expect(
      screen.getAllByText('1 queued / 0 running / 1 failed').length,
    ).toBeGreaterThan(0)

    const titleNormalizationRow = screen
      .getByText('Title normalization')
      .closest('.result-row')
    expect(titleNormalizationRow).not.toBeNull()
    if (!(titleNormalizationRow instanceof HTMLElement)) {
      throw new Error('expected title normalization row')
    }
    await user.click(
      within(titleNormalizationRow).getByRole('button', {
        name: settingsT('disablePlugin'),
      }),
    )

    await waitFor(() => expect(shellValue.saveConfig).toHaveBeenCalledTimes(1))
    expect(shellValue.saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        enrichment: {
          plugins: expect.arrayContaining([
            expect.objectContaining({
              id: 'title-normalization',
              enabled: false,
            }),
          ]),
        },
        ai: expect.objectContaining({
          enrichmentPlugins: expect.arrayContaining([
            expect.objectContaining({
              pluginId: 'title-normalization',
              enabled: false,
            }),
          ]),
        }),
      }),
    )
  })

  test('renders background jobs controls and lets the user pause or replay work', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const queueStatus: AiQueueStatus = {
      paused: false,
      concurrency: 2,
      queued: 1,
      running: 1,
      failed: 1,
      recentJobs: [
        {
          id: 77,
          jobType: 'index-build',
          state: 'failed',
          priority: 10,
          attempt: 2,
          maxAttempts: 3,
          runId: null,
          summary: 'Provider quota window has not reset yet.',
          queuedAt: '2026-04-07T18:00:00Z',
          availableAt: '2026-04-07T18:00:00Z',
          startedAt: '2026-04-07T18:01:00Z',
          finishedAt: '2026-04-07T18:02:00Z',
          heartbeatAt: '2026-04-07T18:01:30Z',
          errorCode: 'rate-limited',
          errorMessage: '429',
        },
        {
          id: 78,
          jobType: 'assistant',
          state: 'queued',
          priority: 10,
          attempt: 1,
          maxAttempts: 3,
          runId: null,
          summary: null,
          queuedAt: '2026-04-07T18:03:00Z',
          availableAt: '2026-04-07T18:03:00Z',
          startedAt: null,
          finishedAt: null,
          heartbeatAt: null,
          errorCode: null,
          errorMessage: null,
        },
      ],
    }
    const runtimeSnapshot: IntelligenceRuntimeSnapshot = {
      queue: {
        queued: 1,
        running: 1,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        lastActivityAt: '2026-04-10T16:30:00Z',
      },
      plugins: [
        {
          pluginId: 'readable-content-refetch',
          sourceKind: 'network',
          enabled: true,
          storedRecords: 5,
          queuedJobs: 1,
          runningJobs: 0,
          failedJobs: 1,
          lastCompletedAt: '2026-04-10T16:20:00Z',
          lastError: '429 from upstream host',
        },
      ],
      modules: [
        {
          moduleId: 'threads',
          enabled: true,
          version: 'diagnostic',
          status: 'stale',
          dependsOn: ['query-groups'],
          derivedTables: ['insight_threads', 'insight_thread_members'],
          lastRunId: 12,
          lastBuiltAt: '2026-04-10T16:25:00Z',
          lastInvalidatedAt: '2026-04-10T16:28:00Z',
          staleReason: 'New imports were added after the last rebuild.',
          notes: ['Thread merge uses query-family and reopen evidence.'],
        },
      ],
      recentJobs: [
        {
          id: 411,
          jobType: 'deterministic-rebuild',
          pluginId: null,
          state: 'running',
          historyId: null,
          profileId: 'chrome:Default',
          url: null,
          title: 'chrome:Default · 30 days',
          attempt: 2,
          createdAt: '2026-04-10T15:35:00Z',
          startedAt: '2026-04-10T15:36:00Z',
          finishedAt: null,
          updatedAt: '2026-04-10T15:36:45Z',
          heartbeatAt: '2026-04-10T15:36:45Z',
          progressLabel: 'Scoring visits',
          progressDetail: '24,000 / 64,781 visits',
          progressCurrent: 24000,
          progressTotal: 64781,
          progressPercent: 46.8,
          lastError: null,
          retryable: false,
          cancellable: true,
        },
        {
          id: 412,
          jobType: 'enrichment-plugin',
          pluginId: 'readable-content-refetch',
          state: 'failed',
          historyId: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Article',
          attempt: 2,
          createdAt: '2026-04-10T15:20:00Z',
          startedAt: '2026-04-10T15:21:00Z',
          finishedAt: '2026-04-10T15:22:00Z',
          updatedAt: '2026-04-10T15:22:00Z',
          heartbeatAt: null,
          progressLabel: null,
          progressDetail: null,
          progressCurrent: null,
          progressTotal: null,
          progressPercent: null,
          lastError: '429 from upstream host',
          retryable: true,
          cancellable: false,
        },
      ],
      notes: [
        'Recovered 1 interrupted deterministic rebuild job after restart.',
      ],
    }

    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue(queueStatus)
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      runtimeSnapshot,
    )
    const replaySpy = vi
      .spyOn(backend, 'replayAiJob')
      .mockResolvedValue(queueStatus.recentJobs[0])
    const retrySpy = vi
      .spyOn(backend, 'retryIntelligenceJob')
      .mockResolvedValue(runtimeSnapshot)

    const pausedSnapshot = structuredClone(snapshot)
    pausedSnapshot.config.ai.jobQueuePaused = true
    const shellValue = createShellValue(snapshot)
    shellValue.saveConfig = vi.fn().mockResolvedValue(pausedSnapshot)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(await screen.findByText(jobsT('failedTitle'))).toBeVisible()
    expect(screen.getByText(jobsT('statusEyebrow'))).toBeVisible()
    expect(screen.getByText('Derived-data queue')).toBeVisible()
    expect(screen.getByText('Scoring visits')).toBeVisible()
    expect(
      screen.getAllByText('24,000 / 64,781 visits').length,
    ).toBeGreaterThan(0)
    expect(screen.getByText('47%')).toBeVisible()

    await user.click(screen.getByRole('button', { name: jobsT('pauseQueue') }))
    await waitFor(() =>
      expect(shellValue.saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          ai: expect.objectContaining({ jobQueuePaused: true }),
        }),
      ),
    )

    const aiPanel = screen.getByText(jobsT('recentAiJobs')).closest('.panel')
    expect(aiPanel).not.toBeNull()
    if (!(aiPanel instanceof HTMLElement)) {
      throw new Error('expected recent ai jobs panel')
    }
    await user.click(
      within(aiPanel).getAllByRole('button', { name: jobsT('retryJob') })[0],
    )
    expect(replaySpy).toHaveBeenCalledWith(77)

    const runtimePanel = screen
      .getByText(jobsT('recentRuntimeJobs'))
      .closest('.panel')
    expect(runtimePanel).not.toBeNull()
    if (!(runtimePanel instanceof HTMLElement)) {
      throw new Error('expected recent runtime jobs panel')
    }
    await user.click(
      within(runtimePanel).getByRole('button', { name: jobsT('retryJob') }),
    )
    expect(retrySpy).toHaveBeenCalledWith(412)
  })

  test('keeps failed backlog honest even when the latest runtime item is still running', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue({
      paused: false,
      concurrency: 1,
      queued: 0,
      running: 0,
      failed: 0,
      recentJobs: [],
    })
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue({
      queue: {
        queued: 3,
        running: 1,
        succeeded: 0,
        failed: 2,
        cancelled: 0,
        lastActivityAt: '2026-04-10T16:30:00Z',
      },
      plugins: [
        {
          pluginId: 'readable-content-refetch',
          sourceKind: 'network',
          enabled: true,
          storedRecords: 5,
          queuedJobs: 3,
          runningJobs: 1,
          failedJobs: 2,
          lastCompletedAt: '2026-04-10T16:20:00Z',
          lastError: 'unsupported-content',
        },
      ],
      modules: [],
      recentJobs: [
        {
          id: 990,
          jobType: 'enrichment-plugin',
          pluginId: 'readable-content-refetch',
          state: 'running',
          historyId: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Article',
          attempt: 1,
          createdAt: '2026-04-10T15:20:00Z',
          startedAt: '2026-04-10T15:21:00Z',
          finishedAt: null,
          updatedAt: '2026-04-10T15:22:00Z',
          heartbeatAt: null,
          progressLabel: null,
          progressDetail: null,
          progressCurrent: null,
          progressTotal: null,
          progressPercent: null,
          lastError: null,
          retryable: false,
          cancellable: true,
        },
      ],
      notes: [],
    })

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      snapshot,
    })

    expect(
      await screen.findByText(jobsT('needsReviewBacklog', { count: 2 })),
    ).toBeVisible()
    expect(
      screen.getAllByText(jobsT('errorUnsupportedContent')).length,
    ).toBeGreaterThan(0)
  })

  test('keeps polling while background work is still queued or running', async () => {
    vi.useFakeTimers()

    try {
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      const queueStatus: AiQueueStatus = {
        paused: false,
        concurrency: 1,
        queued: 2,
        running: 1,
        failed: 0,
        recentJobs: [],
      }

      const runtimeSnapshot: IntelligenceRuntimeSnapshot = {
        queue: {
          queued: 1,
          running: 0,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: '2026-04-10T16:30:00Z',
        },
        plugins: [],
        modules: [],
        recentJobs: [],
        notes: [],
      }

      const loadAiQueueStatusSpy = vi
        .spyOn(backend, 'loadAiQueueStatus')
        .mockResolvedValue(queueStatus)
      const loadRuntimeSpy = vi
        .spyOn(backend, 'loadIntelligenceRuntime')
        .mockResolvedValue(runtimeSnapshot)

      renderSurface(<JobsPage />, {
        language: 'en',
        route: '/jobs',
        snapshot,
      })

      await act(async () => {
        await Promise.resolve()
        await Promise.resolve()
      })

      expect(screen.getByText(jobsT('runningTitle'))).toBeVisible()
      expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(1)
      expect(loadRuntimeSpy).toHaveBeenCalledTimes(1)

      await act(async () => {
        await vi.advanceTimersByTimeAsync(3000)
      })

      expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(2)
      expect(loadRuntimeSpy).toHaveBeenCalledTimes(2)

      await act(async () => {
        await vi.advanceTimersByTimeAsync(3000)
      })

      expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(3)
      expect(loadRuntimeSpy).toHaveBeenCalledTimes(3)
    } finally {
      vi.useRealTimers()
    }
  })

  test('renders assistant queue state, provider probe, and answer citations', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const assistantT = createNamespaceTranslator('zh-CN', 'assistant')

    enableAi(snapshot)
    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')

    const queueStatus: AiQueueStatus = {
      paused: false,
      concurrency: 1,
      queued: 1,
      running: 1,
      failed: 0,
      recentJobs: [
        {
          id: 77,
          jobType: 'assistant',
          state: 'queued',
          priority: 10,
          attempt: 1,
          maxAttempts: 3,
          runId: null,
          summary: null,
          queuedAt: '2026-04-07T18:00:00Z',
          availableAt: '2026-04-07T18:00:00Z',
          startedAt: null,
          finishedAt: null,
          heartbeatAt: null,
          errorCode: null,
          errorMessage: null,
        },
      ],
    }
    const providerProbe: AiProviderConnectionTestReport = {
      providerId: 'llm-local',
      purpose: 'llm',
      model: 'qwen3:8b',
      ok: true,
      latencyMs: 210,
      capabilities: {
        supportsChat: true,
        supportsEmbeddings: false,
        supportsStreaming: true,
        supportsToolUse: false,
        supportsStructuredOutput: true,
      },
      warnings: [],
      message: 'Provider responded with a healthy chat completion.',
      actionHint: 'Safe to use for evidence-backed answers.',
      retryHint: null,
      errorCode: null,
    }

    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue(queueStatus)
    vi.spyOn(backend, 'testAiProviderConnection').mockResolvedValue(
      providerProbe,
    )
    vi.spyOn(backend, 'askAiAssistant').mockResolvedValue({
      state: 'completed',
      answer: '语义搜索质量趋势稳定，证据契约文档被多次访问。',
      jobId: 91,
      runId: 12,
      providerId: 'llm-local',
      embeddingProviderId: 'embed-local',
      citations: [
        {
          historyId: 301,
          profileId: 'chrome:Default',
          url: 'https://example.com/semantic-quality',
          title: 'Semantic quality notes',
          visitedAt: '2026-04-06T15:20:00Z',
          score: 0.92,
        },
      ],
      notes: ['Answer kept inside the archive evidence boundary.'],
    })

    renderSurface(<AssistantPage />, {
      language: 'zh-CN',
      route: '/assistant',
      snapshot,
    })

    expect(await screen.findByText(assistantT('scopedViewTitle'))).toBeVisible()
    expect(await screen.findByText(assistantT('runningContext'))).toBeVisible()
    expect(
      await screen.findByText(assistantT('queuedJobLabel', { id: 77 })),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: assistantT('testProvider') }),
    )
    expect(
      await screen.findByText(assistantT('providerReachable')),
    ).toBeVisible()

    const input = await screen.findByPlaceholderText(
      assistantT('inputPlaceholder'),
    )
    expect(
      screen.getByRole('button', { name: assistantT('sendAction') }),
    ).toBeVisible()
    await user.type(input, '总结最近的证据{enter}')

    expect(
      await screen.findByText('语义搜索质量趋势稳定，证据契约文档被多次访问。'),
    ).toBeVisible()
    expect(
      await screen.findByText(assistantT('evidenceLabel', { count: 1 })),
    ).toBeVisible()

    window.localStorage.removeItem('pathkeep.profile-scope')
  })

  test('inherits shared intelligence scope and lets explicit profileId override it', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const summary = {
      dateRange: { start: '2026-04-01', end: '2026-04-07' },
      totalVisits: { value: 12, trend: 'flat' as const },
      totalSearches: { value: 3, trend: 'flat' as const },
      newDomains: { value: 2, trend: 'flat' as const },
      deepReadPages: { value: 1, trend: 'flat' as const },
      refindPages: { value: 1, trend: 'flat' as const },
    }
    const digestSpy = vi
      .spyOn(coreIntelligenceApi, 'getDigestSummary')
      .mockResolvedValue(summary)

    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')

    try {
      const first = renderSurface(<IntelligencePage />, {
        route: '/intelligence',
        snapshot,
      })

      expect(await screen.findByTestId('intelligence-page')).toBeVisible()
      await waitFor(() =>
        expect(digestSpy).toHaveBeenCalledWith(
          expect.anything(),
          'chrome:Default',
        ),
      )
      expect(
        await screen.findByText(intelligenceT('scopedViewTitle')),
      ).toBeVisible()

      first.unmount()
      digestSpy.mockClear()

      renderSurface(<IntelligencePage />, {
        route: '/intelligence?profileId=firefox:Research',
        snapshot,
      })

      await waitFor(() =>
        expect(digestSpy).toHaveBeenCalledWith(
          expect.anything(),
          'firefox:Research',
        ),
      )
      expect(
        await screen.findByText(intelligenceT('scopedViewTitle')),
      ).toBeVisible()
    } finally {
      window.localStorage.removeItem('pathkeep.profile-scope')
    }
  })

  test('renders explorer session view and keeps navigation tracing wired to the selected grouped visit', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getSessions').mockResolvedValue({
      sessions: [
        {
          sessionId: 'session-1',
          firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
          lastVisitMs: Date.parse('2026-04-05T14:30:00Z'),
          visitCount: 3,
          searchCount: 1,
          domainCount: 2,
          isDeepDive: true,
          autoTitle: 'SQLite WAL research',
        },
      ],
      total: 1,
      page: 0,
      pageSize: 20,
    })
    vi.spyOn(coreIntelligenceApi, 'getSessionDetail').mockResolvedValue({
      session: {
        sessionId: 'session-1',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:30:00Z'),
        visitCount: 3,
        searchCount: 1,
        domainCount: 2,
        isDeepDive: true,
        autoTitle: 'SQLite WAL research',
      },
      visits: [
        {
          visitId: 101,
          url: 'https://www.sqlite.org/wal.html',
          title: 'SQLite WAL',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:05:00Z'),
          isSearchEvent: false,
          searchQuery: null,
          searchEngine: null,
          trailId: null,
          transitionType: 'link',
        },
      ],
      trails: [],
    })
    vi.spyOn(coreIntelligenceApi, 'getNavigationPath').mockResolvedValue({
      targetVisitId: 101,
      steps: [
        {
          visitId: 100,
          url: 'https://www.google.com/search?q=sqlite+wal',
          title: 'Google',
          visitTimeMs: Date.parse('2026-04-05T14:04:00Z'),
          depth: 0,
        },
        {
          visitId: 101,
          url: 'https://www.sqlite.org/wal.html',
          title: 'SQLite WAL',
          visitTimeMs: Date.parse('2026-04-05T14:05:00Z'),
          depth: 1,
        },
      ],
    })

    renderSurface(<ExplorerPage />, {
      route: '/explorer?view=session&start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    expect(await screen.findByText('SQLite WAL research')).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: /SQLite WAL research/i }),
    )
    await user.click(await screen.findByText('SQLite WAL'))
    await user.click(
      screen.getByRole('button', { name: intelligenceT('tracerTitle') }),
    )
    expect(await screen.findByText('Google')).toBeVisible()
    expect(
      screen.getByText(new RegExp(intelligenceT('tracerHere'))),
    ).toBeVisible()
  })

  test('renders explorer trail view and keeps grouped selection wired to the detail rail', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getSearchTrails').mockResolvedValue({
      trails: [
        {
          trailId: 'trail-1',
          sessionId: 'session-1',
          initialQuery: 'sqlite wal checkpoint',
          searchEngine: 'Google',
          reformulationCount: 1,
          visitCount: 2,
          landingUrl: 'https://www.sqlite.org/pragma.html',
          landingDomain: 'sqlite.org',
          firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
          lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
          maxDepth: 2,
          queries: ['sqlite wal checkpoint', 'sqlite wal checkpoint passive'],
        },
      ],
      total: 1,
      page: 0,
      pageSize: 20,
    })
    vi.spyOn(coreIntelligenceApi, 'getTrailDetail').mockResolvedValue({
      trail: {
        trailId: 'trail-1',
        sessionId: 'session-1',
        initialQuery: 'sqlite wal checkpoint',
        searchEngine: 'Google',
        reformulationCount: 1,
        visitCount: 2,
        landingUrl: 'https://www.sqlite.org/pragma.html',
        landingDomain: 'sqlite.org',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
        maxDepth: 2,
        queries: ['sqlite wal checkpoint', 'sqlite wal checkpoint passive'],
      },
      members: [
        {
          trailId: 'trail-1',
          visitId: 201,
          ordinal: 0,
          role: 'search_event',
          url: 'https://www.google.com/search?q=sqlite+wal+checkpoint+passive',
          title: 'Google',
          visitTimeMs: Date.parse('2026-04-05T14:02:00Z'),
          searchQuery: 'sqlite wal checkpoint passive',
        },
        {
          trailId: 'trail-1',
          visitId: 202,
          ordinal: 1,
          role: 'landing',
          url: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          searchQuery: null,
        },
      ],
    })
    vi.spyOn(coreIntelligenceApi, 'getNavigationPath').mockResolvedValue({
      targetVisitId: 202,
      steps: [
        {
          visitId: 201,
          url: 'https://www.google.com/search?q=sqlite+wal+checkpoint+passive',
          title: 'Google',
          visitTimeMs: Date.parse('2026-04-05T14:02:00Z'),
          depth: 0,
        },
        {
          visitId: 202,
          url: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          depth: 1,
        },
      ],
    })

    renderSurface(<ExplorerPage />, {
      route: '/explorer?view=trail&start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    expect(await screen.findByText('"sqlite wal checkpoint"')).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: /sqlite wal checkpoint/i }),
    )
    await user.click(await screen.findByText('PRAGMA wal_checkpoint'))
    await user.click(
      screen.getByRole('button', { name: intelligenceT('tracerTitle') }),
    )
    expect(
      (await screen.findAllByText('PRAGMA wal_checkpoint')).length,
    ).toBeGreaterThan(1)
  })

  test('keeps domain deep dives deep-linkable and preserves route-backed scope and date range', async () => {
    const { snapshot } = await seedArchiveState()

    const domainSpy = vi
      .spyOn(coreIntelligenceApi, 'getDomainDeepDive')
      .mockResolvedValue({
        registrableDomain: 'github.com',
        displayName: 'GitHub',
        domainCategory: 'developer',
        totalVisits: 38,
        activeDays: 7,
        trailCount: 4,
        arrivalBreakdown: { search: 10, link: 12, typed: 8, other: 8 },
        topPages: [{ path: '/issues', visitCount: 12 }],
        topReferrers: [
          { domain: 'google.com', displayName: 'Google', count: 6 },
        ],
        topExits: [
          {
            domain: 'stackoverflow.com',
            displayName: 'Stack Overflow',
            count: 4,
          },
        ],
        visitTrend: [{ dateKey: '2026-04-05', visitCount: 6 }],
      })

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/domain/:domain"
          element={<DomainDeepDiveRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/domain/github.com?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(await screen.findByTestId('domain-deep-dive-page')).toBeVisible()
    await waitFor(() =>
      expect(domainSpy).toHaveBeenCalledWith(
        'github.com',
        { start: '2026-04-01', end: '2026-04-07' },
        'chrome:Default',
      ),
    )
    expect(screen.getByRole('link', { name: /Back/i })).toHaveAttribute(
      'href',
      '/intelligence?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome%3ADefault',
    )
  })

  test('limits path-flow steps to supported values and wires explainability to supported intelligence entities', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue([
      {
        canonicalUrl: 'https://example.com/reference',
        url: 'https://example.com/reference',
        title: 'Reference page',
        registrableDomain: 'example.com',
        crossDayCount: 4,
        trailCount: 3,
        searchArrivalCount: 2,
        typedRevisitCount: 1,
        refindScore: 0.9,
        firstSeenAt: '2026-04-01T00:00:00Z',
        lastSeenAt: '2026-04-07T00:00:00Z',
      },
    ])
    vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue({
      families: [],
      total: 0,
      page: 0,
      pageSize: 10,
    })
    vi.spyOn(
      coreIntelligenceApi,
      'getReopenedInvestigations',
    ).mockResolvedValue([])
    vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue([])
    vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue([])
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue([])
    const explainSpy = vi
      .spyOn(coreIntelligenceApi, 'explainEntity')
      .mockResolvedValue({
        entityType: 'refind_page',
        entityId: 'https://example.com/reference',
        triggerRule: 'Refind score >= 0.7',
        factors: [],
        participatingVisitIds: [1, 2],
      })

    renderSurface(<IntelligencePage />, {
      route: '/intelligence?profileId=chrome:Default',
      snapshot,
    })

    expect(await screen.findByTestId('intelligence-page')).toBeVisible()
    expect(
      screen.queryByText(intelligenceT('pathFlowsStep4')),
    ).not.toBeInTheDocument()

    const refindSection = screen
      .getByText(intelligenceT('refindTitle'))
      .closest('section')
    expect(refindSection).not.toBeNull()
    if (!(refindSection instanceof HTMLElement)) {
      throw new Error('expected refind section')
    }

    await user.click(
      within(refindSection).getByRole('button', {
        name: intelligenceT('explainTitle'),
      }),
    )

    await waitFor(() =>
      expect(explainSpy).toHaveBeenCalledWith(
        'refind_page',
        'https://example.com/reference',
      ),
    )
  })

  test('clears both explorer date bounds in a single interaction', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer?start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    const startInput = await screen.findByLabelText(explorerT('filterStart'))
    const endInput = await screen.findByLabelText(explorerT('filterEnd'))

    expect(startInput).toHaveValue('2026-04-01')
    expect(endInput).toHaveValue('2026-04-07')
    expect(
      screen.getByRole('button', {
        name: explorerT('removeFilter', {
          label: explorerT('filterStart'),
          value: '2026-04-01',
        }),
      }),
    ).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Clear range' }))

    await waitFor(() => {
      expect(startInput).toHaveValue('')
      expect(endInput).toHaveValue('')
    })
  })

  test('keeps the explorer profile control aligned with the shared profile scope', async () => {
    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')

    try {
      const { snapshot } = await seedArchiveState()
      const explorerT = createNamespaceTranslator('en', 'explorer')

      renderSurface(<ExplorerPage />, {
        language: 'en',
        route: '/explorer',
        snapshot,
      })

      expect(
        await screen.findByLabelText(explorerT('filterProfileAria')),
      ).toHaveValue('chrome:Default')
      expect(await screen.findByText(explorerT('scopeInherited'))).toBeVisible()
    } finally {
      window.localStorage.removeItem('pathkeep.profile-scope')
    }
  })

  test('debounces explorer keyword query commits while the user is typing', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const querySpy = vi.spyOn(backend, 'queryHistory')

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()
    await waitFor(() => expect(querySpy).toHaveBeenCalledTimes(1))

    await user.type(
      screen.getByLabelText(explorerT('filterKeywordAria')),
      'sqlite',
    )

    expect(querySpy).toHaveBeenCalledTimes(1)

    await new Promise((resolve) => window.setTimeout(resolve, 220))
    await waitFor(() => expect(querySpy).toHaveBeenCalledTimes(2))
  })
})
