/**
 * Tests for `useBrowseDayInsightsCache`.
 *
 * Covers:
 * - First `resolve(date)` returns null AND fires a backend fetch.
 * - Second `resolve(date)` during the same cycle is a no-op (in-flight
 *   dedup).
 * - When the backend reply lands, the next `resolve(date)` returns
 *   the adapted insights.
 * - Bumping `refreshKey` clears the cache so a new fetch fires.
 * - Switching `profileId` clears the cache.
 * - Errors are captured into the cache as a sentinel state and the
 *   resolver keeps returning null (so the contact sheet's client-side
 *   fallback aggregator keeps rendering instead of throwing).
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '@/lib/backend-client'
import type { BrowseDayInsights } from '@/lib/backend-client/explorer'
import { useBrowseDayInsightsCache } from './use-browse-day-insights-cache'

function fakeInsights(date: string): BrowseDayInsights {
  return {
    date,
    totalPages: 42,
    typedCount: 3,
    linkCount: 30,
    searchCount: 9,
    distinctDomains: 7,
    sessionCount: 2,
    topDomains: [{ domain: 'example.test', visits: 12 }],
    hourBuckets: Array.from({ length: 24 }, (_, hour) =>
      hour === 10 ? 12 : 0,
    ),
    hourPeak: 12,
    firstVisitMs: 1_716_624_000_000,
    lastVisitMs: 1_716_660_000_000,
    peakHour: 10,
    longestSessionMs: 1_800_000,
    topUrls: [{ url: 'https://example.test/', title: 'Example', visits: 5 }],
    topSearchQueries: [{ query: 'sqlite wal', count: 2 }],
  }
}

describe('useBrowseDayInsightsCache', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('resolve first miss triggers a backend fetch and returns null', () => {
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockResolvedValue(fakeInsights('2026-05-25'))
    const { result } = renderHook(() =>
      useBrowseDayInsightsCache({ profileId: null, refreshKey: 1 }),
    )
    let initial: ReturnType<typeof result.current.resolve> | undefined
    act(() => {
      initial = result.current.resolve('2026-05-25')
    })
    expect(initial).toBeNull()
    expect(spy).toHaveBeenCalledTimes(1)
    expect(spy).toHaveBeenCalledWith({
      date: '2026-05-25',
      profileId: null,
    })
    // Second resolve during the same cycle does not fire another call.
    act(() => {
      result.current.resolve('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(1)
  })

  test('returns adapted insights once the backend reply lands', async () => {
    vi.spyOn(backend, 'getBrowseDayInsights').mockResolvedValue(
      fakeInsights('2026-05-25'),
    )
    const { result } = renderHook(() =>
      useBrowseDayInsightsCache({ profileId: null, refreshKey: 1 }),
    )
    act(() => {
      result.current.resolve('2026-05-25')
    })
    await waitFor(() => {
      const ready = result.current.resolve('2026-05-25')
      expect(ready).not.toBeNull()
    })
    const insights = result.current.resolve('2026-05-25')!
    expect(insights.totalPages).toBe(42)
    expect(insights.topDomains[0]).toEqual({
      domain: 'example.test',
      visits: 12,
    })
    expect(insights.peakHour).toBe(10)
    expect(insights.hourBuckets).toHaveLength(24)
    // The wire `date` field must be stripped from the adapted shape so
    // it stays interchangeable with the client-side DayInsights type.
    expect((insights as unknown as { date?: unknown }).date).toBeUndefined()
  })

  test('changing refreshKey clears the cache and re-fetches', () => {
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockResolvedValue(fakeInsights('2026-05-25'))
    const { result, rerender } = renderHook(
      (props: { refreshKey: number }) =>
        useBrowseDayInsightsCache({
          profileId: null,
          refreshKey: props.refreshKey,
        }),
      { initialProps: { refreshKey: 1 } },
    )
    act(() => {
      result.current.resolve('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(1)
    rerender({ refreshKey: 2 })
    act(() => {
      result.current.resolve('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(2)
  })

  test('drops a successful reply that resolves after refreshKey changes', async () => {
    const slow = deferred<BrowseDayInsights>()
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockReturnValueOnce(slow.promise)
      .mockResolvedValue(fakeInsights('2026-05-25'))
    const { result, rerender } = renderHook(
      (props: { refreshKey: number }) =>
        useBrowseDayInsightsCache({
          profileId: null,
          refreshKey: props.refreshKey,
        }),
      { initialProps: { refreshKey: 1 } },
    )

    act(() => {
      result.current.resolve('2026-05-25')
    })
    rerender({ refreshKey: 2 })
    await act(async () => {
      slow.resolve(fakeInsights('2026-05-25'))
      await slow.promise
    })
    act(() => {
      result.current.resolve('2026-05-25')
    })

    expect(spy).toHaveBeenCalledTimes(2)
  })

  test('drops a rejected reply that resolves after refreshKey changes', async () => {
    const slow = deferred<BrowseDayInsights>()
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockReturnValueOnce(slow.promise)
      .mockResolvedValue(fakeInsights('2026-05-25'))
    const { result, rerender } = renderHook(
      (props: { refreshKey: number }) =>
        useBrowseDayInsightsCache({
          profileId: null,
          refreshKey: props.refreshKey,
        }),
      { initialProps: { refreshKey: 1 } },
    )

    act(() => {
      result.current.resolve('2026-05-25')
    })
    rerender({ refreshKey: 2 })
    await act(async () => {
      slow.reject(new Error('stale failure'))
      await slow.promise.catch(() => undefined)
    })
    act(() => {
      result.current.resolve('2026-05-25')
    })

    expect(spy).toHaveBeenCalledTimes(2)
  })

  test('changing profileId clears the cache', () => {
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockResolvedValue(fakeInsights('2026-05-25'))
    const { result, rerender } = renderHook(
      (props: { profileId: string | null }) =>
        useBrowseDayInsightsCache({
          profileId: props.profileId,
          refreshKey: 1,
        }),
      { initialProps: { profileId: null as string | null } },
    )
    act(() => {
      result.current.resolve('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(1)
    rerender({ profileId: 'chrome:Default' })
    act(() => {
      result.current.resolve('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(2)
    expect(spy).toHaveBeenLastCalledWith({
      date: '2026-05-25',
      profileId: 'chrome:Default',
    })
  })

  test('backend rejection keeps resolve returning null without retrying', async () => {
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockRejectedValue(new Error('archive locked'))
    const { result } = renderHook(() =>
      useBrowseDayInsightsCache({ profileId: null, refreshKey: 1 }),
    )
    act(() => {
      result.current.resolve('2026-05-25')
    })
    await waitFor(() => {
      // Wait for the rejection to settle into the cache as an error
      // entry — once that happens, additional resolve calls are
      // no-ops.
      expect(spy).toHaveBeenCalledTimes(1)
    })
    act(() => {
      result.current.resolve('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(1)
    expect(result.current.resolve('2026-05-25')).toBeNull()
  })
})

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  let reject: (error: Error) => void = () => {}
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}
