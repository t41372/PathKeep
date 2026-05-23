/**
 * @file use-explorer-infinite-pages.test.tsx
 * @description Hook tests for the accumulating-page buffer driving paper
 * Browse infinite scroll.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '@/lib/backend-client'
import type { HistoryQuery, HistoryQueryResponse } from '@/lib/types'
import { useExplorerInfinitePages } from './use-explorer-infinite-pages'

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
    const slow = new Promise<ReturnType<typeof makeHeadResponse>>(
      (resolve) => {
        resolveFetch = resolve
      },
    )
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
})
