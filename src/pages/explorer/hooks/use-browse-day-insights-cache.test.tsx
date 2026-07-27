/**
 * Tests for `useBrowseDayInsightsCache`.
 *
 * Covers:
 * - `resolve(date)` is a PURE lookup: a miss returns null and fires no
 *   backend call. (It used to fetch as a side effect of render — the E5
 *   render-path-IPC bug. These tests are the contract that keeps it pure.)
 * - `request(date)` is the only fetch trigger, deduped per `(token, date)`.
 * - When the backend reply lands, the next `resolve(date)` returns the
 *   adapted insights.
 * - Bumping `refreshKey` clears the cache so a new fetch fires, and rotates
 *   `request`'s identity so callers' effects re-arm.
 * - Switching `profileId` clears the cache.
 * - Failures are retried a BOUNDED number of times and then settle, so a
 *   day is never pinned to client-side aggregation forever by one transient
 *   error, and a permanently failing backend is never hammered.
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

  test('resolve is a pure lookup: a miss never fires a backend call', () => {
    // E5 contract. This assertion used to be the opposite
    // (`expect(spy).toHaveBeenCalledTimes(1)` after a bare `resolve`), which
    // is precisely what put IPC + an in-place Map mutation on the contact
    // sheet's render path.
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockResolvedValue(fakeInsights('2026-05-25'))
    const { result } = renderHook(() =>
      useBrowseDayInsightsCache({ profileId: null, refreshKey: 1 }),
    )

    expect(result.current.resolve('2026-05-25')).toBeNull()
    expect(result.current.resolve('2026-05-25')).toBeNull()
    expect(spy).not.toHaveBeenCalled()
  })

  test('request fires exactly one backend call per date and dedupes repeats', () => {
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockResolvedValue(fakeInsights('2026-05-25'))
    const { result } = renderHook(() =>
      useBrowseDayInsightsCache({ profileId: null, refreshKey: 1 }),
    )

    act(() => {
      result.current.request('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(1)
    expect(spy).toHaveBeenCalledWith({
      date: '2026-05-25',
      profileId: null,
    })

    // Two adjacent day mounts asking for the same date share one call.
    act(() => {
      result.current.request('2026-05-25')
      result.current.request('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(1)
  })

  test('request identity is stable across replies and rotates with the cache token', async () => {
    // The contact sheet uses `request` as an effect dependency. A churning
    // identity would re-run every mounted day's effect on every reply; a
    // frozen one would never re-arm after a refresh. It must change on
    // exactly one thing: the cache token.
    vi.spyOn(backend, 'getBrowseDayInsights').mockResolvedValue(
      fakeInsights('2026-05-25'),
    )
    const { result, rerender } = renderHook(
      (props: { refreshKey: number }) =>
        useBrowseDayInsightsCache({
          profileId: null,
          refreshKey: props.refreshKey,
        }),
      { initialProps: { refreshKey: 1 } },
    )
    const initialRequest = result.current.request

    act(() => {
      result.current.request('2026-05-25')
    })
    await waitFor(() => {
      expect(result.current.resolve('2026-05-25')).not.toBeNull()
    })
    // A landed reply must NOT churn `request`…
    expect(result.current.request).toBe(initialRequest)

    // …but a refreshKey bump must.
    rerender({ refreshKey: 2 })
    expect(result.current.request).not.toBe(initialRequest)
  })

  test('returns adapted insights once the backend reply lands', async () => {
    vi.spyOn(backend, 'getBrowseDayInsights').mockResolvedValue(
      fakeInsights('2026-05-25'),
    )
    const { result } = renderHook(() =>
      useBrowseDayInsightsCache({ profileId: null, refreshKey: 1 }),
    )
    act(() => {
      result.current.request('2026-05-25')
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

  test('changing refreshKey clears the cache and re-fetches', async () => {
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
      result.current.request('2026-05-25')
    })
    await waitFor(() => {
      expect(result.current.resolve('2026-05-25')).not.toBeNull()
    })
    expect(spy).toHaveBeenCalledTimes(1)

    rerender({ refreshKey: 2 })
    // The stale aggregate must not be served against the new archive state.
    expect(result.current.resolve('2026-05-25')).toBeNull()
    act(() => {
      result.current.request('2026-05-25')
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
      result.current.request('2026-05-25')
    })
    rerender({ refreshKey: 2 })
    act(() => {
      result.current.request('2026-05-25')
    })
    await act(async () => {
      slow.resolve(fakeInsights('2026-05-25'))
      await slow.promise
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
      result.current.request('2026-05-25')
    })
    rerender({ refreshKey: 2 })
    act(() => {
      result.current.request('2026-05-25')
    })
    await act(async () => {
      slow.reject(new Error('stale failure'))
      await slow.promise.catch(() => undefined)
    })

    expect(spy).toHaveBeenCalledTimes(2)
    // The stale rejection must not have poisoned the fresh token's entry.
    await waitFor(() => {
      expect(result.current.resolve('2026-05-25')).not.toBeNull()
    })
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
      result.current.request('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(1)
    rerender({ profileId: 'chrome:Default' })
    act(() => {
      result.current.request('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(2)
    expect(spy).toHaveBeenLastCalledWith({
      date: '2026-05-25',
      profileId: 'chrome:Default',
    })
  })

  test('retries a failed day a bounded number of times, then settles', async () => {
    // Previously a single rejection pinned the day to the client-side
    // aggregator for the entire refresh cycle — permanently, for a merely
    // transient failure (archive briefly locked mid-import). Now a later
    // mount of the same day gets another chance, but only a bounded number.
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockRejectedValue(new Error('archive locked'))
    const { result } = renderHook(() =>
      useBrowseDayInsightsCache({ profileId: null, refreshKey: 1 }),
    )

    // Attempts 1..3: each subsequent `request` (i.e. the day scrolling back
    // into view) re-attempts while the budget lasts.
    for (const expected of [1, 2, 3]) {
      act(() => {
        result.current.request('2026-05-25')
      })
      expect(spy).toHaveBeenCalledTimes(expected)
      await waitFor(() => {
        expect(result.current.resolve('2026-05-25')).toBeNull()
      })
    }

    // Budget exhausted: further requests are silent no-ops.
    act(() => {
      result.current.request('2026-05-25')
      result.current.request('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(3)
    expect(result.current.resolve('2026-05-25')).toBeNull()
  })

  test('a retry that succeeds replaces the error entry', async () => {
    const spy = vi
      .spyOn(backend, 'getBrowseDayInsights')
      .mockRejectedValueOnce(new Error('archive locked'))
      .mockResolvedValue(fakeInsights('2026-05-25'))
    const { result } = renderHook(() =>
      useBrowseDayInsightsCache({ profileId: null, refreshKey: 1 }),
    )

    act(() => {
      result.current.request('2026-05-25')
    })
    await waitFor(() => {
      expect(spy).toHaveBeenCalledTimes(1)
    })
    expect(result.current.resolve('2026-05-25')).toBeNull()

    act(() => {
      result.current.request('2026-05-25')
    })
    await waitFor(() => {
      expect(result.current.resolve('2026-05-25')).not.toBeNull()
    })
    // Success is terminal — no further attempts are spent.
    act(() => {
      result.current.request('2026-05-25')
    })
    expect(spy).toHaveBeenCalledTimes(2)
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
