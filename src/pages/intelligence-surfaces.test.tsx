import type { ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../app/shell-data-context'
import { backend, backendTestHarness } from '../lib/backend'
import { I18nContext, type I18nContextValue } from '../lib/i18n/context'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from '../lib/i18n'
import { ProfileScopeProvider } from '../lib/profile-scope'
import type {
  AiProviderConnectionTestReport,
  AiQueueStatus,
  AppConfig,
  AppSnapshot,
  DashboardSnapshot,
  InsightExplanation,
  InsightSnapshot,
} from '../lib/types'
import { AssistantPage } from './assistant'
import { DashboardPage } from './dashboard'
import { ExplorerPage } from './explorer'
import { InsightsPage } from './insights'

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
  ai: {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt:
      'You are an audit-first history research assistant. Use the available browser history evidence before answering.',
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

function renderSurface(
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
        <ProfileScopeProvider>
          <ShellDataContext.Provider
            value={createShellValue(snapshot, dashboard)}
          >
            {ui}
          </ShellDataContext.Provider>
        </ProfileScopeProvider>
      </I18nContext.Provider>
    </MemoryRouter>,
  )
}

async function seedArchiveState() {
  await backend.initializeArchive(baseConfig, 'vault-passphrase')
  await backend.runBackupNow(false)

  const snapshot = structuredClone(await backend.getAppSnapshot())
  const dashboard = structuredClone(await backend.loadDashboardSnapshot())

  return { snapshot, dashboard }
}

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
        'Archive tooling is gaining momentum across docs, repo issues, and comparison pages.',
      ),
    ).toBeVisible()
    expect(screen.getByText(settingsT('disabled'))).toBeVisible()
    expect(screen.queryByText('settings.disabled')).not.toBeInTheDocument()
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
          reopenCount: 2,
          openLoopScore: 0.82,
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
          rootTerm: 'archive tool',
          profileId: 'chrome:Default',
          steps: [
            'archive tool',
            'archive tool compare',
            'site:github.com archive tool',
          ],
          stages: ['broad', 'compare', 'site-restrict'],
          count: 3,
          chromiumOnly: true,
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
    expect(await screen.findByText('Semantic recall drift')).toBeVisible()
    expect(await screen.findByText(insightsT('queryEvolution'))).toBeVisible()
    expect(
      screen.getByText(
        'archive tool -> archive tool compare -> site:github.com archive tool',
      ),
    ).toBeVisible()

    await user.click(screen.getByRole('button', { name: insightsT('explain') }))

    expect(await screen.findByText(insightsT('explainability'))).toBeVisible()
    expect(
      await screen.findByText(
        'This card stayed visible because the same evidence cluster reopened several times.',
      ),
    ).toBeVisible()
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
