/**
 * @file use-explorer-og-images.test.tsx
 * @description Hook-level coverage for the lazy og:image hydration path.
 * @module pages/explorer/hooks
 */

import { renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../../lib/backend-client'
import type { HistoryEntry, HistoryQueryResponse } from '../../../lib/types'
import { historyOgImageLookupKey } from '../helpers'
import { useExplorerOgImages } from './use-explorer-og-images'

describe('useExplorerOgImages', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  test('deduplicates visible URLs and caches hydrated og:images', async () => {
    vi.spyOn(backend, 'loadHistoryOgImages').mockResolvedValue([
      {
        url: 'https://example.com/a',
        ogImage: { dataUrl: 'data:image/png;base64,AAA=' },
        fetchStatus: 'ok',
      },
    ])
    vi.spyOn(backend, 'markOgImagesShown').mockResolvedValue()

    const results = historyResponse([
      historyEntry(1, 'https://example.com/a'),
      historyEntry(2, 'https://example.com/a'),
    ])

    const { result } = renderHook(() =>
      useExplorerOgImages({ cacheToken: 1, loading: false, results }),
    )

    await waitFor(() =>
      expect(backend.loadHistoryOgImages).toHaveBeenCalledWith([
        { url: 'https://example.com/a' },
      ]),
    )
    await waitFor(() =>
      expect(
        result.current.ogImageCache.get(
          historyOgImageLookupKey('https://example.com/a'),
        ),
      ).toEqual({ dataUrl: 'data:image/png;base64,AAA=' }),
    )
  })

  test('stores null in the cache when the lookup misses or fails', async () => {
    vi.spyOn(backend, 'loadHistoryOgImages').mockResolvedValue([
      {
        url: 'https://example.com/missing',
        ogImage: null,
        fetchStatus: 'missing',
      },
    ])
    vi.spyOn(backend, 'markOgImagesShown').mockResolvedValue()

    const results = historyResponse([
      historyEntry(1, 'https://example.com/missing'),
    ])

    const { result } = renderHook(() =>
      useExplorerOgImages({ cacheToken: 1, loading: false, results }),
    )

    await waitFor(() =>
      expect(
        result.current.ogImageCache.has(
          historyOgImageLookupKey('https://example.com/missing'),
        ),
      ).toBe(true),
    )
    expect(
      result.current.ogImageCache.get(
        historyOgImageLookupKey('https://example.com/missing'),
      ),
    ).toBeNull()
  })

  test('skips loading while the route page is still fetching', () => {
    const loadSpy = vi.spyOn(backend, 'loadHistoryOgImages')
    const markSpy = vi.spyOn(backend, 'markOgImagesShown')

    renderHook(() =>
      useExplorerOgImages({
        cacheToken: 1,
        loading: true,
        results: historyResponse([historyEntry(1, 'https://example.com/a')]),
      }),
    )

    expect(loadSpy).not.toHaveBeenCalled()
    expect(markSpy).not.toHaveBeenCalled()
  })

  test('skips loading entirely when enabled=false (list mode)', () => {
    const loadSpy = vi.spyOn(backend, 'loadHistoryOgImages')
    const markSpy = vi.spyOn(backend, 'markOgImagesShown')

    renderHook(() =>
      useExplorerOgImages({
        cacheToken: 1,
        loading: false,
        enabled: false,
        results: historyResponse([historyEntry(1, 'https://example.com/a')]),
      }),
    )

    expect(loadSpy).not.toHaveBeenCalled()
    expect(markSpy).not.toHaveBeenCalled()
  })

  test('bumps last_shown_at via markOgImagesShown after the debounce fires', async () => {
    vi.useFakeTimers()
    vi.spyOn(backend, 'loadHistoryOgImages').mockResolvedValue([])
    const markSpy = vi.spyOn(backend, 'markOgImagesShown').mockResolvedValue()

    renderHook(() =>
      useExplorerOgImages({
        cacheToken: 1,
        loading: false,
        results: historyResponse([
          historyEntry(1, 'https://example.com/a'),
          historyEntry(2, 'https://example.com/b'),
        ]),
      }),
    )

    // markOgImagesShown is debounced ~1 s; advancing fake timers triggers it.
    await vi.advanceTimersByTimeAsync(1100)
    expect(markSpy).toHaveBeenCalledTimes(1)
    const urls = markSpy.mock.calls[0][0] as string[]
    expect(urls.sort()).toEqual([
      'https://example.com/a',
      'https://example.com/b',
    ])
  })

  test('returns an empty cache while the cache token rotates', () => {
    vi.spyOn(backend, 'loadHistoryOgImages').mockResolvedValue([])
    vi.spyOn(backend, 'markOgImagesShown').mockResolvedValue()

    const { result, rerender } = renderHook(
      ({ cacheToken }: { cacheToken: number }) =>
        useExplorerOgImages({
          cacheToken,
          loading: false,
          results: historyResponse([historyEntry(1, 'https://example.com/a')]),
        }),
      { initialProps: { cacheToken: 1 } },
    )

    rerender({ cacheToken: 2 })
    expect(result.current.ogImageCache.size).toBe(0)
  })

  test('swallows markOgImagesShown failures silently', async () => {
    vi.useFakeTimers()
    vi.spyOn(backend, 'loadHistoryOgImages').mockResolvedValue([])
    vi.spyOn(backend, 'markOgImagesShown').mockRejectedValue(
      new Error('transport blew up'),
    )

    renderHook(() =>
      useExplorerOgImages({
        cacheToken: 1,
        loading: false,
        results: historyResponse([historyEntry(1, 'https://example.com/a')]),
      }),
    )

    // No throw, no unhandled rejection bubbling out of the hook.
    await vi.advanceTimersByTimeAsync(1100)
  })
})

function historyResponse(items: HistoryEntry[]): HistoryQueryResponse {
  return {
    items,
    total: items.length,
    page: 1,
    pageSize: 25,
    pageCount: 1,
    hasNext: false,
    hasPrevious: false,
    nextCursor: null,
  }
}

function historyEntry(id: number, url: string): HistoryEntry {
  return {
    appId: null,
    domain: new URL(url).hostname,
    durationMs: null,
    favicon: null,
    id,
    profileId: 'chrome:Default',
    sourceVisitId: id,
    title: 'Example',
    transition: null,
    url,
    visitedAt: new Date(0).toISOString(),
    visitTime: id * 1000,
  }
}
