/**
 * @file runtime-semantic-panel.test.tsx
 * @description Component coverage for Explorer runtime and semantic recall panels.
 * @module pages/explorer/panels
 *
 * ## Responsibilities
 * - Verify queue/index buttons, provider probe rows, job replay/cancel controls, and degraded runtime copy.
 * - Verify semantic recall prompt, loading, error, result, pagination, and empty states.
 * - Keep Explorer panel behavior covered without mounting the full route shell.
 *
 * ## Not responsible for
 * - Re-testing backend queue or semantic-search implementations.
 * - Re-testing global route URL state.
 *
 * ## Dependencies
 * - Uses MemoryRouter because both panels expose route links.
 *
 * ## Performance notes
 * - Panel fixtures stay bounded to a couple of rows/jobs so the tests remain cheap under full coverage.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import type {
  AiIndexStatus,
  AiProviderConfig,
  AiProviderConnectionTestReport,
  AiQueueJob,
  AiQueueStatus,
  AiSearchResponse,
} from '../../../lib/types'
import { ExplorerRuntimePanel } from './runtime-panel'
import { ExplorerSemanticPanel } from './semantic-panel'

const t = (key: string, params?: Record<string, string | number>) =>
  params ? `${key}:${JSON.stringify(params)}` : key

describe('ExplorerRuntimePanel', () => {
  test('renders queue controls, provider probe, job actions, and degraded/loading states', async () => {
    const user = userEvent.setup()
    const handlers = {
      onBuildIndex: vi.fn(),
      onCancelJob: vi.fn(),
      onClearIndex: vi.fn(),
      onDrainQueue: vi.fn(),
      onFullRebuild: vi.fn(),
      onReplayJob: vi.fn(),
      onRefreshQueue: vi.fn(),
      onTestProvider: vi.fn(),
    }
    const { rerender } = renderRuntimePanel({
      ...handlers,
      intelligenceError: 'runtime transport down',
      providerProbe: providerProbeFixture(),
      queueStatus: queueStatusFixture(),
    })

    expect(screen.getByText('runtime transport down')).toBeVisible()
    expect(screen.getByText('providerNeedsAttention')).toBeVisible()
    expect(screen.getByText('OpenAI / text-embedding-3-small')).toBeVisible()
    expect(screen.getByText('provider action')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'buildIndex' }))
    await user.click(screen.getByRole('button', { name: 'fullRebuild' }))
    await user.click(screen.getByRole('button', { name: 'clearIndex' }))
    await user.click(screen.getByRole('button', { name: 'refreshQueue' }))
    await user.click(screen.getByRole('button', { name: 'drainQueue' }))
    await user.click(screen.getByRole('button', { name: 'testProvider' }))
    await user.click(screen.getAllByRole('button', { name: 'replayJob' })[0])
    await user.click(screen.getAllByRole('button', { name: 'cancelJob' })[0])

    expect(handlers.onBuildIndex).toHaveBeenCalledTimes(1)
    expect(handlers.onFullRebuild).toHaveBeenCalledTimes(1)
    expect(handlers.onClearIndex).toHaveBeenCalledTimes(1)
    expect(handlers.onRefreshQueue).toHaveBeenCalledTimes(1)
    expect(handlers.onDrainQueue).toHaveBeenCalledTimes(1)
    expect(handlers.onTestProvider).toHaveBeenCalledTimes(1)
    expect(handlers.onReplayJob).toHaveBeenCalledWith(10)
    expect(handlers.onCancelJob).toHaveBeenCalledWith(10)

    rerender(
      <MemoryRouter>
        <ExplorerRuntimePanel
          {...handlers}
          aiMeta={{
            description: 'Semantic recall is paused.',
            label: 'Paused',
            tone: 'warning',
          }}
          embeddingProvider={null}
          explorerT={t}
          indexAction="building"
          intelligenceError={null}
          language="en"
          providerProbe={null}
          queueAction={null}
          queueStatus={null}
          snapshotAiStatus={aiStatusFixture()}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText('noEmbeddingProviderSelected')).toBeVisible()
    expect(screen.getByText('building')).toBeVisible()
    expect(screen.getByRole('button', { name: 'buildIndex' })).toBeDisabled()
    expect(screen.getByRole('link', { name: 'openSettings' })).toHaveAttribute(
      'href',
      '/settings',
    )
  })

  test('renders paused queues, queue-action loading labels, and summary fallbacks', () => {
    renderRuntimePanel({
      embeddingProvider: null,
      indexAction: null,
      queueAction: 'draining',
      queueStatus: {
        ...queueStatusFixture(),
        paused: true,
        recentJobs: [
          queueJobFixture({
            errorMessage: null,
            id: 13,
            state: 'cancelled',
            summary: null,
          }),
        ],
      },
    })

    expect(screen.getByText('queueStatePaused')).toBeVisible()
    expect(screen.getByText('draining')).toBeVisible()
    expect(screen.getByText('noEmbeddingProviderSelected')).toBeVisible()
    expect(screen.getByText('noJobSummary')).toBeVisible()
  })
})

describe('ExplorerSemanticPanel', () => {
  test('renders prompt, loading, error, results, pagination, and empty states', async () => {
    const user = userEvent.setup()
    const onSelectHistory = vi.fn()
    const onNextPage = vi.fn()
    const onPreviousPage = vi.fn()
    const { rerender } = renderSemanticPanel({
      onNextPage,
      onPreviousPage,
      onSelectHistory,
    })

    expect(screen.getByText('semanticPrompt')).toBeVisible()

    rerender(
      semanticPanelNode({
        mode: 'hybrid',
        semanticLoading: true,
        semanticQuery: { query: 'sqlite wal' },
        onNextPage,
        onPreviousPage,
        onSelectHistory,
      }),
    )
    expect(screen.getByText('rankingSemanticEvidence')).toBeVisible()
    expect(screen.getByText('1 / 3')).toBeVisible()

    rerender(
      semanticPanelNode({
        semanticError: 'semantic provider down',
        semanticQuery: { query: 'sqlite wal' },
        onNextPage,
        onPreviousPage,
        onSelectHistory,
      }),
    )
    expect(screen.getByText('semantic provider down')).toBeVisible()

    rerender(
      semanticPanelNode({
        semanticQuery: { query: 'sqlite wal' },
        semanticResults: searchResponseFixture(),
        semanticTrailLength: 1,
        onNextPage,
        onPreviousPage,
        onSelectHistory,
      }),
    )
    expect(screen.getByText('ranking note')).toBeVisible()
    expect(screen.getByText('SQLite WAL guide')).toBeVisible()
    expect(screen.getByText('example.com/docs')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'jumpToRecord' }))
    await user.click(
      screen.getByRole('button', { name: 'previousEvidencePage' }),
    )
    await user.click(screen.getByRole('button', { name: 'nextEvidencePage' }))

    expect(onSelectHistory).toHaveBeenCalledWith(42)
    expect(onPreviousPage).toHaveBeenCalledTimes(1)
    expect(onNextPage).toHaveBeenCalledWith('cursor-2')
    expect(screen.getByRole('link', { name: 'openEvidence' })).toHaveAttribute(
      'href',
      expect.stringContaining('/explorer?'),
    )
    expect(screen.getByRole('link', { name: 'askAssistant' })).toHaveAttribute(
      'href',
      expect.stringContaining('/assistant?'),
    )

    rerender(
      semanticPanelNode({
        semanticQuery: { query: 'sqlite wal' },
        semanticResults: { ...searchResponseFixture(), items: [], total: 0 },
        onNextPage,
        onPreviousPage,
        onSelectHistory,
      }),
    )
    expect(screen.getByText('noSemanticTitle')).toBeVisible()
  })

  test('renders semantic result fallbacks for untitled records and exhausted pages', () => {
    renderSemanticPanel({
      semanticQuery: { query: 'sqlite wal' },
      semanticResults: {
        ...searchResponseFixture(),
        items: [
          {
            ...searchResponseFixture().items[0],
            historyId: 99,
            score: 0.33,
            title: null,
            visitedAt: 'not-a-date',
          },
        ],
        nextCursor: null,
        notes: [],
      },
      semanticTrailLength: 0,
    })

    expect(screen.getAllByText('example.com/docs').length).toBeGreaterThan(0)
    expect(screen.getByText(/not-a-date/)).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'previousEvidencePage' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'nextEvidencePage' }),
    ).toBeDisabled()
  })
})

function renderRuntimePanel(
  overrides: Partial<Parameters<typeof ExplorerRuntimePanel>[0]> = {},
) {
  return render(
    <MemoryRouter>
      <ExplorerRuntimePanel
        aiMeta={{
          description: 'Semantic recall is healthy.',
          label: 'Ready',
          tone: 'success',
        }}
        embeddingProvider={embeddingProviderFixture()}
        explorerT={t}
        indexAction={null}
        intelligenceError={null}
        language="en"
        onBuildIndex={vi.fn()}
        onCancelJob={vi.fn()}
        onClearIndex={vi.fn()}
        onDrainQueue={vi.fn()}
        onFullRebuild={vi.fn()}
        onReplayJob={vi.fn()}
        onRefreshQueue={vi.fn()}
        onTestProvider={vi.fn()}
        providerProbe={null}
        queueAction={null}
        queueStatus={null}
        snapshotAiStatus={aiStatusFixture()}
        {...overrides}
      />
    </MemoryRouter>,
  )
}

function renderSemanticPanel(
  overrides: Partial<Parameters<typeof ExplorerSemanticPanel>[0]> = {},
) {
  return render(semanticPanelNode(overrides))
}

function semanticPanelNode(
  overrides: Partial<Parameters<typeof ExplorerSemanticPanel>[0]> = {},
) {
  return (
    <MemoryRouter>
      <ExplorerSemanticPanel
        explorerT={t}
        intelligenceT={t}
        language="en"
        mode="semantic"
        onNextPage={vi.fn()}
        onPreviousPage={vi.fn()}
        onSelectHistory={vi.fn()}
        semanticError={null}
        semanticLoading={false}
        semanticQuery={{ query: '' }}
        semanticResults={null}
        semanticTrailLength={0}
        {...overrides}
      />
    </MemoryRouter>
  )
}

function embeddingProviderFixture(): AiProviderConfig {
  return {
    id: 'embed-1',
    name: 'OpenAI',
    purpose: 'embedding',
    requestFormat: 'openai',
    enabled: true,
    baseUrl: 'https://api.openai.com/v1',
    apiKeySaved: true,
    defaultModel: 'text-embedding-3-small',
    modelCatalog: [],
    temperature: null,
    maxTokens: null,
    dimensions: 1536,
    notes: null,
  }
}

function aiStatusFixture(): AiIndexStatus {
  return {
    enabled: true,
    assistantEnabled: true,
    mcpEnabled: false,
    skillEnabled: false,
    state: 'ready',
    ready: true,
    indexedItems: 10,
    lastIndexedAt: '2026-04-25T10:00:00Z',
    llmProviderId: null,
    embeddingProviderId: 'embed-1',
    queuePaused: false,
    queueConcurrency: 1,
    queuedJobs: 3,
    runningJobs: 1,
    failedJobs: 1,
    recentJobs: [queueJobFixture({ id: 12, state: 'queued' })],
    semanticSidecarBytes: 10,
    semanticMetadataBytes: 5,
    estimatedEmbeddingTokens: 100,
    warning: null,
  }
}

function queueStatusFixture(): AiQueueStatus {
  return {
    paused: false,
    concurrency: 2,
    queued: 2,
    running: 1,
    failed: 1,
    recentJobs: [
      queueJobFixture({ id: 10, state: 'failed', summary: null }),
      queueJobFixture({ id: 11, state: 'running' }),
    ],
  }
}

function queueJobFixture(overrides: Partial<AiQueueJob> = {}): AiQueueJob {
  return {
    id: 10,
    jobType: 'embedding',
    state: 'failed',
    priority: 100,
    attempt: 1,
    maxAttempts: 3,
    runId: null,
    summary: 'Index stale rows',
    queuedAt: '2026-04-25T09:00:00Z',
    availableAt: '2026-04-25T09:00:00Z',
    startedAt: '2026-04-25T09:01:00Z',
    finishedAt: '2026-04-25T09:02:00Z',
    heartbeatAt: null,
    errorCode: 'provider-down',
    errorMessage: 'Provider unavailable',
    ...overrides,
  }
}

function providerProbeFixture(): AiProviderConnectionTestReport {
  return {
    providerId: 'embed-1',
    purpose: 'embedding',
    model: 'text-embedding-3-small',
    ok: false,
    latencyMs: 245,
    capabilities: {
      supportsChat: false,
      supportsEmbeddings: true,
      supportsStreaming: false,
      supportsToolUse: false,
      supportsStructuredOutput: false,
    },
    errorCode: 'provider-down',
    actionHint: 'provider action',
    retryHint: null,
    warnings: ['rate limited'],
    message: 'Provider unavailable',
  }
}

function searchResponseFixture(): AiSearchResponse {
  return {
    total: 1,
    providerId: 'embed-1',
    model: 'text-embedding-3-small',
    items: [
      {
        historyId: 42,
        profileId: 'chrome:Default',
        url: 'https://example.com/docs',
        title: 'SQLite WAL guide',
        domain: 'example.com',
        visitedAt: '2026-04-20T10:00:00Z',
        score: 0.91,
        matchReason: 'Matched semantic recall evidence.',
      },
    ],
    notes: ['ranking note'],
    nextCursor: 'cursor-2',
  }
}
