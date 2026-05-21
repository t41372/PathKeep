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
