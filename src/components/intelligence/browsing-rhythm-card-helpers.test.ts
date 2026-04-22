/**
 * @file browsing-rhythm-card-helpers.test.ts
 * @description Guards the pure date math and summary formatting extracted from the browsing-rhythm card.
 * @module components/intelligence
 */

import { expect, test } from 'vitest'
import {
  buildCalendarWeeks,
  buildMonthLabels,
  buildVisitSummary,
  buildYearOptions,
  extractPointYear,
} from './browsing-rhythm-card-helpers'
import type { BrowsingRhythmTranslator } from './browsing-rhythm-card-helpers'

const t: BrowsingRhythmTranslator = (key, vars) => {
  switch (key) {
    case 'rhythmVisitSummaryYear':
      return `${vars?.count} visits in ${vars?.year}`
    case 'rhythmVisitSummaryDay':
      return `${vars?.count} visits on ${vars?.date}`
    case 'rhythmVisitSummaryMonth':
      return `${vars?.count} visits in ${vars?.monthYear}`
    case 'rhythmVisitSummaryRange':
      return `${vars?.count} visits from ${vars?.start} to ${vars?.end}`
    default:
      return key
  }
}

test('buildCalendarWeeks keeps in-range cells and point counts aligned', () => {
  const weeks = buildCalendarWeeks(
    { start: '2026-01-01', end: '2026-01-03' },
    new Map([
      [
        '2026-01-03',
        {
          dateKey: '2026-01-03',
          discoveryRate: 0.25,
          newDomainCount: 2,
          totalVisits: 8,
        },
      ],
    ]),
  )

  expect(weeks).toHaveLength(1)
  expect(weeks[0]).toHaveLength(7)
  expect(weeks[0][0]).toMatchObject({
    dateKey: '2025-12-28',
    inRange: false,
    totalVisits: 0,
  })
  expect(weeks[0][6]).toMatchObject({
    dateKey: '2026-01-03',
    inRange: true,
    newDomainCount: 2,
    totalVisits: 8,
  })
})

test('buildMonthLabels surfaces month captions at the first in-range week', () => {
  const weeks = buildCalendarWeeks(
    { start: '2026-01-01', end: '2026-02-03' },
    new Map(),
  )

  expect(buildMonthLabels(weeks, 'en')).toEqual([
    'Jan',
    'Jan',
    '',
    '',
    '',
    'Feb',
  ])
})

test('buildYearOptions always includes the current year and sorts descending', () => {
  expect(buildYearOptions([], 2026)).toEqual([2026])
  expect(buildYearOptions([2024, 2022, 2025], 2026)).toEqual([
    2026, 2025, 2024, 2023, 2022,
  ])
})

test('buildVisitSummary preserves the calendar-year and range branches', () => {
  expect(
    buildVisitSummary({
      dateRange: { start: '2026-01-01', end: '2026-12-31' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'calendar-year',
      totalVisits: 120,
      t,
    }),
  ).toBe('120 visits in 2026')

  expect(
    buildVisitSummary({
      dateRange: { start: '2026-03-20', end: '2026-04-20' },
      language: 'zh-TW',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 120,
      t,
    }),
  ).toBe('120 visits from 2026年 3月20日 to 4月20日')
})

test('extractPointYear ignores malformed date keys', () => {
  expect(extractPointYear('2026-01-03')).toBe(2026)
  expect(extractPointYear('not-a-date')).toBeNull()
})
