/**
 * @file use-explorer-data.test.tsx
 * @description Hook-level coverage for Explorer backend loading, semantic recall, and action handlers.
 * @module pages/explorer/hooks
 *
 * ## Responsibilities
 * - Verify Explorer history loading selects a valid row and records recent searches.
 * - Protect semantic recall success/error states and queue/index/provider/export/open handlers.
 * - Keep backend action failures surfaced through route-owned state.
 *
 * ## Not responsible for
 * - Re-testing URL state parsing or Explorer panel rendering.
 * - Re-testing backend preview command implementations.
 *
 * ## Dependencies
 * - Mocks `waitForNextPaint` so hook tests focus on state transitions instead of browser frame timing.
 *
 * ## Performance notes
 * - Tests use small typed fixtures and avoid mounting the full Explorer route shell.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../../lib/backend-client'
import type {
  AiProviderConnectionTestReport,
  AiSearchResponse,
  ExportResult,
  HistoryQuery,
  HistoryQueryResponse,
} from '../../../lib/types'
import { useExplorerData } from './use-explorer-data'

vi.mock('../../../lib/wait-for-next-paint', () => ({
  waitForNextPaint: vi.fn(() => Promise.resolve()),
}))

describe('useExplorerData', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    window.localStorage.clear()
  })

  test('loads history results, selects the first row, and skips blocked history requests', async () => {
    const currentQuery = historyQueryFixture({ page: 1 })
    const historyResponse = historyResponseFixture({
      page: 1,
      pageCount: 3,
      hasNext: true,
    })
    const queryHistory = vi
      .spyOn(backend, 'queryHistory')
      .mockResolvedValue(historyResponse)
    const persistRecentSearch = vi.fn()
    const setRecentSearches = vi.fn()

    const options = createOptions({
      currentQuery,
      requestKey: historyRequestKey(currentQuery, 1),
      persistRecentSearch,
      setRecentSearches,
    })
    const { result, rerender } = renderHook(
      (props: ReturnType<typeof createOptions>) => useExplorerData(props),
      { initialProps: options },
    )

    await waitFor(() =>
      expect(result.current.queryState.results).not.toBeNull(),
    )
    expect(result.current.queryState.results?.items[0]?.id).toBe(101)
    expect(result.current.selectedId).toBe(101)
    expect(queryHistory).toHaveBeenCalledWith(currentQuery)
    expect(persistRecentSearch).toHaveBeenCalledWith(
      expect.objectContaining({
        q: 'sqlite',
        mode: 'keyword',
        view: 'time',
        sort: 'newest',
      }),
    )
    expect(setRecentSearches).toHaveBeenCalled()

    queryHistory.mockClear()
    rerender({
      ...options,
      archiveReady: false,
      requestKey: 'blocked-history',
    })

    await waitFor(() =>
      expect(result.current.queryState).toEqual({
        requestKey: 'blocked-history',
        results: null,
        error: null,
      }),
    )
    expect(queryHistory).not.toHaveBeenCalled()
  })

  test('prefetches adjacent history pages and reuses an inflight prefetch during navigation', async () => {
    const pageOneQuery = historyQueryFixture({ page: 1 })
    const pageTwoQuery = historyQueryFixture({ page: 2 })
    const pageOneResponse = historyResponseFixture({
      page: 1,
      pageCount: 3,
      hasNext: true,
    })
    const pageTwoResponse = historyResponseFixture({
      items: [
        {
          ...historyResponseFixture().items[0],
          id: 202,
          url: 'https://example.com/sqlite-page-2',
        },
      ],
      page: 2,
      pageCount: 3,
      hasPrevious: true,
      hasNext: true,
    })
    const deferredPageTwo =
      deferred<Awaited<ReturnType<typeof backend.queryHistory>>>()
    const queryHistory = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) => {
        if (query.page === 2) {
          return deferredPageTwo.promise
        }
        return Promise.resolve(pageOneResponse)
      })
    const options = createOptions({
      backgroundPrefetchPages: 1,
      currentQuery: pageOneQuery,
      requestKey: historyRequestKey(pageOneQuery, 1),
    })
    const { result, rerender } = renderHook(
      (props: ReturnType<typeof createOptions>) => useExplorerData(props),
      { initialProps: options },
    )

    await waitFor(() => {
      expect(result.current.queryState.results?.page).toBe(1)
    })
    await waitFor(() => {
      expect(queryHistory).toHaveBeenCalledWith(pageTwoQuery)
    })

    rerender({
      ...options,
      currentQuery: pageTwoQuery,
      requestKey: historyRequestKey(pageTwoQuery, 1),
    })
    await act(async () => {
      deferredPageTwo.resolve(pageTwoResponse)
      await deferredPageTwo.promise
    })

    await waitFor(() => {
      expect(result.current.queryState.results?.page).toBe(2)
    })
    expect(result.current.selectedId).toBe(202)
    expect(
      queryHistory.mock.calls.filter(([query]) => query.page === 2),
    ).toHaveLength(1)
  })

  test('reuses an inflight adjacent prefetch when a new load schedules the same page', async () => {
    const pageOneQuery = historyQueryFixture({ page: 1 })
    const pageTwoQuery = historyQueryFixture({ page: 2 })
    const pageOneResponse = historyResponseFixture({
      page: 1,
      pageCount: 3,
      hasNext: true,
    })
    const pageThreeResponse = historyResponseFixture({
      items: [
        {
          ...historyResponseFixture().items[0],
          id: 303,
          url: 'https://example.com/sqlite-page-3',
        },
      ],
      page: 3,
      pageCount: 3,
      hasPrevious: true,
      hasNext: false,
    })
    const deferredPageTwo =
      deferred<Awaited<ReturnType<typeof backend.queryHistory>>>()
    const queryHistory = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) => {
        if (query.page === 2) {
          return deferredPageTwo.promise
        }
        if (query.page === 3) {
          return Promise.resolve(pageThreeResponse)
        }
        return Promise.resolve(pageOneResponse)
      })
    const options = createOptions({
      backgroundPrefetchPages: 1,
      currentQuery: pageOneQuery,
      requestKey: historyRequestKey(pageOneQuery, 1),
    })
    const { result, rerender } = renderHook(
      (props: ReturnType<typeof createOptions>) => useExplorerData(props),
      { initialProps: options },
    )

    await waitFor(() => {
      expect(result.current.queryState.results?.page).toBe(1)
    })
    await waitFor(() => {
      expect(queryHistory).toHaveBeenCalledWith(pageTwoQuery)
    })

    const pageThreeQuery = historyQueryFixture({ page: 3 })
    rerender({
      ...options,
      currentQuery: pageThreeQuery,
      requestKey: historyRequestKey(pageThreeQuery, 1),
    })

    await waitFor(() => {
      expect(result.current.queryState.results?.page).toBe(3)
    })
    await waitFor(() => {
      expect(
        queryHistory.mock.calls.filter(([query]) => query.page === 2),
      ).toHaveLength(1)
    })

    await act(async () => {
      deferredPageTwo.resolve(
        historyResponseFixture({
          items: [
            {
              ...historyResponseFixture().items[0],
              id: 202,
              url: 'https://example.com/sqlite-page-2',
            },
          ],
          page: 2,
          pageCount: 3,
          hasPrevious: true,
          hasNext: true,
        }),
      )
      await deferredPageTwo.promise
    })
  })

  test('keeps loaded history when adjacent prefetch fails', async () => {
    const pageOneQuery = historyQueryFixture({ page: 1 })
    const pageTwoQuery = historyQueryFixture({ page: 2 })
    const queryHistory = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) => {
        if (query.page === 2) {
          return Promise.reject(new Error('prefetch failed'))
        }
        return Promise.resolve(
          historyResponseFixture({
            page: 1,
            pageCount: 3,
            hasNext: true,
          }),
        )
      })

    const { result } = renderHook(() =>
      useExplorerData(
        createOptions({
          backgroundPrefetchPages: 1,
          currentQuery: pageOneQuery,
          requestKey: historyRequestKey(pageOneQuery, 1),
        }),
      ),
    )

    await waitFor(() => {
      expect(result.current.queryState.results?.page).toBe(1)
    })
    await waitFor(() => {
      expect(queryHistory).toHaveBeenCalledWith(pageTwoQuery)
    })
    expect(result.current.queryState.error).toBeNull()
  })

  test('bounds the history page cache and refetches pages after eviction', async () => {
    const queryHistory = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) =>
        Promise.resolve(
          historyResponseFixture({
            items: [
              {
                ...historyResponseFixture().items[0],
                id: 100 + (query.page ?? 1),
                url: `https://example.com/page-${query.page ?? 1}`,
              },
            ],
            page: query.page ?? 1,
            pageCount: 8,
          }),
        ),
      )
    const { result, rerender } = renderHook(
      (props: ReturnType<typeof createOptions>) => useExplorerData(props),
      {
        initialProps: createOptions({
          currentQuery: historyQueryFixture({ page: 1 }),
          requestKey: historyRequestKey(historyQueryFixture({ page: 1 }), 1),
        }),
      },
    )

    for (let page = 1; page <= 7; page += 1) {
      const currentQuery = historyQueryFixture({ page })
      rerender(
        createOptions({
          currentQuery,
          requestKey: historyRequestKey(currentQuery, 1),
        }),
      )
      await waitFor(() => {
        expect(result.current.queryState.results?.page).toBe(page)
      })
    }

    const firstPageQuery = historyQueryFixture({ page: 1 })
    rerender(
      createOptions({
        currentQuery: firstPageQuery,
        requestKey: historyRequestKey(firstPageQuery, 1),
      }),
    )

    await waitFor(() => {
      expect(
        queryHistory.mock.calls.filter(([query]) => query.page === 1),
      ).toHaveLength(2)
    })
  })

  test('reports history fallback errors and copy feedback', async () => {
    vi.spyOn(backend, 'queryHistory').mockRejectedValue('not an Error')
    const { result } = renderHook(() => useExplorerData(createOptions()))

    await waitFor(() => {
      expect(result.current.queryState).toMatchObject({
        results: null,
        error: 'Query failed',
      })
    })

    await act(async () => {
      await result.current.handleCopyExportPath('/tmp/export.md')
    })

    expect(result.current.copyFeedback).toEqual({
      key: 'explorer:export:/tmp/export.md',
      tone: expect.any(String),
    })
  })

  test('keeps selected rows across cached and fresh history reloads', async () => {
    const queryHistory = vi.spyOn(backend, 'queryHistory').mockResolvedValue(
      historyResponseFixture({
        items: [
          {
            ...historyResponseFixture().items[0],
            id: 101,
          },
        ],
      }),
    )
    const query = historyQueryFixture({
      regexMode: true,
      sort: undefined,
    })
    const persistRecentSearch = vi.fn()
    const options = createOptions({
      currentQuery: query,
      persistRecentSearch,
      requestKey: historyRequestKey(query, 1),
    })
    const { result, rerender } = renderHook(
      (props: ReturnType<typeof createOptions>) => useExplorerData(props),
      { initialProps: options },
    )

    await waitFor(() => {
      expect(result.current.selectedId).toBe(101)
    })
    expect(persistRecentSearch).toHaveBeenCalledWith(
      expect.objectContaining({
        regex: '1',
        sort: 'newest',
      }),
    )

    act(() => {
      result.current.setSelectedId(101)
    })
    rerender({
      ...options,
      requestKey: `${historyRequestKey(query, 1)}:cached-rerun`,
    })

    await waitFor(() => {
      expect(result.current.queryState.requestKey).toBe(
        historyRequestKey(query, 1),
      )
    })
    expect(result.current.selectedId).toBe(101)

    rerender({
      ...options,
      cacheToken: 2,
      requestKey: historyRequestKey(query, 2),
    })

    await waitFor(() => {
      expect(queryHistory).toHaveBeenCalledTimes(2)
    })
    expect(result.current.selectedId).toBe(101)
  })

  test('keeps blocked history state stable when the same empty request repeats', async () => {
    const queryHistory = vi.spyOn(backend, 'queryHistory')
    const options = createOptions({
      archiveReady: false,
      requestKey: 'blocked-history',
    })
    const { result, rerender } = renderHook(
      (props: ReturnType<typeof createOptions>) => useExplorerData(props),
      { initialProps: options },
    )

    await waitFor(() => {
      expect(result.current.queryState).toEqual({
        requestKey: 'blocked-history',
        results: null,
        error: null,
      })
    })
    rerender(options)
    expect(result.current.queryState).toEqual({
      requestKey: 'blocked-history',
      results: null,
      error: null,
    })
    expect(queryHistory).not.toHaveBeenCalled()
  })

  test('keeps blocked history state stable when a non-time view repeats the same empty request', async () => {
    const queryHistory = vi.spyOn(backend, 'queryHistory')
    const options = createOptions({
      requestKey: 'blocked-history',
      view: 'session',
    })
    const { result, rerender } = renderHook(
      (props: ReturnType<typeof createOptions>) => useExplorerData(props),
      { initialProps: options },
    )

    await waitFor(() => {
      expect(result.current.queryState).toEqual({
        requestKey: 'blocked-history',
        results: null,
        error: null,
      })
    })
    rerender({
      ...options,
      historyBlockedByInvalidRegex: true,
    })

    expect(result.current.queryState).toEqual({
      requestKey: 'blocked-history',
      results: null,
      error: null,
    })
    expect(queryHistory).not.toHaveBeenCalled()
  })

  test('reuses cached empty history results without inventing a selected row', async () => {
    const query = historyQueryFixture({ page: 1 })
    const queryHistory = vi.spyOn(backend, 'queryHistory').mockResolvedValue(
      historyResponseFixture({
        items: [],
        total: 0,
      }),
    )
    const options = createOptions({
      currentQuery: query,
      requestKey: historyRequestKey(query, 1),
    })
    const { result, rerender } = renderHook(
      (props: ReturnType<typeof createOptions>) => useExplorerData(props),
      { initialProps: options },
    )

    await waitFor(() => {
      expect(result.current.queryState.results?.items).toEqual([])
    })
    expect(result.current.selectedId).toBeNull()

    rerender({
      ...options,
      requestKey: `${historyRequestKey(query, 1)}:cached-empty`,
    })

    await waitFor(() => {
      expect(result.current.queryState.requestKey).toBe(
        historyRequestKey(query, 1),
      )
    })
    expect(result.current.selectedId).toBeNull()
    expect(queryHistory).toHaveBeenCalledTimes(1)
  })

  test('ignores history and semantic completions after unmount', async () => {
    const historyDeferred =
      deferred<Awaited<ReturnType<typeof backend.queryHistory>>>()
    const semanticDeferred =
      deferred<Awaited<ReturnType<typeof backend.searchAiHistory>>>()
    vi.spyOn(backend, 'queryHistory').mockReturnValue(historyDeferred.promise)
    vi.spyOn(backend, 'searchAiHistory').mockReturnValue(
      semanticDeferred.promise,
    )

    const options = createOptions({
      mode: 'semantic',
      semanticQuery: {
        query: 'local recall',
        profileId: 'chrome:Default',
        domain: null,
        limit: 8,
        cursor: null,
      },
      semanticRequestKey: 'semantic-cancel',
    })
    const { result, unmount } = renderHook(() => useExplorerData(options))

    unmount()
    await act(async () => {
      historyDeferred.resolve(historyResponseFixture())
      semanticDeferred.resolve(semanticResponseFixture())
      await Promise.all([historyDeferred.promise, semanticDeferred.promise])
    })

    expect(result.current.queryState.results).toBeNull()
    expect(result.current.semanticState.results).toBeNull()
  })

  test('ignores history completions that arrive after the request is cancelled', async () => {
    const historyDeferred =
      deferred<Awaited<ReturnType<typeof backend.queryHistory>>>()
    const queryHistory = vi
      .spyOn(backend, 'queryHistory')
      .mockReturnValue(historyDeferred.promise)

    const { result, unmount } = renderHook(() =>
      useExplorerData(createOptions()),
    )

    await waitFor(() => {
      expect(queryHistory).toHaveBeenCalledTimes(1)
    })
    unmount()
    await act(async () => {
      historyDeferred.resolve(historyResponseFixture())
      await historyDeferred.promise
    })

    expect(result.current.queryState.results).toBeNull()
  })

  test('ignores rejected history and semantic requests after unmount', async () => {
    const historyDeferred =
      deferred<Awaited<ReturnType<typeof backend.queryHistory>>>()
    const semanticDeferred =
      deferred<Awaited<ReturnType<typeof backend.searchAiHistory>>>()
    vi.spyOn(backend, 'queryHistory').mockReturnValue(historyDeferred.promise)
    vi.spyOn(backend, 'searchAiHistory').mockReturnValue(
      semanticDeferred.promise,
    )

    const options = createOptions({
      mode: 'semantic',
      semanticQuery: {
        query: 'local recall',
        profileId: 'chrome:Default',
        domain: null,
        limit: 8,
        cursor: null,
      },
      semanticRequestKey: 'semantic-cancel-error',
    })
    const { result, unmount } = renderHook(() => useExplorerData(options))

    unmount()
    await act(async () => {
      historyDeferred.reject(new Error('history after unmount'))
      semanticDeferred.reject(new Error('semantic after unmount'))
      await Promise.allSettled([
        historyDeferred.promise,
        semanticDeferred.promise,
      ])
    })

    expect(result.current.queryState.error).toBeNull()
    expect(result.current.semanticState.error).toBeNull()
  })

  test('ignores history errors that arrive after the request is cancelled', async () => {
    const historyDeferred =
      deferred<Awaited<ReturnType<typeof backend.queryHistory>>>()
    const queryHistory = vi
      .spyOn(backend, 'queryHistory')
      .mockReturnValue(historyDeferred.promise)

    const { result, unmount } = renderHook(() =>
      useExplorerData(createOptions()),
    )

    await waitFor(() => {
      expect(queryHistory).toHaveBeenCalledTimes(1)
    })
    unmount()
    await act(async () => {
      historyDeferred.reject(new Error('late history failure'))
      await Promise.allSettled([historyDeferred.promise])
    })

    expect(result.current.queryState.error).toBeNull()
  })

  test('surfaces native history errors without replacing their message', async () => {
    vi.spyOn(backend, 'queryHistory').mockRejectedValue(
      new Error('history hard fail'),
    )
    const { result } = renderHook(() => useExplorerData(createOptions()))

    await waitFor(() => {
      expect(result.current.queryState.error).toBe('history hard fail')
    })
  })

  test('surfaces non-Error action fallbacks for Explorer handlers', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const refreshRuntimeStatus = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'buildAiIndex').mockRejectedValue('index fallback')
    vi.spyOn(backend, 'testAiProviderConnection').mockRejectedValue(
      new Error('provider failed'),
    )
    vi.spyOn(backend, 'exportHistory').mockRejectedValue('export fallback')
    vi.spyOn(backend, 'openExternalUrl').mockRejectedValue('visit fallback')

    const { result } = renderHook(() =>
      useExplorerData(
        createOptions({
          archiveReady: false,
          embeddingProviderId: 'provider-1',
          refreshAppData,
          refreshRuntimeStatus,
        }),
      ),
    )

    await act(async () => {
      await result.current.handleQueueAction('Retry', async () => {
        await Promise.resolve()
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- this regression protects fallback handling for non-Error queue failures from command adapters.
        throw 'queue fallback'
      })
    })
    expect(result.current.intelligenceError).toBe('Loading archive')

    await act(async () => {
      await result.current.handleIndexAction('Build index', {
        fullRebuild: false,
        clearOnly: false,
      })
    })
    expect(result.current.intelligenceError).toBe('Semantic recall degraded')

    await act(async () => {
      await result.current.handleProviderProbe()
    })
    expect(result.current.intelligenceError).toBe('provider failed')

    await act(async () => {
      await result.current.handleExport('markdown')
    })
    expect(result.current.actionError).toBe('Export failed')

    await act(async () => {
      await result.current.handleVisit('https://example.com')
    })
    expect(result.current.actionError).toBe('Visit failed')
  })

  test('loads semantic results and reports semantic failures', async () => {
    const semanticResponse = semanticResponseFixture()
    const searchAiHistory = vi
      .spyOn(backend, 'searchAiHistory')
      .mockResolvedValueOnce(semanticResponse)
      .mockRejectedValueOnce(new Error('semantic unavailable'))
    const options = createOptions({
      historyBlockedByInvalidRegex: true,
      mode: 'semantic',
      semanticQuery: {
        query: 'local recall',
        profileId: 'chrome:Default',
        domain: null,
        limit: 8,
        cursor: null,
      },
      semanticRequestKey: 'semantic-1',
    })
    const { result, rerender } = renderHook(
      (props: ReturnType<typeof createOptions>) => useExplorerData(props),
      { initialProps: options },
    )

    await waitFor(() =>
      expect(result.current.semanticState.results?.items[0]?.historyId).toBe(
        202,
      ),
    )
    expect(result.current.selectedId).toBe(202)
    expect(searchAiHistory).toHaveBeenCalledWith(options.semanticQuery)

    rerender({
      ...options,
      semanticRequestKey: 'semantic-2',
      semanticQuery: { ...options.semanticQuery, cursor: 'next' },
    })

    await waitFor(() =>
      expect(result.current.semanticState).toEqual({
        requestKey: 'semantic-2',
        results: null,
        error: 'semantic unavailable',
      }),
    )
  })

  test('falls back to the semantic recall degraded label for non-Error semantic failures', async () => {
    vi.spyOn(backend, 'searchAiHistory').mockRejectedValue('semantic fallback')
    const options = createOptions({
      mode: 'semantic',
      semanticQuery: {
        query: 'local recall',
        profileId: 'chrome:Default',
        domain: null,
        limit: 8,
        cursor: null,
      },
      semanticRequestKey: 'semantic-non-error',
    })
    const { result } = renderHook(() => useExplorerData(options))

    await waitFor(() => {
      expect(result.current.semanticState).toEqual({
        requestKey: 'semantic-non-error',
        results: null,
        error: 'Semantic recall degraded',
      })
    })
  })

  test('runs queue, index, provider, export, and visit handlers with success and failure state', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const refreshRuntimeStatus = vi.fn().mockResolvedValue(undefined)
    const buildAiIndex = vi.spyOn(backend, 'buildAiIndex').mockResolvedValue({
      jobId: 1,
      runId: null,
      providerId: 'provider-1',
      model: 'embedding-model',
      indexedItems: 0,
      updatedItems: 0,
      skippedItems: 0,
      removedItems: 0,
      lastIndexedAt: '2026-04-25T00:00:00.000Z',
      notes: ['queued'],
    })
    const testProvider = vi
      .spyOn(backend, 'testAiProviderConnection')
      .mockResolvedValue(providerProbeFixture())
    const exportHistory = vi
      .spyOn(backend, 'exportHistory')
      .mockResolvedValue(exportResultFixture())
    const openExternalUrl = vi
      .spyOn(backend, 'openExternalUrl')
      .mockResolvedValue('opened')

    const options = createOptions({
      archiveReady: false,
      embeddingProviderId: 'provider-1',
      refreshAppData,
      refreshRuntimeStatus,
    })
    const { result } = renderHook(() => useExplorerData(options))

    await act(async () => {
      await result.current.handleQueueAction('Retry', async () => {
        await Promise.resolve()
        return 'ok'
      })
    })
    expect(refreshAppData).toHaveBeenCalledTimes(1)
    expect(refreshRuntimeStatus).toHaveBeenCalledTimes(1)
    expect(result.current.queueAction).toBeNull()

    await act(async () => {
      await result.current.handleQueueAction('Retry', async () => {
        await Promise.resolve()
        throw new Error('queue failed')
      })
    })
    expect(result.current.intelligenceError).toBe('queue failed')

    await act(async () => {
      await result.current.handleIndexAction('Build index', {
        fullRebuild: true,
        clearOnly: false,
      })
    })
    expect(buildAiIndex).toHaveBeenCalledWith({
      providerId: 'provider-1',
      fullRebuild: true,
      clearOnly: false,
      limit: null,
    })
    expect(result.current.indexAction).toBeNull()

    buildAiIndex.mockRejectedValueOnce(new Error('index failed'))
    await act(async () => {
      await result.current.handleIndexAction('Clear index', {
        fullRebuild: false,
        clearOnly: true,
      })
    })
    expect(result.current.intelligenceError).toBe('index failed')

    await act(async () => {
      await result.current.handleProviderProbe()
    })
    expect(testProvider).toHaveBeenCalledWith({
      providerId: 'provider-1',
      purpose: 'embedding',
    })
    expect(result.current.providerProbe?.ok).toBe(true)

    testProvider.mockRejectedValueOnce('provider fallback')
    await act(async () => {
      await result.current.handleProviderProbe()
    })
    expect(result.current.intelligenceError).toBe('Semantic recall degraded')

    await act(async () => {
      await result.current.handleExport('markdown')
    })
    expect(exportHistory).toHaveBeenCalledWith({
      format: 'markdown',
      query: options.currentQuery,
    })
    expect(result.current.exportResult?.path).toBe('/tmp/export.md')

    exportHistory.mockRejectedValueOnce(new Error('export failed'))
    await act(async () => {
      await result.current.handleExport('jsonl')
    })
    expect(result.current.actionError).toBe('export failed')

    await act(async () => {
      await result.current.handleVisit('https://example.com')
    })
    expect(openExternalUrl).toHaveBeenCalledWith('https://example.com')

    openExternalUrl.mockRejectedValueOnce(new Error('open failed'))
    await act(async () => {
      await result.current.handleVisit('https://example.org')
    })
    expect(result.current.actionError).toBe('open failed')
  })

  test('skips provider probes when no embedding provider is configured', async () => {
    const testProvider = vi.spyOn(backend, 'testAiProviderConnection')
    const { result } = renderHook(() =>
      useExplorerData(
        createOptions({
          archiveReady: false,
          embeddingProviderId: null,
        }),
      ),
    )

    await act(async () => {
      await result.current.handleProviderProbe()
    })

    expect(testProvider).not.toHaveBeenCalled()
  })
})

function createOptions(
  overrides: Partial<Parameters<typeof useExplorerData>[0]> = {},
) {
  const currentQuery =
    overrides.currentQuery ?? historyQueryFixture({ page: 1 })
  return {
    archiveReady: true,
    backgroundPrefetchPages: 0,
    cacheToken: 1,
    currentQuery,
    embeddingProviderId: null,
    end: null,
    historyBlockedByInvalidRegex: false,
    labels: {
      exportFailed: 'Export failed',
      loadingArchive: 'Loading archive',
      queryFailedTitle: 'Query failed',
      semanticRecallDegradedTitle: 'Semantic recall degraded',
      visitFailed: 'Visit failed',
    },
    mode: 'keyword',
    view: 'time',
    persistRecentSearch: vi.fn(),
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn().mockResolvedValue(undefined),
    requestKey: historyRequestKey(currentQuery, 1),
    semanticQuery: {
      query: '',
      profileId: 'chrome:Default',
      domain: null,
      limit: 8,
      cursor: null,
    },
    semanticRequestKey: 'semantic-empty',
    setRecentSearches: vi.fn(),
    start: null,
    ...overrides,
  } satisfies Parameters<typeof useExplorerData>[0]
}

function historyRequestKey(query: HistoryQuery, cacheToken: number) {
  return JSON.stringify({
    currentQuery: query,
    refreshKey: cacheToken,
  })
}

function historyQueryFixture(
  overrides: Partial<HistoryQuery> = {},
): HistoryQuery {
  return {
    q: 'sqlite',
    profileId: 'chrome:Default',
    browserKind: 'chrome',
    domain: null,
    startTimeMs: null,
    endTimeMs: null,
    sort: 'newest',
    limit: 50,
    page: null,
    cursor: null,
    regexMode: false,
    ...overrides,
  }
}

function historyResponseFixture(
  overrides: Partial<HistoryQueryResponse> = {},
): HistoryQueryResponse {
  return {
    total: 1,
    items: [
      {
        id: 101,
        profileId: 'chrome:Default',
        url: 'https://example.com/sqlite',
        title: 'SQLite notes',
        domain: 'example.com',
        favicon: null,
        visitedAt: '2026-04-20T12:00:00.000Z',
        visitTime: Date.parse('2026-04-20T12:00:00.000Z'),
        durationMs: null,
        transition: null,
        sourceVisitId: 1,
        appId: null,
      },
    ],
    page: 1,
    pageSize: 50,
    pageCount: 1,
    hasPrevious: false,
    hasNext: false,
    nextCursor: null,
    ...overrides,
  }
}

function semanticResponseFixture(): AiSearchResponse {
  return {
    total: 1,
    providerId: 'provider-1',
    model: 'embedding-model',
    items: [
      {
        historyId: 202,
        profileId: 'chrome:Default',
        url: 'https://example.com/vector',
        title: 'Vector recall',
        domain: 'example.com',
        visitedAt: '2026-04-20T13:00:00.000Z',
        score: 0.92,
        matchReason: 'semantic',
      },
    ],
    notes: [],
    nextCursor: null,
  }
}

function providerProbeFixture(): AiProviderConnectionTestReport {
  return {
    providerId: 'provider-1',
    purpose: 'embedding',
    model: 'embedding-model',
    ok: true,
    latencyMs: 12,
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
    message: 'ok',
  }
}

function exportResultFixture(): ExportResult {
  return {
    format: 'markdown',
    path: '/tmp/export.md',
    count: 1,
  }
}

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  let reject: (error: unknown) => void = () => {}
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })

  return { promise, reject, resolve }
}
