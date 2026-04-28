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
  formatDisplayDate,
} from './browsing-rhythm-card-helpers'
import type {
  BrowsingRhythmCalendarCell,
  BrowsingRhythmTranslator,
} from './browsing-rhythm-card-helpers'

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
    case 'rhythmVisitSummaryAll':
      return `${vars?.count} visits across all history`
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

test('buildCalendarWeeks includes a week when the range starts on the calendar week boundary', () => {
  const weeks = buildCalendarWeeks(
    { start: '2026-01-04', end: '2026-01-04' },
    new Map(),
  )

  expect(weeks).toHaveLength(1)
  expect(weeks[0].map((cell) => cell.dateKey)).toEqual([
    '2026-01-04',
    '2026-01-05',
    '2026-01-06',
    '2026-01-07',
    '2026-01-08',
    '2026-01-09',
    '2026-01-10',
  ])
  expect(weeks[0].filter((cell) => cell.inRange)).toHaveLength(1)
})

test('buildCalendarWeeks tolerates year-only fallback date keys', () => {
  const weeks = buildCalendarWeeks(
    { start: '2026', end: '2026' },
    new Map([
      [
        '2026-01-01',
        {
          dateKey: '2026-01-01',
          discoveryRate: 0.5,
          newDomainCount: 3,
          totalVisits: 12,
        },
      ],
    ]),
  )

  const inRangeCells = weeks.flat().filter((cell) => cell.inRange)
  expect(inRangeCells).toHaveLength(1)
  expect(inRangeCells[0]).toMatchObject({
    dateKey: '2026-01-01',
    newDomainCount: 3,
    totalVisits: 12,
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
  expect(buildMonthLabels([emptyWeek()], 'en')).toEqual([''])
})

test('buildMonthLabels repeats early-month labels and relabels month changes away from month start', () => {
  expect(
    buildMonthLabels(
      [
        [calendarCell('2026-01-01', true)],
        [calendarCell('2026-01-07', true)],
        [calendarCell('2026-01-20', true)],
        [calendarCell('2026-02-08', true)],
        [calendarCell('2026-02-15', true)],
      ],
      'en',
    ),
  ).toEqual(['Jan', 'Jan', '', 'Feb', ''])
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
      dateRange: { start: '2025-01-01', end: '2025-12-31' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'calendar-year',
      totalVisits: 120,
      t,
    }),
  ).toBe('120 visits in 2026')

  expect(
    buildVisitSummary({
      dateRange: { start: '1900-01-01', end: '2026-04-25' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'all',
      totalVisits: 480,
      t,
    }),
  ).toBe('480 visits across all history')

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

  expect(
    buildVisitSummary({
      dateRange: { start: '2026-04-25', end: '2026-04-25' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 12,
      t,
    }),
  ).toBe('12 visits on Apr 25, 2026')

  expect(
    buildVisitSummary({
      dateRange: { start: '2026-04-01', end: '2026-04-30' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 30,
      t,
    }),
  ).toBe('30 visits in April 2026')

  expect(
    buildVisitSummary({
      dateRange: { start: '2025-01-01', end: '2025-12-31' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 365,
      t,
    }),
  ).toBe('365 visits in 2025')

  expect(
    buildVisitSummary({
      dateRange: { start: '2026-04-20', end: '2026-04-25' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 6,
      t,
    }),
  ).toBe('6 visits from Apr 20 to 25')

  expect(
    buildVisitSummary({
      dateRange: { start: '2026-03-20', end: '2026-04-20' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 32,
      t,
    }),
  ).toBe('32 visits from Mar 20 to Apr 20, 2026')

  expect(
    buildVisitSummary({
      dateRange: { start: '2026-04-20', end: '2026-04-25' },
      language: 'zh-TW',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 6,
      t,
    }),
  ).toBe('6 visits from 2026年 4月20日 to 25日')

  expect(
    buildVisitSummary({
      dateRange: { start: '2025-12-30', end: '2026-01-02' },
      language: 'zh-CN',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 4,
      t,
    }),
  ).toBe('4 visits from 2025年12月30日 to 2026年1月2日')

  expect(
    buildVisitSummary({
      dateRange: { start: '2025-12-30', end: '2026-01-02' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 4,
      t,
    }),
  ).toBe('4 visits from Dec 30, 2025 to Jan 2, 2026')

  expect(
    buildVisitSummary({
      dateRange: { start: '2025-01-02', end: '2025-12-31' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 364,
      t,
    }),
  ).toBe('364 visits from Jan 2 to Dec 31, 2025')

  expect(
    buildVisitSummary({
      dateRange: { start: '2025-01-01', end: '2025-12-30' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 364,
      t,
    }),
  ).toBe('364 visits from Jan 1 to Dec 30, 2025')

  expect(
    buildVisitSummary({
      dateRange: { start: '2025-01-01', end: '2026-12-31' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 730,
      t,
    }),
  ).toBe('730 visits from Jan 1, 2025 to Dec 31, 2026')

  expect(
    buildVisitSummary({
      dateRange: { start: '2026-04-02', end: '2026-04-30' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 29,
      t,
    }),
  ).toBe('29 visits from Apr 2 to 30')

  expect(
    buildVisitSummary({
      dateRange: { start: '2026-04-01', end: '2026-04-29' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 29,
      t,
    }),
  ).toBe('29 visits from Apr 1 to 29')

  expect(
    buildVisitSummary({
      dateRange: { start: '2025-04-01', end: '2026-04-30' },
      language: 'en',
      selectedYear: 2026,
      summaryPreset: 'custom',
      totalVisits: 395,
      t,
    }),
  ).toBe('395 visits from Apr 1, 2025 to Apr 30, 2026')
})

test('extractPointYear ignores malformed date keys', () => {
  expect(extractPointYear('2026-01-03')).toBe(2026)
  expect(extractPointYear('prefix-2026-01-03')).toBeNull()
  expect(extractPointYear('not-a-date')).toBeNull()
})

test('formatDisplayDate preserves supported locale-specific date formatting', () => {
  expect(formatDisplayDate('2026-04-25', 'en')).toBe('Apr 25, 2026')
  expect(formatDisplayDate('2026-04-25', 'zh-CN')).toBe('2026年4月25日')
  expect(formatDisplayDate('2026-04-25', 'zh-TW')).toBe('2026年4月25日')
})

function emptyWeek(): BrowsingRhythmCalendarCell[] {
  return [
    {
      date: new Date('2026-01-01T00:00:00.000Z'),
      dateKey: '2026-01-01',
      inRange: false,
      newDomainCount: 0,
      totalVisits: 0,
    },
  ]
}

function calendarCell(
  dateKey: string,
  inRange: boolean,
): BrowsingRhythmCalendarCell {
  return {
    date: new Date(`${dateKey}T12:00:00.000Z`),
    dateKey,
    inRange,
    newDomainCount: 0,
    totalVisits: 0,
  }
}
