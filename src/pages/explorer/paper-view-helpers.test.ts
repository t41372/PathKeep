import { describe, expect, it } from 'vitest'

import type { PaperDay } from '@/pages/explorer/paper/group-entries'

import {
  buildPerDayDensity,
  buildPerYearDensity,
  inferBounds,
  pickInitialDate,
} from './paper-view-helpers'

function makeDay(date: string, visitCount = 1): PaperDay {
  return { date, visitCount, domains: 1, sessions: [] }
}

describe('pickInitialDate', () => {
  it('returns targetDate when provided', () => {
    expect(
      pickInitialDate('2025-10-01', [makeDay('2025-09-30')], '2026-05-21'),
    ).toBe('2025-10-01')
  })

  it('returns the newest day when no target and the archive has rows', () => {
    expect(
      pickInitialDate(
        null,
        [makeDay('2025-09-30'), makeDay('2025-09-29')],
        '2026-05-21',
      ),
    ).toBe('2025-09-30')
  })

  it('falls back to today when no target and no days', () => {
    expect(pickInitialDate(null, [], '2026-05-21')).toBe('2026-05-21')
  })
})

describe('buildPerDayDensity', () => {
  it('returns the loaded-day counts when no overrides supplied', () => {
    const map = buildPerDayDensity(
      [makeDay('2025-09-30', 5), makeDay('2025-09-29', 3)],
      undefined,
    )
    expect(map.get('2025-09-30')).toBe(5)
    expect(map.get('2025-09-29')).toBe(3)
  })

  it('lets overrides fill in days we have not paged yet', () => {
    const overrides = new Map<string, number>([['2025-08-01', 12]])
    const map = buildPerDayDensity([makeDay('2025-09-30', 5)], overrides)
    expect(map.get('2025-08-01')).toBe(12)
    expect(map.get('2025-09-30')).toBe(5)
  })

  it('overrides loaded counts only when the override count is higher', () => {
    const overrides = new Map<string, number>([
      ['2025-09-30', 99], // higher → wins
      ['2025-09-29', 1], // lower → loses
    ])
    const map = buildPerDayDensity(
      [makeDay('2025-09-30', 5), makeDay('2025-09-29', 3)],
      overrides,
    )
    expect(map.get('2025-09-30')).toBe(99)
    expect(map.get('2025-09-29')).toBe(3)
  })
})

describe('buildPerYearDensity', () => {
  it('sums visit counts per year from loaded days', () => {
    const map = buildPerYearDensity(
      [
        makeDay('2025-09-30', 5),
        makeDay('2025-08-01', 7),
        makeDay('2024-12-31', 2),
      ],
      undefined,
    )
    expect(map.get(2025)).toBe(12)
    expect(map.get(2024)).toBe(2)
  })

  it('skips days whose year cannot be parsed', () => {
    const map = buildPerYearDensity(
      [makeDay('bogus', 9), makeDay('2025-01-01', 4)],
      undefined,
    )
    expect(map.get(2025)).toBe(4)
    expect(map.has(Number.NaN)).toBe(false)
  })

  it('lets overrides fill in years we have not paged yet and raise low loaded counts', () => {
    const overrides = new Map<number, number>([
      [2020, 50], // year missing from loaded days → fills in
      [2025, 100], // higher than loaded sum → wins
      [2024, 1], // lower → loses
    ])
    const map = buildPerYearDensity(
      [makeDay('2025-09-30', 5), makeDay('2024-12-31', 2)],
      overrides,
    )
    expect(map.get(2020)).toBe(50)
    expect(map.get(2025)).toBe(100)
    expect(map.get(2024)).toBe(2)
  })
})

describe('inferBounds', () => {
  it('falls back to today + parsed today-year when days are empty', () => {
    const bounds = inferBounds([], '2026-05-21')
    expect(bounds.firstIso).toBe('2026-05-21')
    expect(bounds.lastIso).toBe('2026-05-21')
    expect(bounds.firstYear).toBe(2026)
    expect(bounds.lastYear).toBe(2026)
    expect(bounds.totalDays).toBe(1)
  })

  it('falls back to the current calendar year when today has an unparseable prefix', () => {
    const bounds = inferBounds([], 'bogus-iso')
    const thisYear = new Date().getFullYear()
    expect(bounds.firstYear).toBe(thisYear)
    expect(bounds.lastYear).toBe(thisYear)
  })

  it('reports the first/last iso and inclusive total-day span across loaded days', () => {
    const bounds = inferBounds(
      [makeDay('2025-09-30'), makeDay('2025-09-29'), makeDay('2025-09-28')],
      '2026-05-21',
    )
    expect(bounds.lastIso).toBe('2025-09-30')
    expect(bounds.firstIso).toBe('2025-09-28')
    expect(bounds.firstYear).toBe(2025)
    expect(bounds.lastYear).toBe(2025)
    expect(bounds.totalDays).toBe(3)
  })

  it('reports the inclusive total-day span as 2 for a single-day archive (start == end + 1 day inclusive)', () => {
    // The +1 inclusive end means a single-day archive reports totalDays=2 by
    // construction; the Math.max(1, ...) floor is for the same-iso division
    // round-down case.
    const bounds = inferBounds([makeDay('2025-09-30')], '2026-05-21')
    expect(bounds.totalDays).toBe(2)
  })
})
