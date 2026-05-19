/**
 * Tests for the paper Browse date helpers.
 *
 * Why this file exists:
 * - Date math is easy to break silently. These helpers feed the calendar
 *   popover and day-nav pill, so a regression here lands as a visible
 *   navigation bug far from the call site.
 */

import { describe, expect, test } from 'vitest'
import {
  addDaysIso,
  dateFromIso,
  dayDensityTier,
  isoFromDate,
  periodDensityTier,
  prettyDay,
  relativeDayLabel,
  type RelativeDayCopy,
} from './date-helpers'

const COPY: RelativeDayCopy = {
  today: 'today',
  yesterday: 'yesterday',
  daysAgo: '{count}d ago',
  weeksAgo: '{count}w ago',
  monthsAgo: '{count}mo ago',
  yearsAgo: '{count}y ago',
}

describe('isoFromDate / dateFromIso', () => {
  test('round-trips local dates without UTC drift', () => {
    const date = new Date(2026, 4, 17) // 2026-05-17 local
    const iso = isoFromDate(date)
    expect(iso).toBe('2026-05-17')
    const restored = dateFromIso(iso)
    expect(restored.getFullYear()).toBe(2026)
    expect(restored.getMonth()).toBe(4)
    expect(restored.getDate()).toBe(17)
  })

  test('dateFromIso returns an invalid date for malformed input', () => {
    expect(Number.isNaN(dateFromIso('not-a-date').getTime())).toBe(true)
    expect(Number.isNaN(dateFromIso('2026-13').getTime())).toBe(true)
  })

  test('isoFromDate pads single-digit months and days', () => {
    expect(isoFromDate(new Date(2026, 0, 5))).toBe('2026-01-05')
  })
})

describe('addDaysIso', () => {
  test('adds and subtracts whole days, crossing month boundaries', () => {
    expect(addDaysIso('2026-05-17', 1)).toBe('2026-05-18')
    expect(addDaysIso('2026-05-01', -1)).toBe('2026-04-30')
    expect(addDaysIso('2025-12-31', 1)).toBe('2026-01-01')
  })

  test('returns the input unchanged when ISO is malformed', () => {
    expect(addDaysIso('not-a-date', 5)).toBe('not-a-date')
  })
})

describe('prettyDay', () => {
  test('formats with locale and weekday', () => {
    const label = prettyDay('2026-05-17', { language: 'en' })
    expect(label).toContain('2026')
    expect(label.toLowerCase()).toContain('may')
  })

  test('omits the year when withYear is false', () => {
    const label = prettyDay('2026-05-17', { language: 'en', withYear: false })
    expect(label).not.toContain('2026')
  })

  test('uses short weekday when requested', () => {
    const longLabel = prettyDay('2026-05-17', { language: 'en' })
    const shortLabel = prettyDay('2026-05-17', { language: 'en', short: true })
    // Long weekday is at least 6 chars (Sunday), short is 3 chars (Sun).
    expect(longLabel.length).toBeGreaterThan(shortLabel.length)
  })

  test('falls back to the ISO string when the date is invalid', () => {
    expect(prettyDay('not-a-date')).toBe('not-a-date')
  })
})

describe('dayDensityTier', () => {
  test.each([
    [0, 0],
    [-1, 0],
    [1, 1],
    [29, 1],
    [30, 2],
    [149, 2],
    [150, 3],
    [499, 3],
    [500, 4],
    [10_000, 4],
  ])('count %i → tier %i', (count, tier) => {
    expect(dayDensityTier(count)).toBe(tier)
  })
})

describe('periodDensityTier', () => {
  test.each([
    [0, 0],
    [-5, 0],
    [1, 1],
    [4_999, 1],
    [5_000, 2],
    [29_999, 2],
    [30_000, 3],
    [89_999, 3],
    [90_000, 4],
  ])('count %i → tier %i', (count, tier) => {
    expect(periodDensityTier(count)).toBe(tier)
  })
})

describe('relativeDayLabel', () => {
  const reference = '2026-05-17'

  test('returns "today" when the dates match', () => {
    expect(relativeDayLabel(reference, reference, COPY)).toBe('today')
  })

  test('returns "yesterday" for one-day deltas', () => {
    expect(relativeDayLabel('2026-05-16', reference, COPY)).toBe('yesterday')
  })

  test('formats days, weeks, months, and years as the delta grows', () => {
    expect(relativeDayLabel('2026-05-13', reference, COPY)).toBe('4d ago')
    expect(relativeDayLabel('2026-04-19', reference, COPY)).toBe('4w ago')
    expect(relativeDayLabel('2026-02-15', reference, COPY)).toBe('3mo ago')
    expect(relativeDayLabel('2024-05-17', reference, COPY)).toBe('2.0y ago')
  })

  test('returns an empty string when either input is invalid', () => {
    expect(relativeDayLabel('garbage', reference, COPY)).toBe('')
    expect(relativeDayLabel(reference, 'garbage', COPY)).toBe('')
  })
})
