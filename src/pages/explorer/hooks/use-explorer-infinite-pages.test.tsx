/**
 * @file use-explorer-infinite-pages.test.tsx
 * @description Hook tests for the accumulating-page buffer driving paper
 * Browse infinite scroll.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '@/lib/backend-client'
import type { HistoryQuery, HistoryQueryResponse } from '@/lib/types'
import {
  deriveExplorerInfinitePagination,
  useExplorerInfinitePages,
} from './use-explorer-infinite-pages'

const baseQuery: HistoryQuery = {
  q: null,
  profileId: 'chrome:Default',
  browserKind: null,
  domain: null,
  startTimeMs: null,
  endTimeMs: null,
  sort: 'newest',
  limit: 50,
  page: null,
  cursor: null,
  regexMode: false,
}

function makeHeadResponse(
  overrides: Partial<HistoryQueryResponse> = {},
): HistoryQueryResponse {
  return {
    total: 200,
    items: [],
    page: 1,
    pageSize: 50,
    pageCount: 4,
    hasPrevious: false,
    hasNext: true,
    nextCursor: null,
    ...overrides,
  }
}

function makeEntry(page: number, url = `https://example.com/page-${page}`) {
  return {
    id: page * 10,
    profileId: 'chrome:Default',
    url,
    title: `page ${page}`,
    domain: 'example.com',
    favicon: null,
    visitedAt: '2025-12-01T10:00:00Z',
    visitTime: 1733050800,
    sourceVisitId: 0,
  }
}

describe('deriveExplorerInfinitePagination', () => {
  test('refuses to advance without an active page-based head response', () => {
    expect(
      deriveExplorerInfinitePagination({
        accumulatedPages: 1,
        disabled: true,
        headResults: makeHeadResponse({ pageCount: 4 }),
        loadingMore: false,
      }),
    ).toEqual({ canLoadMore: false, capReached: false, canAdvance: false })
    expect(
      deriveExplorerInfinitePagination({
        accumulatedPages: 1,
        disabled: false,
        headResults: null,
        loadingMore: false,
      }),
    ).toEqual({ canLoadMore: false, capReached: false, canAdvance: false })
  })

  test('uses pageCount, not cursor hasNext, as the page-based load contract', () => {
    expect(
      deriveExplorerInfinitePagination({
        accumulatedPages: 1,
        disabled: false,
        headResults: makeHeadResponse({ pageCount: 1, hasNext: true }),
        loadingMore: false,
      }),
    ).toEqual({ canLoadMore: false, capReached: false, canAdvance: false })
  })

  test('keeps the load affordance visible while a page request is already running', () => {
    expect(
      deriveExplorerInfinitePagination({
        accumulatedPages: 2,
        disabled: false,
        headResults: makeHeadResponse({ pageCount: 4 }),
        loadingMore: true,
      }),
    ).toEqual({ canLoadMore: true, capReached: false, canAdvance: false })
  })

  test('reports the hard cap separately from a real end-of-archive', () => {
    expect(
      deriveExplorerInfinitePagination({
        accumulatedPages: 500,
        disabled: false,
        headResults: makeHeadResponse({ pageCount: 600 }),
        loadingMore: false,
      }),
    ).toEqual({ canLoadMore: false, capReached: true, canAdvance: false })
  })
})

describe('useExplorerInfinitePages', () => {
  beforeEach(() => {
    vi.spyOn(backend, 'queryHistory').mockReset()
  })
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('stays dormant when disabled (date-filtered / search surface)', () => {
    const querySpy = vi.spyOn(backend, 'queryHistory')
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: makeHeadResponse(),
        disabled: true,
        cacheToken: 1,
      }),
    )
    expect(result.current.canLoadMore).toBe(false)
    expect(result.current.extraItems).toEqual([])
    expect(querySpy).not.toHaveBeenCalled()
  })

  test('stays dormant until the first page response is available', () => {
    const querySpy = vi.spyOn(backend, 'queryHistory')
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: null,
        disabled: false,
        cacheToken: 1,
      }),
    )

    act(() => result.current.loadMore())

    expect(result.current.canLoadMore).toBe(false)
    expect(result.current.loadedPageCount).toBe(1)
    expect(querySpy).not.toHaveBeenCalled()
  })

  test('fetches page 2 on loadMore and appends entries to extraItems', async () => {
    vi.spyOn(backend, 'queryHistory').mockResolvedValue(
      makeHeadResponse({
        page: 2,
        items: [
          {
            id: 51,
            profileId: 'chrome:Default',
            url: 'https://example.com/older',
            title: 'older entry',
            domain: 'example.com',
            favicon: null,
            visitedAt: '2025-12-01T10:00:00Z',
            visitTime: 1733050800,
            sourceVisitId: 0,
          },
        ],
      }),
    )
    // Stable reference: the hook's effect uses headResults as a dep; the
    // previous test was passing a fresh object on every render which made
    // the reset effect re-fire and clobber the accumulated state.
    const head = makeHeadResponse()
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
      }),
    )
    expect(result.current.canLoadMore).toBe(true)
    act(() => result.current.loadMore())
    await waitFor(() =>
      expect(result.current.extraItems.length).toBeGreaterThan(0),
    )
    expect(result.current.extraItems[0]?.url).toBe('https://example.com/older')
    expect(result.current.loadedPageCount).toBe(2)
  })

  test('rolls back accumulatedPages when the fetch rejects so loadMore can retry', async () => {
    vi.spyOn(backend, 'queryHistory').mockRejectedValueOnce(
      new Error('archive locked'),
    )
    const head = makeHeadResponse()
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
      }),
    )
    act(() => result.current.loadMore())
    await waitFor(() => expect(result.current.error).not.toBeNull())
    // Buffer rolled back so canLoadMore stays truthy for retry.
    expect(result.current.loadedPageCount).toBe(1)
    expect(result.current.canLoadMore).toBe(true)
  })

  test('clears loadingMore when a filter change cancels an in-flight fetch', async () => {
    // Regression: user reported "filter on Browse, scroll to bottom →
    // loading animation spins forever". Reset effect ran setLoadingMore
    // (false) and the same render's fetch effect then queued
    // setLoadingMore(true) for the now-cancelled fetch; the .finally
    // skipped its clear because cancelled=true, so the flag stuck.
    let resolveFetch: (response: ReturnType<typeof makeHeadResponse>) => void
    const slow = new Promise<ReturnType<typeof makeHeadResponse>>((resolve) => {
      resolveFetch = resolve
    })
    vi.spyOn(backend, 'queryHistory').mockReturnValue(slow)
    // Stable headResults so the fetch effect doesn't re-run on every
    // render of the test wrapper.
    const head = makeHeadResponse()
    const { result, rerender } = renderHook(
      ({ q }: { q: string | null }) =>
        useExplorerInfinitePages({
          query: { ...baseQuery, q },
          headResults: head,
          disabled: false,
          cacheToken: 1,
        }),
      { initialProps: { q: null as string | null } },
    )
    act(() => result.current.loadMore())
    // The foreground fetch is in flight; loadingMore should be true.
    await waitFor(() => expect(result.current.loadingMore).toBe(true))
    // Filter change → query signature changes → reset + cancellation.
    rerender({ q: 'github.com' })
    expect(result.current.loadingMore).toBe(false)
    // Resolve the now-cancelled fetch; it must not flip loadingMore back.
    resolveFetch!(makeHeadResponse({ page: 2, items: [] }))
    await waitFor(() => expect(result.current.loadingMore).toBe(false))
  })

  test('resets accumulated state when the query signature changes', async () => {
    vi.spyOn(backend, 'queryHistory').mockResolvedValue(
      makeHeadResponse({ page: 2, items: [] }),
    )
    const { result, rerender } = renderHook(
      ({ q }: { q: string | null }) =>
        useExplorerInfinitePages({
          query: { ...baseQuery, q },
          headResults: makeHeadResponse(),
          disabled: false,
          cacheToken: 1,
        }),
      { initialProps: { q: null as string | null } },
    )
    act(() => result.current.loadMore())
    await waitFor(() => expect(result.current.loadedPageCount).toBe(2))
    rerender({ q: 'newer-term' })
    expect(result.current.loadedPageCount).toBe(1)
    expect(result.current.extraItems).toEqual([])
  })

  test('keeps accumulated pages when the same reset key renders again', async () => {
    vi.spyOn(backend, 'queryHistory').mockResolvedValue(
      makeHeadResponse({
        page: 2,
        items: [makeEntry(2, 'https://example.com/still-buffered')],
      }),
    )
    const head = makeHeadResponse()
    const props = {
      query: baseQuery,
      headResults: head,
      disabled: false,
      cacheToken: 1,
    }
    const { result, rerender } = renderHook(
      (options: Parameters<typeof useExplorerInfinitePages>[0]) =>
        useExplorerInfinitePages(options),
      { initialProps: props },
    )

    act(() => result.current.loadMore())
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toEqual([
        'https://example.com/still-buffered',
      ]),
    )
    rerender(props)

    expect(result.current.loadedPageCount).toBe(2)
    expect(result.current.extraItems.map((item) => item.url)).toEqual([
      'https://example.com/still-buffered',
    ])
  })

  test('resets accumulated state when the cache token changes without a query change', async () => {
    vi.spyOn(backend, 'queryHistory').mockResolvedValue(
      makeHeadResponse({
        page: 2,
        items: [makeEntry(2, 'https://example.com/before-refresh')],
      }),
    )
    const head = makeHeadResponse()
    const { result, rerender } = renderHook(
      ({ cacheToken }: { cacheToken: number }) =>
        useExplorerInfinitePages({
          query: baseQuery,
          headResults: head,
          disabled: false,
          cacheToken,
        }),
      { initialProps: { cacheToken: 1 } },
    )
    act(() => result.current.loadMore())
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toEqual([
        'https://example.com/before-refresh',
      ]),
    )
    rerender({ cacheToken: 2 })
    expect(result.current.loadedPageCount).toBe(1)
    expect(result.current.extraItems).toEqual([])
    expect(result.current.error).toBeNull()
    expect(result.current.loadingMore).toBe(false)
  })

  test('does not issue a duplicate foreground request while a page is loading', async () => {
    let resolvePage2: ((response: HistoryQueryResponse) => void) | undefined
    const slowPage2 = new Promise<HistoryQueryResponse>((resolve) => {
      resolvePage2 = resolve
    })
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) => {
        const page = req.page ?? 1
        if (page === 2) return slowPage2
        return Promise.resolve(
          makeHeadResponse({ page, items: [makeEntry(page)] }),
        )
      })
    const head = makeHeadResponse({ pageCount: 4, hasNext: true })
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
      }),
    )
    act(() => result.current.loadMore())
    await waitFor(() => expect(result.current.loadingMore).toBe(true))

    act(() => result.current.loadMore())
    expect(
      querySpy.mock.calls.filter((call) => call[0].page === 2),
    ).toHaveLength(1)

    await act(async () => {
      resolvePage2?.(makeHeadResponse({ page: 2, items: [makeEntry(2)] }))
      await slowPage2
    })
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toEqual([
        'https://example.com/page-2',
      ]),
    )
  })

  test('does not prefetch a page beyond the reported pageCount', async () => {
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) =>
        Promise.resolve(
          makeHeadResponse({
            page: req.page ?? 1,
            items: [makeEntry(req.page ?? 1)],
          }),
        ),
      )
    const head = makeHeadResponse({ pageCount: 2, hasNext: false })
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
      }),
    )
    act(() => result.current.loadMore())
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toEqual([
        'https://example.com/page-2',
      ]),
    )
    await Promise.resolve()
    expect(querySpy.mock.calls.map((call) => call[0].page)).toEqual([2])
    expect(result.current.canLoadMore).toBe(false)
  })

  test('keeps background prefetch failures silent when the foreground page succeeds', async () => {
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) => {
        const page = req.page ?? 1
        if (page === 3) return Promise.reject(new Error('background timeout'))
        return Promise.resolve(
          makeHeadResponse({ page, items: [makeEntry(page)] }),
        )
      })
    const head = makeHeadResponse({ pageCount: 4, hasNext: true })
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
      }),
    )
    act(() => result.current.loadMore())
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toEqual([
        'https://example.com/page-2',
      ]),
    )
    await waitFor(() =>
      expect(querySpy.mock.calls.map((call) => call[0].page)).toContain(3),
    )
    expect(result.current.error).toBeNull()
    expect(result.current.loadedPageCount).toBe(2)
    expect(result.current.loadingMore).toBe(false)
  })

  test('drops a rejected page response after the request is cancelled', async () => {
    const slowPage2 = deferred<HistoryQueryResponse>()
    vi.spyOn(backend, 'queryHistory').mockReturnValue(slowPage2.promise)
    const head = makeHeadResponse()
    const { result, rerender } = renderHook(
      ({ q }: { q: string | null }) =>
        useExplorerInfinitePages({
          query: { ...baseQuery, q },
          headResults: head,
          disabled: false,
          cacheToken: 1,
        }),
      { initialProps: { q: null as string | null } },
    )

    act(() => result.current.loadMore())
    await waitFor(() => expect(result.current.loadingMore).toBe(true))
    rerender({ q: 'changed' })
    await act(async () => {
      slowPage2.reject(new Error('late page failure'))
      await slowPage2.promise.catch(() => undefined)
    })

    expect(result.current.error).toBeNull()
    expect(result.current.loadingMore).toBe(false)
  })

  test('downward prefetch does not warm a second page beyond the reported pageCount', async () => {
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) =>
        Promise.resolve(
          makeHeadResponse({
            page: req.page ?? 1,
            items: [makeEntry(req.page ?? 1)],
          }),
        ),
      )
    const head = makeHeadResponse({ pageCount: 3, hasNext: false })
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
        scrollDirection: 'down',
      }),
    )
    act(() => result.current.loadMore())
    await waitFor(() => {
      const queriedPages = querySpy.mock.calls.map((call) => call[0].page)
      expect(queriedPages).toContain(2)
      expect(queriedPages).toContain(3)
    })
    await Promise.resolve()
    expect(querySpy.mock.calls.map((call) => call[0].page)).toEqual([2, 3])
  })

  test('prefetches one page ahead so the next loadMore is instant', async () => {
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) =>
        Promise.resolve(
          makeHeadResponse({
            page: req.page ?? 1,
            items: [
              {
                id: (req.page ?? 1) * 10,
                profileId: 'chrome:Default',
                url: `https://example.com/page-${req.page ?? 1}`,
                title: `page ${req.page ?? 1}`,
                domain: 'example.com',
                favicon: null,
                visitedAt: '2025-12-01T10:00:00Z',
                visitTime: 1733050800,
                sourceVisitId: 0,
              },
            ],
          }),
        ),
      )
    const head = makeHeadResponse({ pageCount: 4, hasNext: true })
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
      }),
    )
    // First loadMore brings page 2 to the surface and warms page 3 in
    // the background.
    act(() => result.current.loadMore())
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toEqual([
        'https://example.com/page-2',
      ]),
    )
    await waitFor(() => {
      const queriedPages = querySpy.mock.calls.map((call) => call[0].page)
      expect(queriedPages).toContain(2)
      expect(queriedPages).toContain(3)
    })
    // Background prefetch must not advance the user-facing loadedPageCount
    // — the contact sheet's "Loaded N of M" copy stays honest.
    expect(result.current.loadedPageCount).toBe(2)
    // Now the second loadMore should surface page 3 immediately without
    // issuing a new query (it was already buffered).
    const callsBeforeNextLoadMore = querySpy.mock.calls.length
    act(() => result.current.loadMore())
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toEqual([
        'https://example.com/page-2',
        'https://example.com/page-3',
      ]),
    )
    expect(result.current.loadedPageCount).toBe(3)
    // The second loadMore must have used the prefetched buffer, not a
    // fresh foreground query.
    const newForegroundCalls = querySpy.mock.calls
      .slice(callsBeforeNextLoadMore)
      .filter((call) => call[0].page === 3)
    expect(newForegroundCalls).toEqual([])
  })

  test('does not load past the head pageCount', () => {
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: makeHeadResponse({ pageCount: 1, hasNext: false }),
        disabled: false,
        cacheToken: 1,
      }),
    )
    expect(result.current.canLoadMore).toBe(false)
    act(() => result.current.loadMore())
    expect(result.current.loadedPageCount).toBe(1)
  })

  test('warms a second page ahead when scrollDirection="down" (BROWSE-VIRT directional prefetch)', async () => {
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) =>
        Promise.resolve(
          makeHeadResponse({
            page: req.page ?? 1,
            items: [
              {
                id: (req.page ?? 1) * 10,
                profileId: 'chrome:Default',
                url: `https://example.com/page-${req.page ?? 1}`,
                title: `page ${req.page ?? 1}`,
                domain: 'example.com',
                favicon: null,
                visitedAt: '2025-12-01T10:00:00Z',
                visitTime: 1733050800,
                sourceVisitId: 0,
              },
            ],
          }),
        ),
      )
    const head = makeHeadResponse({ pageCount: 6, hasNext: true })
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
        scrollDirection: 'down',
      }),
    )
    act(() => result.current.loadMore())
    // foreground page 2 + background page 3 (always) + background
    // page 4 (because direction is "down"). The waitFor settles after
    // all three have been requested.
    await waitFor(() => {
      const queriedPages = querySpy.mock.calls.map((call) => call[0].page)
      expect(queriedPages).toContain(2)
      expect(queriedPages).toContain(3)
      expect(queriedPages).toContain(4)
    })
  })

  test('keeps directional prefetch failures silent', async () => {
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) => {
        const page = req.page ?? 1
        if (page === 4) return Promise.reject(new Error('directional timeout'))
        return Promise.resolve(
          makeHeadResponse({
            page,
            items: [makeEntry(page)],
          }),
        )
      })
    const head = makeHeadResponse({ pageCount: 6, hasNext: true })
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
        scrollDirection: 'down',
      }),
    )

    act(() => result.current.loadMore())
    await waitFor(() => {
      expect(querySpy.mock.calls.map((call) => call[0].page)).toEqual(
        expect.arrayContaining([2, 3, 4]),
      )
    })
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toContain(
        'https://example.com/page-2',
      ),
    )

    expect(result.current.error).toBeNull()
  })

  test('does not re-fetch a directional prefetch page when it becomes the next warm target', async () => {
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) =>
        Promise.resolve(
          makeHeadResponse({
            page: req.page ?? 1,
            items: [makeEntry(req.page ?? 1)],
          }),
        ),
      )
    const head = makeHeadResponse({ pageCount: 6, hasNext: true })
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
        scrollDirection: 'down',
      }),
    )

    act(() => result.current.loadMore())
    await waitFor(() => {
      expect(querySpy.mock.calls.map((call) => call[0].page)).toEqual(
        expect.arrayContaining([2, 3, 4]),
      )
    })
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toContain(
        'https://example.com/page-2',
      ),
    )

    act(() => result.current.loadMore())
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toContain(
        'https://example.com/page-3',
      ),
    )

    expect(
      querySpy.mock.calls.filter((call) => call[0].page === 4),
    ).toHaveLength(1)
  })

  test('idle scroll direction keeps the original single-page prefetch (no +2 warmup)', async () => {
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) =>
        Promise.resolve(
          makeHeadResponse({
            page: req.page ?? 1,
            items: [
              {
                id: (req.page ?? 1) * 10,
                profileId: 'chrome:Default',
                url: `u${req.page ?? 1}`,
                title: 't',
                domain: 'example.com',
                favicon: null,
                visitedAt: '2025-12-01T10:00:00Z',
                visitTime: 1733050800,
                sourceVisitId: 0,
              },
            ],
          }),
        ),
      )
    const head = makeHeadResponse({ pageCount: 6, hasNext: true })
    const { result } = renderHook(() =>
      useExplorerInfinitePages({
        query: baseQuery,
        headResults: head,
        disabled: false,
        cacheToken: 1,
        scrollDirection: 'idle',
      }),
    )
    act(() => result.current.loadMore())
    await waitFor(() => {
      const queriedPages = querySpy.mock.calls.map((call) => call[0].page)
      expect(queriedPages).toContain(2)
      expect(queriedPages).toContain(3)
    })
    // Give a microtask for any deferred +2 prefetch that should NOT
    // happen on "idle" direction; assertion follows.
    await Promise.resolve()
    const queriedPages = querySpy.mock.calls.map((call) => call[0].page)
    expect(queriedPages.filter((p) => p === 4)).toEqual([])
  })

  test('scrollDirection flipping during an in-flight foreground fetch does NOT drop that fetch (review §2 regression)', async () => {
    // Pre-fix behaviour: scrollDirection was in the main fetch effect's
    // dep array, so flipping it tore down the cleanup which set
    // `cancelled = true` and short-circuited the resolved foreground
    // response — the loaded page was silently dropped. The fix splits
    // the directional +2 prefetch into a separate effect so direction
    // flips can only cancel the opportunistic background prefetch.
    let resolvePage2: ((value: unknown) => void) | undefined
    const slow = new Promise<unknown>((resolve) => {
      resolvePage2 = resolve
    })
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((req: HistoryQuery) => {
        const page = req.page ?? 1
        if (page === 2) return slow as Promise<HistoryQueryResponse>
        return Promise.resolve(
          makeHeadResponse({
            page,
            items: [
              {
                id: page * 10,
                profileId: 'chrome:Default',
                url: `u${page}`,
                title: 't',
                domain: 'example.com',
                favicon: null,
                visitedAt: '2025-12-01T10:00:00Z',
                visitTime: 1733050800,
                sourceVisitId: 0,
              },
            ],
          }),
        )
      })
    const head = makeHeadResponse({ pageCount: 6, hasNext: true })
    const { result, rerender } = renderHook(
      ({ direction }: { direction: 'idle' | 'down' | 'up' }) =>
        useExplorerInfinitePages({
          query: baseQuery,
          headResults: head,
          disabled: false,
          cacheToken: 1,
          scrollDirection: direction,
        }),
      {
        initialProps: { direction: 'idle' as 'idle' | 'down' | 'up' },
      },
    )
    // Start a foreground fetch for page 2 — it parks in `slow`.
    act(() => result.current.loadMore())
    await waitFor(() => {
      expect(querySpy.mock.calls.some((call) => call[0].page === 2)).toBe(true)
    })
    // User wobbles scroll direction mid-fetch.
    rerender({ direction: 'down' })
    rerender({ direction: 'idle' })
    rerender({ direction: 'down' })
    // Now the slow page-2 resolves. With the bug, the cleanup chain
    // would have set cancelled=true and silently dropped the response;
    // with the fix, the page lands in pageItems and extraItems exposes
    // it.
    await act(async () => {
      resolvePage2?.(
        makeHeadResponse({
          page: 2,
          items: [
            {
              id: 20,
              profileId: 'chrome:Default',
              url: 'https://example.com/page-2',
              title: 'page 2',
              domain: 'example.com',
              favicon: null,
              visitedAt: '2025-12-01T10:00:00Z',
              visitTime: 1733050800,
              sourceVisitId: 0,
            },
          ],
        }),
      )
      await slow
    })
    await waitFor(() =>
      expect(result.current.extraItems.map((item) => item.url)).toContain(
        'https://example.com/page-2',
      ),
    )
    expect(result.current.loadingMore).toBe(false)
  })
})

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  let reject: (error: unknown) => void = () => {}
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })

  return { promise, reject, resolve }
}
