/**
 * @file use-explorer-archive-density.test.tsx
 * @description Hook-level tests for the archive-wide density loader that
 * powers the paper Browse calendar popover.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { useExplorerArchiveDensity } from './use-explorer-archive-density'

vi.mock('@/lib/core-intelligence', () => ({
  getDiscoveryTrend: vi.fn(),
}))

import { getDiscoveryTrend } from '@/lib/core-intelligence'

const mockedGetDiscoveryTrend = vi.mocked(getDiscoveryTrend)

describe('useExplorerArchiveDensity', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    mockedGetDiscoveryTrend.mockReset()
  })

  test('returns empty maps + null bounds before the archive is ready', () => {
    const { result } = renderHook(() =>
      useExplorerArchiveDensity({
        archiveReady: false,
        profileId: null,
      }),
    )
    expect(result.current.perDay.size).toBe(0)
    expect(result.current.perYear.size).toBe(0)
    expect(result.current.bounds).toBeNull()
    expect(mockedGetDiscoveryTrend).not.toHaveBeenCalled()
  })

  test('defers the not-ready reset via queueMicrotask so the effect does not write state synchronously during commit', () => {
    const scheduled: Array<() => void> = []
    const queueMicrotaskSpy = vi
      .spyOn(globalThis, 'queueMicrotask')
      .mockImplementation((cb: () => void) => {
        scheduled.push(cb)
      })

    try {
      renderHook(() =>
        useExplorerArchiveDensity({
          archiveReady: false,
          profileId: null,
        }),
      )
      // The not-ready branch must defer its setState through a microtask,
      // not call setDensity inline (which would trip the
      // `react-hooks/set-state-in-effect` lint and synchronously re-render
      // during commit). A regression that drops queueMicrotask would leave
      // `scheduled` empty.
      expect(scheduled).toHaveLength(1)
      expect(typeof scheduled[0]).toBe('function')
    } finally {
      queueMicrotaskSpy.mockRestore()
    }
  })

  test('the deferred reset body is a no-op after the not-ready render unmounts (cancellation guard)', () => {
    const scheduled: Array<() => void> = []
    const queueMicrotaskSpy = vi
      .spyOn(globalThis, 'queueMicrotask')
      .mockImplementation((cb: () => void) => {
        scheduled.push(cb)
      })
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

    try {
      const { unmount } = renderHook(() =>
        useExplorerArchiveDensity({
          archiveReady: false,
          profileId: null,
        }),
      )
      expect(scheduled).toHaveLength(1)

      // Cleanup must flip the closure's cancelledReset flag so the still-
      // queued body short-circuits instead of calling setDensity on an
      // unmounted component (which logs a React dev-mode warning).
      unmount()
      expect(() => scheduled[0]()).not.toThrow()
      const reactWarnings = errorSpy.mock.calls.filter((args) =>
        args.some(
          (arg) => typeof arg === 'string' && /unmounted component/i.test(arg),
        ),
      )
      expect(reactWarnings).toHaveLength(0)
    } finally {
      queueMicrotaskSpy.mockRestore()
      errorSpy.mockRestore()
    }
  })

  test('aggregates daily points into per-day + per-year maps and derives bounds from availableYears', async () => {
    mockedGetDiscoveryTrend.mockResolvedValue({
      data: {
        points: [
          {
            dateKey: '2025-06-15',
            totalVisits: 12,
            newDomainCount: 1,
            discoveryRate: 0.1,
          },
          {
            dateKey: '2025-06-16',
            totalVisits: 7,
            newDomainCount: 0,
            discoveryRate: 0,
          },
          {
            dateKey: '2024-12-31',
            totalVisits: 3,
            newDomainCount: 1,
            discoveryRate: 0.3,
          },
        ],
        availableYears: [2024, 2025],
      },
      meta: { state: 'ready' },
    } as never)
    const { result } = renderHook(() =>
      useExplorerArchiveDensity({
        archiveReady: true,
        profileId: 'chrome:Default',
      }),
    )
    await waitFor(() => expect(result.current.bounds).not.toBeNull())
    expect(result.current.perDay.get('2025-06-15')).toBe(12)
    expect(result.current.perDay.get('2024-12-31')).toBe(3)
    expect(result.current.perYear.get(2025)).toBe(19)
    expect(result.current.perYear.get(2024)).toBe(3)
    expect(result.current.bounds?.firstYear).toBe(2024)
    expect(result.current.bounds?.lastYear).toBe(2025)
    // The calendar popover and day-nav prev/next clamp on `firstIso` /
    // `lastIso`. Both must reflect the real earliest / latest visit days,
    // otherwise clicking the topmost year on a partial-year archive
    // jumps to Dec 31 of the future and falls into the empty state.
    expect(result.current.bounds?.firstIso).toBe('2024-12-31')
    expect(result.current.bounds?.lastIso).toBe('2025-06-16')
  })

  test('firstYear/lastYear track the real perDay range when availableYears reports pre-window years', async () => {
    // Backend reports years stretching back to 2010 (covered by the
    // discovery_trend rollup table) but the 20-year query window only
    // returns points for 2018+. The calendar year rail must only expose
    // years that actually land inside [firstIso, lastIso] — otherwise
    // clicking 2010 jumps to 2010-01-01 below firstIso and strands the
    // user on an empty contact sheet.
    mockedGetDiscoveryTrend.mockResolvedValue({
      data: {
        points: [
          {
            dateKey: '2018-01-15',
            totalVisits: 4,
            newDomainCount: 0,
            discoveryRate: 0,
          },
          {
            dateKey: '2025-06-15',
            totalVisits: 12,
            newDomainCount: 0,
            discoveryRate: 0,
          },
        ],
        availableYears: [2010, 2018, 2025],
      },
      meta: { state: 'ready' },
    } as never)
    const { result } = renderHook(() =>
      useExplorerArchiveDensity({
        archiveReady: true,
        profileId: null,
      }),
    )
    await waitFor(() => expect(result.current.bounds).not.toBeNull())
    expect(result.current.bounds?.firstIso).toBe('2018-01-15')
    expect(result.current.bounds?.lastIso).toBe('2025-06-15')
    // firstYear/lastYear must match the perDay range, NOT availableYears.
    // The old code returned firstYear=2010 from availableYears[0]; the year
    // rail rendered 2010 as clickable and jumped activeDate below firstIso.
    expect(result.current.bounds?.firstYear).toBe(2018)
    expect(result.current.bounds?.lastYear).toBe(2025)
  })

  test('falls back to empty density when the backend rejects', async () => {
    mockedGetDiscoveryTrend.mockRejectedValue(new Error('archive locked'))
    const { result } = renderHook(() =>
      useExplorerArchiveDensity({
        archiveReady: true,
        profileId: null,
      }),
    )
    await waitFor(() => expect(mockedGetDiscoveryTrend).toHaveBeenCalled())
    expect(result.current.perDay.size).toBe(0)
    expect(result.current.bounds).toBeNull()
  })

  test('ignores a stale successful density response after refreshKey changes', async () => {
    const slow = deferred<Awaited<ReturnType<typeof getDiscoveryTrend>>>()
    mockedGetDiscoveryTrend
      .mockReturnValueOnce(slow.promise)
      .mockResolvedValueOnce({
        data: {
          points: [
            {
              dateKey: '2026-01-01',
              totalVisits: 4,
              newDomainCount: 0,
              discoveryRate: 0,
            },
          ],
          availableYears: [2026],
        },
        meta: { state: 'ready' },
      } as never)
    const { result, rerender } = renderHook(
      ({ refreshKey }: { refreshKey: number }) =>
        useExplorerArchiveDensity({
          archiveReady: true,
          profileId: null,
          refreshKey,
        }),
      { initialProps: { refreshKey: 1 } },
    )

    rerender({ refreshKey: 2 })
    slow.resolve({
      data: {
        points: [
          {
            dateKey: '2020-01-01',
            totalVisits: 99,
            newDomainCount: 0,
            discoveryRate: 0,
          },
        ],
        availableYears: [2020],
      },
      meta: { state: 'ready' },
    } as never)

    await waitFor(() => expect(result.current.perDay.get('2026-01-01')).toBe(4))
    expect(result.current.perDay.has('2020-01-01')).toBe(false)
  })

  test('ignores a stale rejected density response after refreshKey changes', async () => {
    const slow = deferred<Awaited<ReturnType<typeof getDiscoveryTrend>>>()
    mockedGetDiscoveryTrend
      .mockReturnValueOnce(slow.promise)
      .mockResolvedValueOnce({
        data: {
          points: [
            {
              dateKey: '2026-01-01',
              totalVisits: 4,
              newDomainCount: 0,
              discoveryRate: 0,
            },
          ],
          availableYears: [2026],
        },
        meta: { state: 'ready' },
      } as never)
    const { result, rerender } = renderHook(
      ({ refreshKey }: { refreshKey: number }) =>
        useExplorerArchiveDensity({
          archiveReady: true,
          profileId: null,
          refreshKey,
        }),
      { initialProps: { refreshKey: 1 } },
    )

    rerender({ refreshKey: 2 })
    slow.reject(new Error('stale density failure'))

    await waitFor(() => expect(result.current.perDay.get('2026-01-01')).toBe(4))
  })

  test('builds year bounds from availableYears when there are no day points', async () => {
    mockedGetDiscoveryTrend.mockResolvedValue({
      data: {
        points: [],
        availableYears: [2021, 2023],
      },
      meta: { state: 'ready' },
    } as never)
    const { result } = renderHook(() =>
      useExplorerArchiveDensity({
        archiveReady: true,
        profileId: null,
      }),
    )

    await waitFor(() => expect(result.current.bounds).not.toBeNull())
    expect(result.current.bounds?.firstIso).toBe('2021-01-01')
    expect(result.current.bounds?.lastIso).toBe('2023-12-31')
    expect(result.current.bounds?.firstYear).toBe(2021)
    expect(result.current.bounds?.lastYear).toBe(2023)
  })

  test('keeps bounds null when the backend returns no finite years', async () => {
    const density = deferred<Awaited<ReturnType<typeof getDiscoveryTrend>>>()
    mockedGetDiscoveryTrend.mockReturnValueOnce(density.promise)
    const { result } = renderHook(() =>
      useExplorerArchiveDensity({
        archiveReady: true,
        profileId: null,
      }),
    )

    await act(async () => {
      density.resolve({
        data: {
          points: [
            {
              dateKey: '',
              totalVisits: 9,
              newDomainCount: 0,
              discoveryRate: 0,
            },
          ],
          availableYears: [Number.NaN, Number.POSITIVE_INFINITY],
        },
        meta: { state: 'ready' },
      } as never)
      await density.promise
    })

    expect(result.current.perDay.size).toBe(0)
    expect(result.current.perYear.size).toBe(0)
    expect(result.current.bounds).toBeNull()
  })

  test('skips points without dateKey + ignores unparseable year prefixes', async () => {
    mockedGetDiscoveryTrend.mockResolvedValue({
      data: {
        points: [
          {
            dateKey: '',
            totalVisits: 99,
            newDomainCount: 0,
            discoveryRate: 0,
          },
          {
            dateKey: 'bogus-iso',
            totalVisits: 5,
            newDomainCount: 0,
            discoveryRate: 0,
          },
          {
            dateKey: '2026-01-01',
            totalVisits: 4,
            newDomainCount: 0,
            discoveryRate: 0,
          },
        ],
        availableYears: [2026],
      },
      meta: { state: 'ready' },
    } as never)
    const { result } = renderHook(() =>
      useExplorerArchiveDensity({
        archiveReady: true,
        profileId: null,
      }),
    )
    await waitFor(() => expect(result.current.bounds).not.toBeNull())
    expect(result.current.perDay.has('')).toBe(false)
    expect(result.current.perDay.get('bogus-iso')).toBe(5)
    expect(result.current.perYear.get(2026)).toBe(4)
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
