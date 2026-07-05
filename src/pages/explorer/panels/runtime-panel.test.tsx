/**
 * @file runtime-panel.test.tsx
 * @description Component coverage for the Explorer runtime (queue + index) panel.
 * @module pages/explorer/panels
 *
 * ## Responsibilities
 * - Verify queue/index buttons, provider probe rows, job replay/cancel controls, and degraded runtime copy.
 * - Keep Explorer runtime-panel behavior covered without mounting the full route shell.
 *
 * ## Not responsible for
 * - Re-testing backend queue implementations.
 * - Re-testing global route URL state.
 * - Semantic-result rendering — REACH-B retired the orphan `ExplorerSemanticPanel`;
 *   Smart results now render through `PaperSearchView`'s relevance layout (covered
 *   by paper-search-view / paper-search-panel tests).
 *
 * ## Dependencies
 * - Uses MemoryRouter because the panel exposes route links.
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
} from '../../../lib/types'
import { ExplorerRuntimePanel } from './runtime-panel'

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
    indexQueued: 2,
    indexRunning: 1,
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
