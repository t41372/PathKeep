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
import { MemoryRouter } from 'react-router-dom'
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
import {
  compactInsightText,
  formatInsightCoverage,
} from '../lib/intelligence-presentation'
import { ProfileScopeProvider } from '../lib/profile-scope'
import { useProfileScope } from '../lib/profile-scope-context'
import type {
  AiProviderConnectionTestReport,
  AiQueueStatus,
  AppConfig,
  AppSnapshot,
  DashboardSnapshot,
  InsightExplanation,
  InsightSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../lib/types'
import { AssistantPage } from './assistant'
import { DashboardPage } from './dashboard'
import { ExplorerPage } from './explorer'
import { InsightsPage } from './insights'
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
 * Explains how scope switcher works.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function ScopeSwitcher({ nextProfileId }: { nextProfileId: string }) {
  const { setActiveProfileId } = useProfileScope()
  return (
    <button type="button" onClick={() => setActiveProfileId(nextProfileId)}>
      Switch scope
    </button>
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

  test('renders dashboard on-this-day and periodic summary cards without leaking raw i18n keys', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const insightsT = createNamespaceTranslator('en', 'insights')
    const settingsT = createNamespaceTranslator('en', 'settings')

    renderSurface(<DashboardPage />, {
      dashboard,
      language: 'en',
      route: '/',
      snapshot,
    })

    expect(await screen.findByText(insightsT('onThisDay'))).toBeVisible()
    expect(await screen.findByText(insightsT('periodicSummary'))).toBeVisible()
    expect(
      screen.getByText(
        'Archive tooling moved from broad comparison to a GitHub-restricted query.',
      ),
    ).toBeVisible()
    expect(screen.getByText(settingsT('disabled'))).toBeVisible()
    expect(screen.queryByText('settings.disabled')).not.toBeInTheDocument()
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
      await screen.findByRole('link', { name: dashboardT('reviewSecurity') }),
    ).toHaveAttribute('href', '/security')
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

  test('renders insights runtime queue controls and calls retry or cancel', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const insightsT = createNamespaceTranslator('en', 'insights')
    const runtimeSnapshot: IntelligenceRuntimeSnapshot = {
      queue: {
        queued: 1,
        running: 0,
        succeeded: 2,
        failed: 1,
        cancelled: 0,
        lastActivityAt: '2026-04-10T16:30:00Z',
      },
      plugins: [
        {
          pluginId: 'title-normalization',
          sourceKind: 'local',
          enabled: true,
          storedRecords: 20,
          queuedJobs: 1,
          runningJobs: 0,
          failedJobs: 0,
          lastCompletedAt: '2026-04-10T16:20:00Z',
          lastError: null,
        },
      ],
      modules: [
        {
          moduleId: 'threads',
          enabled: true,
          version: 'diagnostic',
          status: 'ready',
          dependsOn: ['query-groups'],
          derivedTables: ['insight_threads', 'insight_thread_members'],
          lastRunId: 12,
          lastBuiltAt: '2026-04-10T16:25:00Z',
          lastInvalidatedAt: null,
          staleReason: null,
          notes: ['Thread merge uses query-family and reopen evidence.'],
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
        {
          id: 412,
          jobType: 'enrichment-plugin',
          pluginId: 'title-normalization',
          state: 'queued',
          historyId: 4,
          profileId: 'chrome:Default',
          url: 'https://example.com/docs',
          title: 'Docs',
          attempt: 1,
          createdAt: '2026-04-10T15:40:00Z',
          startedAt: null,
          finishedAt: null,
          updatedAt: '2026-04-10T15:40:00Z',
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
      notes: [
        'Browser preview mode shows a deterministic queue/runtime fixture.',
      ],
    }

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      runtimeSnapshot,
    )
    const retrySpy = vi
      .spyOn(backend, 'retryIntelligenceJob')
      .mockResolvedValue(runtimeSnapshot)
    const cancelSpy = vi
      .spyOn(backend, 'cancelIntelligenceJob')
      .mockResolvedValue(runtimeSnapshot)

    renderSurface(<InsightsPage />, {
      language: 'en',
      route: '/insights',
      snapshot,
    })

    expect(await screen.findByText(insightsT('overviewTitle'))).toBeVisible()
    expect(
      screen.getAllByRole('link', { name: settingsT('runtimeQueueTitle') })[0],
    ).toHaveAttribute('href', '/jobs')

    await user.click(
      screen.getByRole('button', { name: settingsT('retryRuntimeJob') }),
    )
    await waitFor(() => expect(retrySpy).toHaveBeenCalledWith(411))
    expect(cancelSpy).not.toHaveBeenCalled()
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

  test('renders insights snapshot, query ladders, and explainability flow', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const insightsT = createNamespaceTranslator('en', 'insights')

    enableAi(snapshot)

    const insightSnapshot: InsightSnapshot = {
      generatedAt: '2026-04-07T12:00:00Z',
      windowDays: 30,
      status: {
        ready: true,
        lastRunAt: '2026-04-07T12:00:00Z',
        runs: 3,
        cards: 1,
        topics: 1,
        threads: 1,
        queryGroups: 1,
        referencePages: 1,
        contentCoverage: 0.76,
        warning: null,
      },
      cards: [
        {
          cardId: 'card-1',
          kind: 'open-loop',
          title: 'Semantic recall drift',
          summary:
            'You revisited semantic recall material across several runs.',
          windowDays: 30,
          profileId: 'chrome:Default',
          score: 0.84,
          chromiumEnhanced: true,
          evidence: [
            {
              historyId: 41,
              profileId: 'chrome:Default',
              url: 'https://example.com/recall-drift',
              title: 'Recall drift notes',
              visitedAt: '2026-04-07T08:00:00Z',
              note: 'Frequently reopened across the last week.',
            },
          ],
        },
      ],
      queryGroups: [
        {
          queryGroupId: 'query-group-1',
          profileId: 'chrome:Default',
          threadId: 'thread-1',
          title: 'archive tool compare',
          rootQuery: 'archive tool',
          latestQuery: 'site:github.com archive tool',
          firstSeenAt: '2026-04-06T18:00:00Z',
          lastSeenAt: '2026-04-07T09:15:00Z',
          visitCount: 4,
          burstCount: 2,
          stepCount: 3,
          confidence: 0.81,
          evidenceTier: 'tier-a',
          chromiumEnhanced: true,
          steps: [
            'archive tool',
            'archive tool compare',
            'site:github.com archive tool',
          ],
          stages: ['broad', 'compare', 'site-restrict'],
          evidence: [
            {
              historyId: 43,
              profileId: 'chrome:Default',
              url: 'https://example.com/search-quality',
              title: 'Search quality',
              visitedAt: '2026-04-06T18:30:00Z',
              note: 'Query group anchor',
            },
          ],
        },
      ],
      topics: [
        {
          topicId: 'topic-1',
          label: 'Evidence contracts',
          profileScope: 'chrome:Default',
          windowDays: 30,
          firstSeenAt: '2026-03-10T10:00:00Z',
          lastSeenAt: '2026-04-07T08:00:00Z',
          visitCount: 5,
          revisitCount: 3,
          trendSlope: 0.7,
          burstScore: 0.6,
          evidence: [
            {
              historyId: 42,
              profileId: 'chrome:Default',
              url: 'https://example.com/evidence-contracts',
              title: 'Evidence contracts',
              visitedAt: '2026-04-07T09:15:00Z',
              note: 'Topic cluster anchor',
            },
          ],
        },
      ],
      threads: [
        {
          threadId: 'thread-1',
          profileId: 'chrome:Default',
          title: 'Semantic search quality',
          status: 'open',
          firstSeenAt: '2026-03-18T10:00:00Z',
          lastSeenAt: '2026-04-07T09:15:00Z',
          visitCount: 6,
          queryGroupCount: 1,
          reopenCount: 2,
          openLoopScore: 0.82,
          confidence: 0.78,
          evidenceTier: 'tier-a',
          dominantTopicId: 'topic-1',
          chromiumEnhanced: true,
          evidence: [
            {
              historyId: 43,
              profileId: 'chrome:Default',
              url: 'https://example.com/search-quality',
              title: 'Search quality',
              visitedAt: '2026-04-06T18:30:00Z',
              note: 'Supports the active thread.',
            },
          ],
        },
      ],
      queryLadders: [
        {
          queryGroupId: 'query-group-1',
          rootTerm: 'archive tool',
          profileId: 'chrome:Default',
          steps: [
            'archive tool',
            'archive tool compare',
            'site:github.com archive tool',
          ],
          stages: ['broad', 'compare', 'site-restrict'],
          count: 3,
          confidence: 0.81,
          evidenceTier: 'tier-a',
          chromiumOnly: true,
        },
      ],
      referencePages: [
        {
          referencePageId: 'reference-1',
          profileId: 'chrome:Default',
          url: 'https://example.com/search-quality',
          title: 'Search quality',
          domain: 'example.com',
          firstSeenAt: '2026-04-06T18:30:00Z',
          lastSeenAt: '2026-04-07T09:15:00Z',
          revisitCount: 2,
          crossDayRevisits: 1,
          queryGroupCount: 1,
          threadCount: 1,
          score: 1.8,
          evidenceTier: 'tier-b',
          evidence: [
            {
              historyId: 43,
              profileId: 'chrome:Default',
              url: 'https://example.com/search-quality',
              title: 'Search quality',
              visitedAt: '2026-04-06T18:30:00Z',
              note: 'Reference page evidence',
            },
          ],
        },
      ],
      sourceEffectiveness: [
        {
          sourceId: 'source-1',
          profileId: 'chrome:Default',
          domain: 'example.com',
          sourceRole: 'docs',
          queryGroupCount: 1,
          threadCount: 1,
          stableLandingCount: 1,
          referencePageCount: 1,
          reopenSupportCount: 1,
          effectivenessScore: 1.6,
          evidenceTier: 'tier-b',
          evidence: [
            {
              historyId: 43,
              profileId: 'chrome:Default',
              url: 'https://example.com/search-quality',
              title: 'Search quality',
              visitedAt: '2026-04-06T18:30:00Z',
              note: 'Source effectiveness evidence',
            },
          ],
        },
      ],
      templateSummaries: [
        {
          summaryId: 'summary-query-groups',
          kind: 'query-groups',
          title: 'Recent query refinement',
          body: 'Archive tool research narrowed into a GitHub-restricted query.',
          confidence: 0.81,
          profileId: 'chrome:Default',
          evidence: [
            {
              historyId: 43,
              profileId: 'chrome:Default',
              url: 'https://example.com/search-quality',
              title: 'Search quality',
              visitedAt: '2026-04-06T18:30:00Z',
              note: 'Template summary evidence',
            },
          ],
        },
      ],
      workflowMap: {
        profileId: 'chrome:Default',
        roles: [],
        edges: [],
        chromiumEnhanced: true,
      },
      profileFacets: [],
      canonical: {
        windowVisitCount: 6,
        windowUniqueDomains: 3,
        onThisDay: [
          {
            historyId: 41,
            profileId: 'chrome:Default',
            url: 'https://example.com/recall-drift',
            title: 'Recall drift notes',
            visitedAt: '2026-04-07T08:00:00Z',
            note: 'Frequently reopened across the last week.',
          },
        ],
        topDomains: [
          { domain: 'example.com', visitCount: 4 },
          { domain: 'sqlite.org', visitCount: 2 },
        ],
      },
      notes: ['Snapshot retained only evidence-backed summaries.'],
    }
    const explanation: InsightExplanation = {
      explanation:
        'This card stayed visible because the same evidence cluster reopened several times.',
      usedLlm: false,
      citations: insightSnapshot.cards[0].evidence,
      notes: ['Explanation derived from stored evidence only.'],
    }

    vi.spyOn(backend, 'loadInsights').mockResolvedValue(insightSnapshot)
    vi.spyOn(backend, 'explainInsight').mockResolvedValue(explanation)

    renderSurface(<InsightsPage />, {
      language: 'en',
      route: '/insights',
      snapshot,
    })

    expect(
      (await screen.findAllByText(insightsT('insightCards'))).length,
    ).toBeGreaterThan(0)
    expect(await screen.findByText(insightsT('queryGroups'))).toBeVisible()
    expect(await screen.findByText(insightsT('referencePages'))).toBeVisible()
    expect(
      await screen.findByText(insightsT('deterministicModules')),
    ).toBeVisible()
    expect(
      await screen.findByText(
        'Archive tool research narrowed into a GitHub-restricted query.',
      ),
    ).toBeVisible()
    expect(await screen.findByText('Semantic recall drift')).toBeVisible()
    expect(await screen.findByText(insightsT('queryEvolution'))).toBeVisible()
    expect(
      screen.getAllByText(
        'archive tool -> archive tool compare -> site:github.com archive tool',
      ).length,
    ).toBeGreaterThan(0)

    await user.click(
      screen.getAllByRole('button', { name: insightsT('explain') })[0],
    )

    expect(await screen.findByText(insightsT('explainability'))).toBeVisible()
    expect(
      await screen.findByText(
        'This card stayed visible because the same evidence cluster reopened several times.',
      ),
    ).toBeVisible()
  })

  test('keeps tiny coverage honest and compacts oversized query labels', async () => {
    const { snapshot } = await seedArchiveState()
    const insightsT = createNamespaceTranslator('en', 'insights')
    const longUrl =
      'https://elink2fb.mail.gethalfbaked.com/ss/c/u001.W7yY1DE5FLIFsRdzd8xlOFxIFmg-LnZyrakPeT0Kr4Akcsd_1nNyG4O-JzeKCfzCHx0L-Q-XaEfbRodxc4QPmYWsoxwYVKVdTuQECM3bSYEh-a_vxV99Ks-5wLaLiMaeY37qxnfpzNzmpXsxTvq_IPXz5HgP5iJZKdDkWjrUqimIY5PctDJFjhBu_zPRRgAg-dQNhu6BYaOyTuNS1qwtzKJBmoFK-4zVhhNl9iu13mgWvQtpcozXWzWK8THCZHcPxnrCdMIIBGV828w4PzF5yB_x3RR-pCoiYJSxvPV7JY5xBOnmLvfnv6r5NzjB43ogYyVW_4TBHTEm9p1YBYuXct8p7_gVGKhCalmuEO4l29RrpEepk_Zt8ZfJbgCHA2NjhaShq8P_ecCqoZkhZbPdK7hXCmS6vCQQRwbCqJMn7rA_6CzHp8cuu2dLM1WxsvcV1C5v3062OGqpliWy3p_B6ZqbcyyyAVhk3tEUvBYh8TUYjsNVDIOCyEAc0dkLDNoDaBX2UtwqDtlcFjtIHf7x8fmFqODD0nlBa94zHuY8gUv1PcmjeDQU_ibOvtlZSJutjF08QvPetF_tT9R1AG-pCUZYPqZPxqcyIhSnpMPA85mgUOD4Ssj59-ReWgfWvpXDFwIpILbfYzvkr6rd2QwWYR7mD2nmsRHmhGPLM4ar_Tdo8VRzL00zEoB-LSpWB_aZAa1FIBk3dWFYNrjwOwn-R6p4vxsbWIzCgtsPQEIjA8CHYwMpgFjDUz7hRPmSHIzKIE4t5jS9G3uUkMWr1m-_JqtApppz5izeYwpp8sYQOhIp02VjCNahZjKvFVAel4258O1sayVYHxyrT181U82dM1cobtRdxHUToaSFPC_2voJT-pw2NzWLA8-sy0gVhOtHAn0VGS_TSTRrJWoukJubqwM2cjTB7VvMhREDJND84eG0Lz0CeZue7gVy4dz-O1UMAy6_ghvYEslOJ8MYK71du2uV8w8d26maPf_m_b6HuuOddElBfgZjYFWwhpjNModDmZKXHamsBFP3Xt-6an2SyB51igaGpmn190V5Zvo8kwk/4po/kXT0fItVSwCLj4QagxK9mw/h35/h001.Asq1GllC9cmWzWIRdZ3K50d8fC8sPF_cT-N0wkOUle8'

    vi.spyOn(backend, 'loadInsights').mockResolvedValue({
      generatedAt: '2026-04-07T12:00:00Z',
      windowDays: 30,
      status: {
        ready: true,
        lastRunAt: '2026-04-07T12:00:00Z',
        runs: 1,
        cards: 1,
        topics: 0,
        threads: 0,
        queryGroups: 1,
        referencePages: 0,
        contentCoverage: 0.001,
        warning: null,
      },
      cards: [
        {
          cardId: 'card-long-url',
          kind: 'open-loop',
          title: `Open loop: ${longUrl}`,
          summary: 'This line is still active.',
          windowDays: 30,
          profileId: 'chrome:Default',
          score: 0.84,
          chromiumEnhanced: true,
          evidence: [],
        },
      ],
      queryGroups: [
        {
          queryGroupId: 'query-group-long-url',
          profileId: 'chrome:Default',
          threadId: null,
          title: longUrl,
          rootQuery: longUrl,
          latestQuery: longUrl,
          firstSeenAt: '2026-04-06T18:00:00Z',
          lastSeenAt: '2026-04-07T09:15:00Z',
          visitCount: 4,
          burstCount: 2,
          stepCount: 1,
          confidence: 0.81,
          evidenceTier: 'tier-a',
          chromiumEnhanced: true,
          steps: [longUrl],
          stages: ['broad'],
          evidence: [],
        },
      ],
      topics: [],
      threads: [],
      queryLadders: [],
      referencePages: [],
      sourceEffectiveness: [],
      templateSummaries: [],
      workflowMap: {
        profileId: 'chrome:Default',
        roles: [],
        edges: [],
        chromiumEnhanced: true,
      },
      profileFacets: [],
      canonical: {
        windowVisitCount: 1,
        windowUniqueDomains: 1,
        onThisDay: [],
        topDomains: [],
      },
      notes: [],
    })
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue({
      queue: {
        queued: 0,
        running: 0,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        lastActivityAt: null,
      },
      plugins: [],
      modules: [],
      recentJobs: [],
      notes: [],
    })

    renderSurface(<InsightsPage />, {
      language: 'en',
      route: '/insights',
      snapshot,
    })

    expect(
      await screen.findByText(formatInsightCoverage(0.001, 'en')),
    ).toBeVisible()
    expect(await screen.findByText(insightsT('queryGroups'))).toBeVisible()
    expect(screen.getByText(compactInsightText(longUrl, 88))).toBeVisible()
    expect(
      screen.getByText(compactInsightText(`Open loop: ${longUrl}`, 88)),
    ).toBeVisible()
    expect(screen.queryByText(longUrl)).not.toBeInTheDocument()
  })

  test('clears stale insights while a newly scoped snapshot is loading', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const insightsT = createNamespaceTranslator('en', 'insights')
    enableAi(snapshot)

    const chromeSnapshot: InsightSnapshot = {
      generatedAt: '2026-04-07T12:00:00Z',
      windowDays: 30,
      status: {
        ready: true,
        lastRunAt: '2026-04-07T12:00:00Z',
        runs: 1,
        cards: 1,
        topics: 0,
        threads: 0,
        queryGroups: 0,
        referencePages: 0,
        contentCoverage: 0.5,
        warning: null,
      },
      cards: [
        {
          cardId: 'card-chrome',
          kind: 'open-loop',
          title: 'Chrome-only summary',
          summary: 'Scoped to Chrome evidence.',
          windowDays: 30,
          profileId: 'chrome:Default',
          score: 0.7,
          chromiumEnhanced: true,
          evidence: [],
        },
      ],
      queryGroups: [],
      topics: [],
      threads: [],
      queryLadders: [],
      referencePages: [],
      sourceEffectiveness: [],
      templateSummaries: [],
      workflowMap: {
        profileId: 'chrome:Default',
        roles: [],
        edges: [],
        chromiumEnhanced: true,
      },
      profileFacets: [],
      canonical: {
        windowVisitCount: 1,
        windowUniqueDomains: 1,
        onThisDay: [],
        topDomains: [],
      },
      notes: [],
    }
    const firefoxSnapshot: InsightSnapshot = {
      ...chromeSnapshot,
      generatedAt: '2026-04-08T12:00:00Z',
      cards: [
        {
          ...chromeSnapshot.cards[0],
          cardId: 'card-firefox',
          title: 'Firefox summary',
          summary: 'Scoped to Firefox evidence.',
          profileId: 'firefox:default-release',
          chromiumEnhanced: false,
        },
      ],
      workflowMap: {
        profileId: 'firefox:default-release',
        roles: [],
        edges: [],
        chromiumEnhanced: false,
      },
    }

    let resolveNextSnapshot!: (value: InsightSnapshot) => void
    const nextSnapshotPromise = new Promise<InsightSnapshot>((resolve) => {
      resolveNextSnapshot = resolve
    })
    vi.spyOn(backend, 'loadInsights')
      .mockResolvedValueOnce(chromeSnapshot)
      .mockImplementationOnce(() => nextSnapshotPromise)

    render(
      <MemoryRouter initialEntries={['/insights']}>
        <I18nContext.Provider value={createI18nValue('en')}>
          <ProfileScopeProvider>
            <ScopeSwitcher nextProfileId="firefox:default-release" />
            <ShellDataContext.Provider value={createShellValue(snapshot)}>
              <InsightsPage />
            </ShellDataContext.Provider>
          </ProfileScopeProvider>
        </I18nContext.Provider>
      </MemoryRouter>,
    )

    expect(await screen.findByText('Chrome-only summary')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Switch scope' }))

    await waitFor(() =>
      expect(screen.queryByText('Chrome-only summary')).not.toBeInTheDocument(),
    )
    expect(
      await screen.findByLabelText(insightsT('loadingLabel')),
    ).toHaveAttribute('aria-busy', 'true')

    resolveNextSnapshot(firefoxSnapshot)

    expect(await screen.findByText('Firefox summary')).toBeVisible()
  })

  test('clears the previous explanation when a new explain request fails', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const insightsT = createNamespaceTranslator('en', 'insights')

    enableAi(snapshot)

    const insightSnapshot: InsightSnapshot = {
      generatedAt: '2026-04-07T12:00:00Z',
      windowDays: 30,
      status: {
        ready: true,
        lastRunAt: '2026-04-07T12:00:00Z',
        runs: 1,
        cards: 1,
        topics: 0,
        threads: 0,
        queryGroups: 0,
        referencePages: 1,
        contentCoverage: 0.4,
        warning: null,
      },
      cards: [
        {
          cardId: 'card-1',
          kind: 'open-loop',
          title: 'First explain target',
          summary: 'Evidence-backed card.',
          windowDays: 30,
          profileId: 'chrome:Default',
          score: 0.84,
          chromiumEnhanced: true,
          evidence: [],
        },
      ],
      queryGroups: [],
      topics: [],
      threads: [],
      queryLadders: [],
      referencePages: [
        {
          referencePageId: 'reference-1',
          profileId: 'chrome:Default',
          url: 'https://example.com/reference',
          title: 'Second explain target',
          domain: 'example.com',
          firstSeenAt: '2026-04-06T18:30:00Z',
          lastSeenAt: '2026-04-07T09:15:00Z',
          revisitCount: 2,
          crossDayRevisits: 1,
          queryGroupCount: 1,
          threadCount: 1,
          score: 1.8,
          evidenceTier: 'tier-b',
          evidence: [],
        },
      ],
      sourceEffectiveness: [],
      templateSummaries: [],
      workflowMap: {
        profileId: 'chrome:Default',
        roles: [],
        edges: [],
        chromiumEnhanced: true,
      },
      profileFacets: [],
      canonical: {
        windowVisitCount: 1,
        windowUniqueDomains: 1,
        onThisDay: [],
        topDomains: [],
      },
      notes: [],
    }
    vi.spyOn(backend, 'loadInsights').mockResolvedValue(insightSnapshot)
    vi.spyOn(backend, 'explainInsight')
      .mockResolvedValueOnce({
        explanation: 'First explanation',
        usedLlm: false,
        citations: [],
        notes: [],
      })
      .mockRejectedValueOnce(new Error('Explain failed'))

    renderSurface(<InsightsPage />, {
      language: 'en',
      route: '/insights',
      snapshot,
    })

    expect(await screen.findByText('First explain target')).toBeVisible()

    const explainButtons = await screen.findAllByRole('button', {
      name: insightsT('explain'),
    })
    await user.click(explainButtons[0])
    expect(await screen.findByText('First explanation')).toBeVisible()

    await user.click(explainButtons[1])

    await waitFor(() =>
      expect(screen.queryByText('First explanation')).not.toBeInTheDocument(),
    )
    expect(screen.getByText('Explain failed')).toBeVisible()
  })

  test('preserves profile scope on insights drilldowns back to explorer', async () => {
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')
    const today = new Date()
    const onThisDayVisitedAt = new Date(
      today.getFullYear(),
      today.getMonth(),
      today.getDate(),
      12,
      0,
      0,
    ).toISOString()

    const insightSnapshot: InsightSnapshot = {
      generatedAt: '2026-04-07T12:00:00Z',
      windowDays: 30,
      status: {
        ready: true,
        lastRunAt: '2026-04-07T12:00:00Z',
        runs: 1,
        cards: 0,
        topics: 1,
        threads: 0,
        queryGroups: 0,
        referencePages: 1,
        contentCoverage: 0.4,
        warning: null,
      },
      cards: [],
      queryGroups: [],
      topics: [
        {
          topicId: 'topic-1',
          label: 'Scoped topic',
          profileScope: 'chrome:Default',
          windowDays: 30,
          firstSeenAt: '2026-04-01T10:00:00Z',
          lastSeenAt: '2026-04-07T10:00:00Z',
          visitCount: 2,
          revisitCount: 1,
          trendSlope: 0.4,
          burstScore: 0.3,
          evidence: [],
        },
      ],
      threads: [],
      queryLadders: [],
      referencePages: [
        {
          referencePageId: 'reference-1',
          profileId: 'chrome:Default',
          url: 'https://example.com/reference',
          title: 'Scoped reference',
          domain: 'example.com',
          firstSeenAt: '2026-04-06T18:30:00Z',
          lastSeenAt: '2026-04-07T09:15:00Z',
          revisitCount: 2,
          crossDayRevisits: 1,
          queryGroupCount: 1,
          threadCount: 1,
          score: 1.8,
          evidenceTier: 'tier-b',
          evidence: [],
        },
      ],
      sourceEffectiveness: [
        {
          sourceId: 'source-1',
          profileId: 'chrome:Default',
          domain: 'example.com',
          sourceRole: 'docs',
          queryGroupCount: 1,
          threadCount: 1,
          stableLandingCount: 1,
          referencePageCount: 1,
          reopenSupportCount: 1,
          effectivenessScore: 1.4,
          evidenceTier: 'tier-b',
          evidence: [],
        },
      ],
      templateSummaries: [],
      workflowMap: {
        profileId: 'chrome:Default',
        roles: [],
        edges: [],
        chromiumEnhanced: true,
      },
      profileFacets: [],
      canonical: {
        windowVisitCount: 3,
        windowUniqueDomains: 1,
        onThisDay: [
          {
            historyId: 1,
            profileId: 'chrome:Default',
            url: 'https://example.com/on-this-day',
            title: 'Scoped on this day',
            visitedAt: onThisDayVisitedAt,
            note: 'Scoped evidence',
          },
        ],
        topDomains: [{ domain: 'example.com', visitCount: 3 }],
      },
      notes: [],
    }
    vi.spyOn(backend, 'loadInsights').mockResolvedValue(insightSnapshot)

    try {
      renderSurface(<InsightsPage />, {
        language: 'en',
        route: '/insights',
        snapshot,
      })

      expect(await screen.findByText('Scoped topic')).toBeVisible()
      expect(screen.queryByText('Scoped on this day')).not.toBeInTheDocument()

      const scopedExplorerSearches = screen
        .getAllByRole('link')
        .map((link) => link.getAttribute('href') ?? '')
        .filter((href) => href.startsWith('/explorer?'))
        .map((href) => new URLSearchParams(href.slice('/explorer?'.length)))

      const scopedEvidenceSearches = scopedExplorerSearches.filter(
        (params) => params.has('q') || params.has('domain'),
      )
      for (const params of scopedEvidenceSearches) {
        expect(params.get('profileId')).toBe('chrome:Default')
      }

      expect(
        scopedExplorerSearches.some(
          (params) =>
            params.get('profileId') === 'chrome:Default' &&
            params.get('q') === 'https://example.com/on-this-day',
        ),
      ).toBe(false)
      expect(
        scopedExplorerSearches.some(
          (params) =>
            params.get('profileId') === 'chrome:Default' &&
            params.get('domain') === 'example.com',
        ),
      ).toBe(true)
      expect(
        scopedExplorerSearches.some(
          (params) =>
            params.get('profileId') === 'chrome:Default' &&
            params.get('q') === 'Scoped topic',
        ),
      ).toBe(true)
      expect(
        scopedExplorerSearches.some(
          (params) =>
            params.get('profileId') === 'chrome:Default' &&
            params.get('q') === 'https://example.com/reference',
        ),
      ).toBe(true)
    } finally {
      window.localStorage.removeItem('pathkeep.profile-scope')
    }
  })

  test('renders storage analytics linked back to the latest audit run', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const insightsT = createNamespaceTranslator('en', 'insights')

    renderSurface(<InsightsPage />, {
      dashboard,
      language: 'en',
      route: '/insights',
      snapshot,
    })

    expect(await screen.findByText(insightsT('storageAnalytics'))).toBeVisible()
    expect(screen.getByText(insightsT('trackedStorage'))).toBeVisible()
    expect(screen.getByText(insightsT('reclaimableSpace'))).toBeVisible()
    expect(
      screen.getAllByText(insightsT('coreStorage')).length,
    ).toBeGreaterThan(0)

    const growthLink = screen.getByRole('link', {
      name: new RegExp(insightsT('latestRunGrowth'), 'i'),
    })
    expect(growthLink).toHaveAttribute(
      'href',
      expect.stringContaining('/audit?run='),
    )
  })
})
