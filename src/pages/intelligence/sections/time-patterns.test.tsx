/**
 * @file time-patterns.test.tsx
 * @description Unit tests for weekday/weekend split and peak hours computation + rendering.
 * @module pages/intelligence/sections
 *
 * ## Responsibilities
 * - Verify pure computation helpers: computeWeekdayWeekend, computePeakHours.
 * - Verify component rendering for empty, partial, and full data scenarios.
 *
 * ## Not responsible for
 * - Testing the browsing-rhythm heatmap calendar (separate module).
 * - Testing data loading (components receive pre-loaded cells).
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import type { RhythmHeatmapCell } from '../../../lib/core-intelligence'
import {
  computePeakHours,
  computeWeekdayWeekend,
} from './time-patterns-helpers'
import { PeakHoursStrip, WeekdayWeekendStrip } from './time-patterns'

function t(key: string) {
  const map: Record<string, string> = {
    hubWeekdayLabel: 'Weekdays',
    hubWeekendLabel: 'Weekends',
    hubWeekdayWeekendEmpty: 'Not enough daily data',
    hubPeakHoursTitle: 'Peak Hours',
    hubPeakHoursEmpty: 'Not enough data for peak hours',
  }
  return map[key] ?? key
}

describe('computeWeekdayWeekend', () => {
  test('splits cells into weekday and weekend totals', () => {
    const cells: RhythmHeatmapCell[] = [
      { dow: 1, hour: 10, visitCount: 5 }, // Mon
      { dow: 2, hour: 10, visitCount: 3 }, // Tue
      { dow: 6, hour: 10, visitCount: 7 }, // Sat
      { dow: 0, hour: 10, visitCount: 2 }, // Sun
    ]

    const result = computeWeekdayWeekend(cells)
    expect(result.weekdayVisits).toBe(8)
    expect(result.weekendVisits).toBe(9)
  })

  test('returns zeros for empty array', () => {
    const result = computeWeekdayWeekend([])
    expect(result.weekdayVisits).toBe(0)
    expect(result.weekendVisits).toBe(0)
  })
})

describe('computePeakHours', () => {
  test('finds the peak 3-hour window across all days', () => {
    const cells: RhythmHeatmapCell[] = [
      { dow: 1, hour: 9, visitCount: 10 },
      { dow: 1, hour: 10, visitCount: 20 },
      { dow: 1, hour: 11, visitCount: 15 },
      { dow: 1, hour: 14, visitCount: 5 },
      { dow: 2, hour: 10, visitCount: 8 },
    ]

    const result = computePeakHours(cells)
    // hour 9: 10, hour 10: 20+8=28, hour 11: 15 = 53
    expect(result).toEqual({ startHour: 9, totalVisits: 53 })
  })

  test('returns null for empty cells', () => {
    expect(computePeakHours([])).toBeNull()
  })

  test('returns null when all visit counts are zero', () => {
    const cells: RhythmHeatmapCell[] = [
      { dow: 1, hour: 10, visitCount: 0 },
      { dow: 1, hour: 11, visitCount: 0 },
    ]
    expect(computePeakHours(cells)).toBeNull()
  })
})

describe('WeekdayWeekendStrip', () => {
  test('renders empty state for no data', () => {
    render(<WeekdayWeekendStrip cells={[]} t={t} />)
    expect(screen.getByText('Not enough daily data')).toBeInTheDocument()
  })

  test('renders percentage bars for valid data', () => {
    const cells: RhythmHeatmapCell[] = [
      { dow: 1, hour: 10, visitCount: 75 },
      { dow: 6, hour: 10, visitCount: 25 },
    ]
    render(<WeekdayWeekendStrip cells={cells} t={t} />)
    expect(screen.getByText('Weekdays')).toBeInTheDocument()
    expect(screen.getByText('Weekends')).toBeInTheDocument()
    expect(screen.getByText('75%')).toBeInTheDocument()
    expect(screen.getByText('25%')).toBeInTheDocument()
  })
})

describe('PeakHoursStrip', () => {
  test('renders empty state for no data', () => {
    render(<PeakHoursStrip cells={[]} t={t} />)
    expect(
      screen.getByText('Not enough data for peak hours'),
    ).toBeInTheDocument()
  })

  test('renders peak hour range for valid data', () => {
    const cells: RhythmHeatmapCell[] = [
      { dow: 1, hour: 14, visitCount: 30 },
      { dow: 1, hour: 15, visitCount: 20 },
      { dow: 1, hour: 16, visitCount: 10 },
    ]
    render(<PeakHoursStrip cells={cells} t={t} />)
    expect(screen.getByText('Peak Hours')).toBeInTheDocument()
  })
})
