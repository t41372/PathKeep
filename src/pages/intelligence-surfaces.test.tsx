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
  })

  test('renders localized dashboard intelligence and trust callouts', async () => {
    const { snapshot, dashboard } = await seedArchiveState()

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
    })
    snapshot.config.selectedProfileIds.push('safari:Personal')

    renderSurface(<DashboardPage />, {
      dashboard,
      language: 'zh-TW',
      route: '/',
      snapshot,
    })

    expect(await screen.findByText('智慧')).toBeVisible()
    expect(screen.getByRole('link', { name: '語義搜尋' })).toBeVisible()
    expect(screen.getAllByRole('link', { name: '檢查洞察' })).toHaveLength(2)
    expect(screen.getAllByRole('link', { name: '檢查安全狀態' })).toHaveLength(
      2,
    )
    expect(screen.getAllByRole('link', { name: '檢查匯入批次' })).toHaveLength(
      2,
    )
    expect(screen.getAllByRole('link', { name: '檢查洞察' })).toHaveLength(2)
  })

  test('renders dashboard on-this-day and periodic summary cards without leaking raw i18n keys', async () => {
    const { snapshot, dashboard } = await seedArchiveState()

    renderSurface(<DashboardPage />, {
      dashboard,
      language: 'en',
      route: '/',
      snapshot,
    })

    expect(await screen.findByText('ON THIS DAY')).toBeVisible()
    expect(
      await screen.findByText('SQLite inspection in browser developer tools'),
    ).toBeVisible()
    expect(await screen.findByText('PERIODIC SUMMARY')).toBeVisible()
    expect(
      screen.getByText(
        'Archive tooling is gaining momentum across docs, repo issues, and comparison pages.',
      ),
    ).toBeVisible()
    expect(screen.getByText('Disabled')).toBeVisible()
    expect(screen.queryByText('common.disabled')).not.toBeInTheDocument()
  })

  test('renders assistant queue state, provider probe, and answer citations', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()

    enableAi(snapshot)

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

    expect(await screen.findByText('运行上下文')).toBeVisible()
    expect(await screen.findByText('助手 · #77')).toBeVisible()

    await user.click(screen.getByRole('button', { name: '测试服务商' }))
    expect(await screen.findByText('服务商可达')).toBeVisible()

    const input =
      await screen.findByPlaceholderText('问一些关于你的浏览历史的问题…')
    await user.type(input, '总结最近的证据{enter}')

    expect(
      await screen.findByText('语义搜索质量趋势稳定，证据契约文档被多次访问。'),
    ).toBeVisible()
    expect(await screen.findByText('证据 · 1 条记录')).toBeVisible()
  })

  test('clears both explorer date bounds in a single interaction', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer?start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    const startInput = await screen.findByLabelText('Explorer start date')
    const endInput = await screen.findByLabelText('Explorer end date')

    expect(startInput).toHaveValue('2026-04-01')
    expect(endInput).toHaveValue('2026-04-07')

    await user.click(screen.getByRole('button', { name: 'Clear range' }))

    await waitFor(() => {
      expect(startInput).toHaveValue('')
      expect(endInput).toHaveValue('')
    })
  })

  test('renders insights snapshot and explainability flow', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()

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
      queryLadders: [],
      workflowMap: {
        profileId: 'chrome:Default',
        roles: [],
        edges: [],
        chromiumEnhanced: true,
      },
      profileFacets: [],
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

    expect(await screen.findByText('INSIGHT CARDS')).toBeVisible()
    expect(await screen.findByText('Semantic recall drift')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Explain' }))

    expect(await screen.findByText('EXPLAINABILITY')).toBeVisible()
    expect(
      await screen.findByText(
        'This card stayed visible because the same evidence cluster reopened several times.',
      ),
    ).toBeVisible()
  })

  test('renders storage analytics linked back to the latest audit run', async () => {
    const { snapshot, dashboard } = await seedArchiveState()

    renderSurface(<InsightsPage />, {
      dashboard,
      language: 'en',
      route: '/insights',
      snapshot,
    })

    expect(await screen.findByText('STORAGE ANALYTICS')).toBeVisible()
    expect(screen.getByText('Tracked storage')).toBeVisible()
    expect(screen.getByText('Reclaimable')).toBeVisible()
    expect(screen.getAllByText('Core archive').length).toBeGreaterThan(0)

    const growthLink = screen.getByRole('link', {
      name: /Latest audit-linked growth/i,
    })
    expect(growthLink).toHaveAttribute(
      'href',
      expect.stringContaining('/audit?run='),
    )
  })
})
