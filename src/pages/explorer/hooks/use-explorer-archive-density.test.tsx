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

  test('cancels the deferred empty reset when the not-ready render unmounts', async () => {
    const { result, unmount } = renderHook(() =>
      useExplorerArchiveDensity({
        archiveReady: false,
        profileId: null,
      }),
    )

    unmount()
    await act(async () => {
      await Promise.resolve()
    })

    expect(result.current.perDay.size).toBe(0)
    expect(result.current.perYear.size).toBe(0)
    expect(result.current.bounds).toBeNull()
    expect(mockedGetDiscoveryTrend).not.toHaveBeenCalled()
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
