/**
 * @file use-explorer-archive-density.test.tsx
 * @description Hook-level tests for the archive-wide density loader that
 * powers the paper Browse calendar popover.
 */

import { renderHook, waitFor } from '@testing-library/react'
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
