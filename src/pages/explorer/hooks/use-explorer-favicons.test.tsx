/**
 * @file use-explorer-favicons.test.tsx
 * @description Hook-level coverage for Explorer lazy favicon hydration.
 * @module pages/explorer/hooks
 */

import { renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../../lib/backend-client'
import type { HistoryEntry, HistoryQueryResponse } from '../../../lib/types'
import { historyFaviconLookupKey } from '../helpers'
import { useExplorerFavicons } from './use-explorer-favicons'

describe('useExplorerFavicons', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('deduplicates visible rows and caches hydrated favicons per token', async () => {
    vi.spyOn(backend, 'loadHistoryFavicons').mockResolvedValue([
      {
        favicon: { dataUrl: 'data:image/png;base64,AAA=' },
        profileId: 'chrome:Default',
        url: 'https://example.com/a',
        visitTime: 1000,
      },
    ])
    const results = historyResponse([
      historyEntry(1, 'https://example.com/a', 1000),
      historyEntry(2, 'https://example.com/a', 1000),
    ])

    const { result } = renderHook(() =>
      useExplorerFavicons({
        cacheToken: 1,
        loading: false,
        results,
      }),
    )

    await waitFor(() =>
      expect(backend.loadHistoryFavicons).toHaveBeenCalledWith([
        {
          profileId: 'chrome:Default',
          url: 'https://example.com/a',
          visitTime: 1000,
        },
      ]),
    )
    await waitFor(() =>
      expect(
        result.current.faviconCache.get(
          historyFaviconLookupKey(
            'chrome:Default',
            'https://example.com/a',
            1000,
          ),
        ),
      ).toEqual({ dataUrl: 'data:image/png;base64,AAA=' }),
    )
  })

  test('skips loading while the route page is still fetching', () => {
    const loadFavicons = vi.spyOn(backend, 'loadHistoryFavicons')

    renderHook(() =>
      useExplorerFavicons({
        cacheToken: 1,
        loading: true,
        results: historyResponse([
          historyEntry(1, 'https://example.com/a', 1000),
        ]),
      }),
    )

    expect(loadFavicons).not.toHaveBeenCalled()
  })

  test('returns an empty cache while the cache token is rotating', () => {
    const results = historyResponse([
      historyEntry(1, 'https://example.com/a', 1000),
    ])
    const { result, rerender } = renderHook(
      ({ cacheToken }: { cacheToken: number }) =>
        useExplorerFavicons({
          cacheToken,
          loading: true,
          results,
        }),
      {
        initialProps: { cacheToken: 1 },
      },
    )

    rerender({ cacheToken: 2 })

    expect(result.current.faviconCache.size).toBe(0)
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

function historyEntry(
  id: number,
  url: string,
  visitTime: number,
): HistoryEntry {
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
    visitedAt: new Date(visitTime).toISOString(),
    visitTime,
  }
}
