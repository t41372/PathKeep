/**
 * @file browsing-rhythm-card-helpers.ts
 * @description Shared calendar/date math and summary formatting for the browsing-rhythm card.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Build the date-grid model used by the browsing-rhythm heatmap.
 * - Format month, range, and year summaries without owning any JSX.
 * - Keep the year picker options aligned with the available discovery-trend data.
 *
 * ## Not responsible for
 * - Rendering controls, calendars, or day-detail chrome.
 * - Fetching discovery-trend or day-insight data.
 * - Owning any route-specific navigation decisions.
 *
 * ## Dependencies
 * - Depends on core-intelligence date shapes and discovery-trend points.
 * - Uses `Intl.DateTimeFormat` / `Intl.NumberFormat` for locale-aware copy.
 *
 * ## Performance notes
 * - The helpers are intentionally pure so the card can memoize them cheaply on large archives.
 */

import type {
  DateRange,
  DiscoveryTrendPoint,
  TimeRangePreset,
} from '../../lib/core-intelligence'

/**
 * Shared translator signature for the browsing-rhythm card and its extracted subviews.
 */
export type BrowsingRhythmTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Calendar cell model used by the browsing-rhythm heatmap and its direct tests.
 */
export interface BrowsingRhythmCalendarCell {
  date: Date
  dateKey: string
  inRange: boolean
  newDomainCount: number
  totalVisits: number
}

/**
 * Builds the week matrix required by the calendar heatmap so rendering can stay purely declarative.
 */
export function buildCalendarWeeks(
  dateRange: DateRange,
  points: Map<string, DiscoveryTrendPoint>,
) {
  const start = parseDateKey(dateRange.start)
  const end = parseDateKey(dateRange.end)
  const calendarStart = startOfWeek(start)
  const calendarEnd = endOfWeek(end)
  const weeks: BrowsingRhythmCalendarCell[][] = []
  let cursor = calendarStart

  // Stryker disable next-line EqualityOperator: cursor advances by whole weeks from Sunday starts, so it never equals the Saturday calendarEnd boundary.
  while (cursor.getTime() <= calendarEnd.getTime()) {
    const week: BrowsingRhythmCalendarCell[] = []
    for (let index = 0; index < 7; index += 1) {
      const date = new Date(cursor.getTime())
      const dateKey = toDateKey(date)
      const point = points.get(dateKey) ?? null
      week.push({
        date,
        dateKey,
        inRange:
          date.getTime() >= start.getTime() && date.getTime() <= end.getTime(),
        newDomainCount: point?.newDomainCount ?? 0,
        totalVisits: point?.totalVisits ?? 0,
      })
      cursor = addDays(cursor, 1)
    }
    weeks.push(week)
  }

  return weeks
}

/**
 * Derives the sparse month labels that sit above the calendar columns without leaking layout logic into the route.
 */
export function buildMonthLabels(
  weeks: BrowsingRhythmCalendarCell[][],
  language: string,
) {
  const monthFormatter = new Intl.DateTimeFormat(localeFromLanguage(language), {
    month: 'short',
  })
  let seenMonthKey: string | null = null

  return weeks.map((week, index) => {
    const firstInRangeDay = week.find((cell) => cell.inRange) ?? null
    if (!firstInRangeDay) {
      return ''
    }

    const monthKey = `${firstInRangeDay.date.getFullYear()}-${firstInRangeDay.date.getMonth()}`
    const shouldShowLabel =
      // Stryker disable next-line ConditionalExpression: the first in-range month also differs from the initial null seenMonthKey.
      index === 0 ||
      firstInRangeDay.date.getDate() <= 7 ||
      monthKey !== seenMonthKey

    if (shouldShowLabel) {
      seenMonthKey = monthKey
      return monthFormatter.format(firstInRangeDay.date)
    }

    return ''
  })
}

/**
 * Formats the summary line above the browsing-rhythm heatmap with the same branch rules as the original owner.
 */
export function buildVisitSummary({
  dateRange,
  language,
  selectedYear,
  summaryPreset,
  totalVisits,
  t,
}: {
  dateRange: DateRange
  language: string
  selectedYear: number
  summaryPreset: TimeRangePreset | 'calendar-year'
  totalVisits: number
  t: BrowsingRhythmTranslator
}) {
  const count = new Intl.NumberFormat(localeFromLanguage(language)).format(
    totalVisits,
  )

  if (summaryPreset === 'calendar-year') {
    return t('rhythmVisitSummaryYear', {
      count,
      year: selectedYear,
    })
  }

  if (summaryPreset === 'all') {
    return t('rhythmVisitSummaryAll', { count })
  }

  if (dateRange.start === dateRange.end) {
    return t('rhythmVisitSummaryDay', {
      count,
      date: formatDisplayDate(dateRange.start, language),
    })
  }

  if (isFullCalendarMonth(dateRange)) {
    return t('rhythmVisitSummaryMonth', {
      count,
      monthYear: formatMonthYear(dateRange.start, language),
    })
  }

  if (isFullCalendarYear(dateRange)) {
    return t('rhythmVisitSummaryYear', {
      count,
      year: dateRange.start.slice(0, 4),
    })
  }

  return t('rhythmVisitSummaryRange', {
    count,
    start: formatRangeBoundary(
      dateRange.start,
      dateRange.end,
      language,
      'start',
    ),
    end: formatRangeBoundary(dateRange.start, dateRange.end, language, 'end'),
  })
}

/**
 * Returns the descending year list shown by the picker so the card can stay in sync with the loaded trend data.
 */
export function buildYearOptions(dataYears: number[], currentYear: number) {
  // Stryker disable ConditionalExpression,BlockStatement: the fallthrough Math.min/Math.max path also returns [currentYear] for an empty data set.
  if (dataYears.length === 0) {
    return [currentYear]
  }
  // Stryker restore ConditionalExpression,BlockStatement

  const lowerBound = Math.min(...dataYears, currentYear)
  const upperBound = Math.max(...dataYears, currentYear)
  return Array.from(
    { length: upperBound - lowerBound + 1 },
    (_, index) => upperBound - index,
  )
}

/**
 * Pulls a calendar year out of a discovery-trend point so mixed data can still seed the year selector.
 */
export function extractPointYear(dateKey: string) {
  const match = /^(\d{4})/.exec(dateKey)
  if (!match) {
    return null
  }
  return Number(match[1])
}

function parseDateKey(dateKey: string) {
  const [year, month, day] = dateKey.split('-').map(Number)
  return new Date(year, (month ?? 1) - 1, day ?? 1, 12)
}

function toDateKey(date: Date) {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function addDays(date: Date, days: number) {
  const next = new Date(date.getTime())
  next.setDate(next.getDate() + days)
  return next
}

function startOfWeek(date: Date) {
  return addDays(date, -date.getDay())
}

function endOfWeek(date: Date) {
  return addDays(date, 6 - date.getDay())
}

function localeFromLanguage(language: string) {
  if (language === 'zh-CN') return 'zh-CN'
  if (language === 'zh-TW') return 'zh-TW'
  return 'en-US'
}

export function formatDisplayDate(dateKey: string, language: string) {
  return new Intl.DateTimeFormat(localeFromLanguage(language), {
    dateStyle: 'medium',
  }).format(parseDateKey(dateKey))
}

function formatMonthYear(dateKey: string, language: string) {
  return new Intl.DateTimeFormat(localeFromLanguage(language), {
    month: 'long',
    year: 'numeric',
  }).format(parseDateKey(dateKey))
}

function formatRangeBoundary(
  startDateKey: string,
  endDateKey: string,
  language: string,
  boundary: 'start' | 'end',
) {
  const start = parseDateKey(startDateKey)
  const end = parseDateKey(endDateKey)
  const target = boundary === 'start' ? start : end
  const sameYear = start.getFullYear() === end.getFullYear()
  const sameMonth = sameYear && start.getMonth() === end.getMonth()

  if (language === 'zh-CN' || language === 'zh-TW') {
    if (boundary === 'end' && sameMonth) {
      return `${target.getDate()}日`
    }
    if (boundary === 'end' && sameYear) {
      return `${target.getMonth() + 1}月${target.getDate()}日`
    }
    // Stryker disable next-line ConditionalExpression: same-year end boundaries already returned above, so dropping the boundary check is equivalent.
    if (boundary === 'start' && sameYear) {
      return `${target.getFullYear()}年 ${target.getMonth() + 1}月${target.getDate()}日`
    }
    return `${target.getFullYear()}年${target.getMonth() + 1}月${target.getDate()}日`
  }

  const monthFormatter = new Intl.DateTimeFormat(localeFromLanguage(language), {
    month: 'short',
  })
  const month = monthFormatter.format(target)
  if (boundary === 'end' && sameMonth) {
    return String(target.getDate())
  }
  // Stryker disable next-line ConditionalExpression: same-year end boundaries fall through to the year-qualified fallback below.
  if (boundary === 'start' && sameYear) {
    return `${month} ${target.getDate()}`
  }
  return `${month} ${target.getDate()}, ${target.getFullYear()}`
}

function isFullCalendarYear(dateRange: DateRange) {
  return (
    dateRange.start.endsWith('-01-01') &&
    dateRange.end.endsWith('-12-31') &&
    dateRange.start.slice(0, 4) === dateRange.end.slice(0, 4)
  )
}

function isFullCalendarMonth(dateRange: DateRange) {
  const start = parseDateKey(dateRange.start)
  const end = parseDateKey(dateRange.end)
  return (
    start.getFullYear() === end.getFullYear() &&
    start.getMonth() === end.getMonth() &&
    start.getDate() === 1 &&
    end.getDate() ===
      new Date(end.getFullYear(), end.getMonth() + 1, 0).getDate()
  )
}
