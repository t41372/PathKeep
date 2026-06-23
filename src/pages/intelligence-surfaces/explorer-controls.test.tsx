/**
 * @file explorer-controls.test.tsx
 * @description Protects the Explorer route shell gates and the route-owned semantic-runtime + prefetch contracts.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Preserve the loading / bootstrap-error / uninitialized / locked gate
 *   contracts at the Explorer route level (paper-layout neutral).
 * - Preserve the adjacent-page background prefetch contract driven by
 *   `useExplorerData`.
 * - Preserve the semantic-runtime action wiring exposed on the Explorer
 *   route when `mode=semantic` is in the URL.
 *
 * ## Non-Responsibilities
 * - Does not exercise the v0.2 ExplorerResultsPanel chrome: the date-range
 *   "Clear range" button, the inline profile combobox, the "filterKeyword"
 *   debounced input, the "Page number" spinbutton / Next/Last/Go paginator,
 *   the rows-per-page selector, and the legacy results skeleton no longer
 *   exist after Phase 4 retired `?layout=legacy`. Paper Browse replaces
 *   them with date-anchored DayNav controls on the contact sheet, an
 *   immediate-commit PaperSearchPanel query box, and the top-bar source
 *   picker — those surfaces are covered by their own tests.
 * - Does not own grouped Explorer view assertions.
 * - Does not broaden the shared harness or mutate unrelated route surfaces.
 *
 * ## Dependencies
 * - Depends on the shared Intelligence surface harness in `test-helpers.tsx`.
 * - Uses the shipped Explorer page as the integration surface.
 *
 * ## Performance Notes
 * - Reuses the shared seeded archive fixture to keep test setup bounded.
 */

import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { AiQueueStatus, HistoryQueryResponse } from '../../lib/types'
import { ExplorerPage } from '../explorer'
import {
  createShellValue,
  enableAi,
  renderSurface,
  resetIntelligenceSurfaceHarness,
  seedArchiveState,
} from './test-helpers'

vi.mock('../../lib/release-capabilities', () => ({
  deferredFeatureReleaseLabel: 'v0.3',
  optionalAiFeaturesAvailable: true,
  readableContentFetchAvailable: false,
}))

describe('intelligence surfaces', () => {
  beforeEach(() => {
    resetIntelligenceSurfaceHarness()
    window.localStorage.removeItem('pathkeep.explorer.page-size')
  })

  test('renders loading, bootstrap error, uninitialized, and locked shell gates before querying', async () => {
    const { snapshot } = await seedArchiveState()
    const commonT = createNamespaceTranslator('en', 'common')
    const explorerT = createNamespaceTranslator('en', 'explorer')

    const loadingRender = renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      shellValue: {
        ...createShellValue(snapshot),
        snapshot: null,
        loading: true,
      },
      snapshot,
    })

    expect(screen.getByLabelText(commonT('loadingExplorer'))).toHaveAttribute(
      'aria-busy',
      'true',
    )
    loadingRender.unmount()

    const errorRender = renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      shellValue: {
        ...createShellValue(snapshot),
        snapshot: null,
        loading: false,
        error: 'bootstrap transport failed',
      },
      snapshot,
    })

    expect(screen.getByText(explorerT('couldNotLoadTitle'))).toBeVisible()
    expect(screen.getByText('bootstrap transport failed')).toBeVisible()
    errorRender.unmount()

    const uninitializedSnapshot = structuredClone(snapshot)
    uninitializedSnapshot.config.initialized = false
    const uninitializedRender = renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot: uninitializedSnapshot,
    })

    expect(screen.getByText(explorerT('uninitializedTitle'))).toBeVisible()
    expect(
      screen.getByRole('link', { name: commonT('initializeFirst') }),
    ).toHaveAttribute('href', '/onboarding')
    uninitializedRender.unmount()

    const lockedSnapshot = structuredClone(snapshot)
    lockedSnapshot.archiveStatus.unlocked = false
    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot: lockedSnapshot,
    })

    expect(screen.getByText(explorerT('lockedTitle'))).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Check security' }),
    ).toHaveAttribute('href', '/security')
  })

  test('prefetches as many adjacent pages as the current config allows', async () => {
    const { snapshot } = await seedArchiveState()
    snapshot.config.explorerBackgroundPrefetchPages = 2
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) =>
        Promise.resolve({
          total: 250,
          page: query.page ?? 1,
          pageSize: query.limit ?? 50,
          pageCount: 5,
          hasPrevious: (query.page ?? 1) > 1,
          hasNext: (query.page ?? 1) < 5,
          nextCursor: null,
          items: [
            {
              id: (query.page ?? 1) * 10,
              profileId: 'chrome:Default',
              url: `https://example.com/page-${query.page ?? 1}`,
              title: `Page ${(query.page ?? 1).toString()}`,
              domain: 'example.com',
              visitedAt: '2026-04-17T10:00:00Z',
              visitTime: Date.parse('2026-04-17T10:00:00Z'),
              transition: null,
              favicon: null,
              sourceVisitId: (query.page ?? 1) * 10,
            },
          ],
        }),
      )

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ page: 2, limit: 50 }),
      ),
    )
    await waitFor(() =>
      expect(querySpy).toHaveBeenCalledWith(
        expect.objectContaining({ page: 3, limit: 50 }),
      ),
    )
  })

  test('routes semantic runtime actions through Explorer route-owned handlers', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    enableAi(snapshot)
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const queueStatus = aiQueueFixture()
    const shellValue = {
      ...createShellValue(snapshot),
      runtimeStatus: {
        aiQueue: queueStatus,
        intelligence: null,
        loading: false,
        error: null,
      },
      refreshAppData: vi.fn().mockResolvedValue(undefined),
      refreshRuntimeStatus: vi.fn().mockResolvedValue({
        aiQueue: queueStatus,
        intelligence: null,
        loading: false,
        error: null,
      }),
    }
    const buildIndexSpy = vi.spyOn(backend, 'buildAiIndex').mockResolvedValue({
      jobId: 77,
      runId: null,
      providerId: 'embed-local',
      model: 'nomic-embed-text',
      indexedItems: 12,
      updatedItems: 0,
      skippedItems: 0,
      removedItems: 0,
      lastIndexedAt: '2026-04-17T10:00:00Z',
      notes: [],
    })
    const runQueueSpy = vi
      .spyOn(backend, 'runAiQueueJobs')
      .mockResolvedValue(queueStatus)
    const replaySpy = vi
      .spyOn(backend, 'replayAiJob')
      .mockResolvedValue(queueStatus.recentJobs[0])
    const cancelSpy = vi.spyOn(backend, 'cancelAiJob').mockResolvedValue({
      ...queueStatus.recentJobs[0],
      state: 'cancelled',
    })
    const providerSpy = vi
      .spyOn(backend, 'testAiProviderConnection')
      .mockResolvedValue({
        providerId: 'embed-local',
        purpose: 'embedding',
        model: 'nomic-embed-text',
        ok: true,
        latencyMs: 42,
        capabilities: {
          supportsChat: false,
          supportsEmbeddings: true,
          supportsStreaming: false,
          supportsToolUse: false,
          supportsStructuredOutput: false,
        },
        errorCode: null,
        actionHint: null,
        retryHint: null,
        warnings: [],
        message: 'provider reachable',
      })
    vi.spyOn(backend, 'queryHistory').mockResolvedValue(
      historyPageFixture(1, 1),
    )
    vi.spyOn(backend, 'searchAiHistory').mockResolvedValue({
      total: 0,
      providerId: 'embed-local',
      model: 'nomic-embed-text',
      items: [],
      nextCursor: null,
      notes: [],
    })

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer?mode=semantic&q=sqlite',
      shellValue,
      snapshot,
    })

    expect(
      await screen.findByText(explorerT('providerQueueTitle')),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: explorerT('buildIndex') }),
    )
    await waitFor(() =>
      expect(buildIndexSpy).toHaveBeenCalledWith({
        providerId: 'embed-local',
        fullRebuild: false,
        clearOnly: false,
        limit: null,
      }),
    )

    await user.click(
      screen.getByRole('button', { name: explorerT('fullRebuild') }),
    )
    await waitFor(() =>
      expect(buildIndexSpy).toHaveBeenCalledWith({
        providerId: 'embed-local',
        fullRebuild: true,
        clearOnly: false,
        limit: null,
      }),
    )

    await user.click(
      screen.getByRole('button', { name: explorerT('clearIndex') }),
    )
    await waitFor(() =>
      expect(buildIndexSpy).toHaveBeenCalledWith({
        providerId: 'embed-local',
        fullRebuild: false,
        clearOnly: true,
        limit: null,
      }),
    )

    await user.click(
      screen.getByRole('button', { name: explorerT('refreshQueue') }),
    )
    await waitFor(() =>
      expect(shellValue.refreshRuntimeStatus).toHaveBeenCalled(),
    )

    await user.click(
      screen.getByRole('button', { name: explorerT('drainQueue') }),
    )
    await waitFor(() => expect(runQueueSpy).toHaveBeenCalledWith(2))

    await user.click(
      screen.getByRole('button', { name: explorerT('testProvider') }),
    )
    await waitFor(() =>
      expect(providerSpy).toHaveBeenCalledWith({
        providerId: 'embed-local',
        purpose: 'embedding',
      }),
    )
    expect(await screen.findByText('provider reachable')).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: explorerT('replayJob') }),
    )
    await waitFor(() => expect(replaySpy).toHaveBeenCalledWith(9001))

    await user.click(
      screen.getByRole('button', { name: explorerT('cancelJob') }),
    )
    await waitFor(() => expect(cancelSpy).toHaveBeenCalledWith(9001))
    expect(shellValue.refreshAppData).toHaveBeenCalled()
  })
})

function historyPageFixture(page: number, pageCount = 3): HistoryQueryResponse {
  return {
    total: pageCount * 50,
    page,
    pageSize: 50,
    pageCount,
    hasPrevious: page > 1,
    hasNext: page < pageCount,
    nextCursor: null,
    items: [
      {
        id: page,
        profileId: 'chrome:Default',
        url: `https://example.com/page-${page}`,
        title: `Page ${page}`,
        domain: 'example.com',
        visitedAt: '2026-04-17T10:00:00Z',
        visitTime: Date.parse('2026-04-17T10:00:00Z'),
        transition: null,
        favicon: null,
        sourceVisitId: page,
      },
    ],
  }
}

function aiQueueFixture(): AiQueueStatus {
  return {
    paused: false,
    concurrency: 2,
    queued: 1,
    running: 0,
    failed: 1,
    indexQueued: 1,
    indexRunning: 0,
    recentJobs: [
      {
        id: 9001,
        jobType: 'semantic-index',
        state: 'failed',
        priority: 10,
        attempt: 1,
        maxAttempts: 3,
        runId: 44,
        summary: 'Index failed on provider timeout.',
        queuedAt: '2026-04-17T09:30:00Z',
        availableAt: '2026-04-17T09:30:00Z',
        startedAt: '2026-04-17T09:31:00Z',
        finishedAt: '2026-04-17T09:32:00Z',
        heartbeatAt: null,
        errorCode: 'provider_timeout',
        errorMessage: 'provider timeout',
      },
    ],
  }
}
