/**
 * @file browsing-rhythm-card-state.ts
 * @description Data and interaction state owner for the shared browsing-rhythm card.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Load the discovery-trend and selected-day preview payloads used by the shared card.
 * - Own selected-year realignment, manual year navigation, and selected-day scoping.
 * - Derive the memoized calendar, summary, range hint, and navigation state consumed by the render shell.
 *
 * ## Not responsible for
 * - Rendering the browsing-rhythm calendar, controls, or selected-day detail UI.
 * - Defining route destinations for day/domain drilldowns.
 * - Owning route-level section chrome or Intelligence/Dashboard page layout.
 *
 * ## Dependencies
 * - Depends on Core Intelligence API readers and date-range helpers.
 * - Depends on the pure browsing-rhythm calendar/summary helpers for bounded derived data.
 *
 * ## Performance notes
 * - All O(days-in-range) calendar derivation stays memoized; no history rows are loaded client-side.
 * - Selected-day detail is lazy and only requested after the user clicks a populated calendar day.
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import * as api from '../../lib/core-intelligence/api'
import {
  dateRangeForCalendarYear,
  useAsyncData,
  type CoreIntelligenceSectionMeta,
  type DateRange,
  type DayInsights,
  type TimeRangePreset,
} from '../../lib/core-intelligence'
import {
  buildCalendarWeeks,
  formatDisplayDate,
  buildMonthLabels,
  buildVisitSummary,
  buildYearOptions,
  extractPointYear,
  type BrowsingRhythmCalendarCell,
  type BrowsingRhythmTranslator,
} from './browsing-rhythm-card-helpers'

/**
 * Input contract for the shared browsing-rhythm card state hook.
 *
 * The route decides the visible date range, profile scope, copy function, and whether
 * the card behaves as a fixed calendar-year dashboard widget or a custom-range Intelligence card.
 */
export interface BrowsingRhythmCardStateOptions {
  dateRange?: DateRange
  language: string
  mode: 'range' | 'year'
  onTrendMetaChange?: (meta: CoreIntelligenceSectionMeta | null) => void
  profileId?: string | null
  refreshToken?: number | string | null
  showCurrentYearShortcut: boolean
  summaryPreset?: TimeRangePreset | 'calendar-year'
  t: BrowsingRhythmTranslator
}

/**
 * Render-ready browsing-rhythm state returned to the card shell.
 *
 * This keeps API status, derived calendar models, selected-day preview data, and year-navigation
 * commands together so the JSX owner can remain a composition layer.
 */
export interface BrowsingRhythmCardState {
  calendarDays: BrowsingRhythmCalendarCell[]
  calendarWeeks: BrowsingRhythmCalendarCell[][]
  canResetToCurrentYear: boolean
  hasCalendarVisits: boolean
  maxVisits: number
  monthLabels: string[]
  newerYear: number | null
  olderYear: number | null
  resetToCurrentYear: () => void
  selectDay: (dateKey: string) => void
  selectYear: (year: number) => void
  selectedDay: BrowsingRhythmCalendarCell | null
  selectedDayDetail: DayInsights | null
  selectedDayError: string | null
  selectedDayLoading: boolean
  selectedYear: number
  trendError: string | null
  trendLoading: boolean
  visibleRangeHint: string | null
  visitSummary: string
  waitingForYearRealignment: boolean
  weekdayLabels: string[]
  yearOptions: number[]
}

/**
 * Builds the state model for the shared browsing-rhythm card without owning any JSX.
 *
 * Dashboard uses this in `year` mode so empty years stay browsable and current-year reset remains
 * explicit. `/intelligence` uses the same hook in `range` mode so custom date ranges reuse the
 * calendar and lazy selected-day preview contract.
 */
export function useBrowsingRhythmCardState({
  dateRange,
  language,
  mode,
  onTrendMetaChange,
  profileId,
  refreshToken,
  showCurrentYearShortcut,
  summaryPreset,
  t,
}: BrowsingRhythmCardStateOptions): BrowsingRhythmCardState {
  const currentCalendarYear = new Date().getFullYear()
  const [selectedYear, setSelectedYear] = useState(() => currentCalendarYear)
  const [manualYearSelection, setManualYearSelection] = useState(false)
  const effectiveDateRange = useMemo(() => {
    if (mode === 'year') {
      return dateRangeForCalendarYear(selectedYear)
    }
    return dateRange ?? dateRangeForCalendarYear(selectedYear)
  }, [dateRange, mode, selectedYear])
  const selectionScopeKey = `${mode}:${effectiveDateRange.start}:${effectiveDateRange.end}:${
    profileId ?? 'archive-wide'
  }`
  const [selectedDateState, setSelectedDateState] = useState<{
    dateKey: string
    scopeKey: string
  } | null>(null)
  const trendResult = useAsyncData(
    () =>
      api.getDiscoveryTrend(effectiveDateRange, profileId, 'day', {
        force: refreshToken !== null && refreshToken !== undefined,
      }),
    [
      effectiveDateRange.start,
      effectiveDateRange.end,
      mode,
      profileId,
      refreshToken,
    ],
    {
      getCached: () =>
        refreshToken === null || refreshToken === undefined
          ? api.peekDiscoveryTrend(effectiveDateRange, profileId, 'day')
          : null,
    },
  )
  const points = useMemo(
    () => trendResult.data?.data.points ?? [],
    [trendResult.data],
  )
  const dataYears = useMemo(
    () =>
      Array.from(
        new Set([
          ...(trendResult.data?.data.availableYears ?? []),
          ...points
            .map((point) => extractPointYear(point.dateKey))
            .filter((year): year is number => year !== null),
        ]),
      ).sort((left, right) => right - left),
    [points, trendResult.data],
  )
  const yearOptions = useMemo(
    () => buildYearOptions(dataYears, currentCalendarYear),
    [currentCalendarYear, dataYears],
  )
  const pointByDate = useMemo(
    () => new Map(points.map((point) => [point.dateKey, point])),
    [points],
  )
  const calendarWeeks = useMemo(
    () => buildCalendarWeeks(effectiveDateRange, pointByDate),
    [effectiveDateRange, pointByDate],
  )
  const calendarDays = useMemo(
    () => calendarWeeks.flat().filter((cell) => cell.inRange),
    [calendarWeeks],
  )
  const monthLabels = useMemo(
    () => buildMonthLabels(calendarWeeks, language),
    [calendarWeeks, language],
  )
  const maxVisits = useMemo(
    () => Math.max(...calendarDays.map((cell) => cell.totalVisits), 1),
    [calendarDays],
  )
  const totalVisits = useMemo(
    () => calendarDays.reduce((sum, cell) => sum + cell.totalVisits, 0),
    [calendarDays],
  )
  const hasCalendarVisits = totalVisits > 0
  const visitSummary = useMemo(
    () =>
      buildVisitSummary({
        dateRange: effectiveDateRange,
        language,
        selectedYear,
        summaryPreset:
          summaryPreset ?? (mode === 'year' ? 'calendar-year' : 'custom'),
        totalVisits,
        t,
      }),
    [
      effectiveDateRange,
      language,
      mode,
      selectedYear,
      summaryPreset,
      t,
      totalVisits,
    ],
  )
  const visibleRangeHint = useMemo(() => {
    if (mode !== 'year') {
      return null
    }

    const occupiedDays = calendarDays.filter((cell) => cell.totalVisits > 0)
    if (occupiedDays.length === 0) {
      return null
    }

    const start = occupiedDays[0]?.dateKey ?? null
    const end = occupiedDays[occupiedDays.length - 1]?.dateKey ?? null
    if (!start || !end) {
      return null
    }

    if (start === `${selectedYear}-01-01` && end === `${selectedYear}-12-31`) {
      return null
    }

    return t('rhythmVisibleRange', {
      start: formatDisplayDate(start, language),
      end: formatDisplayDate(end, language),
    })
  }, [calendarDays, language, mode, selectedYear, t])
  const selectedDateOverride =
    selectedDateState?.scopeKey === selectionScopeKey
      ? selectedDateState.dateKey
      : null
  const selectedDay = useMemo(() => {
    if (!selectedDateOverride) {
      return null
    }

    return (
      calendarDays.find((cell) => cell.dateKey === selectedDateOverride) ?? null
    )
  }, [calendarDays, selectedDateOverride])
  const selectedDayResult = useAsyncData(
    () =>
      selectedDay
        ? api.getDayInsights(selectedDay.dateKey, profileId)
        : Promise.resolve(null),
    [profileId, selectedDay?.dateKey],
  )

  useEffect(() => {
    onTrendMetaChange?.(trendResult.data?.meta ?? null)
  }, [onTrendMetaChange, trendResult.data?.meta])

  const weekdayLabels = useMemo(
    () => [
      t('dow_sun'),
      t('dow_mon'),
      t('dow_tue'),
      t('dow_wed'),
      t('dow_thu'),
      t('dow_fri'),
      t('dow_sat'),
    ],
    [t],
  )
  const waitingForYearRealignment =
    mode === 'year' &&
    yearOptions.length > 0 &&
    !yearOptions.includes(selectedYear)
  const selectedYearIndex = yearOptions.indexOf(selectedYear)
  const newerYear =
    selectedYearIndex > 0 ? yearOptions[selectedYearIndex - 1] : null
  const olderYear =
    selectedYearIndex >= 0 && selectedYearIndex < yearOptions.length - 1
      ? yearOptions[selectedYearIndex + 1]
      : null
  const canResetToCurrentYear =
    showCurrentYearShortcut &&
    mode === 'year' &&
    yearOptions.includes(currentCalendarYear) &&
    selectedYear !== currentCalendarYear

  useEffect(() => {
    if (mode !== 'year' || yearOptions.length === 0) {
      return
    }

    const hasSelectedYear = yearOptions.includes(selectedYear)
    const nextYear =
      manualYearSelection && hasSelectedYear
        ? selectedYear
        : currentCalendarYear

    if (nextYear === selectedYear && hasSelectedYear) {
      return
    }

    let cancelled = false
    queueMicrotask(() => {
      if (cancelled) {
        return
      }
      setSelectedYear(nextYear)
      if (!hasSelectedYear) {
        setManualYearSelection(false)
      }
    })

    return () => {
      cancelled = true
    }
  }, [
    currentCalendarYear,
    manualYearSelection,
    mode,
    selectedYear,
    yearOptions,
  ])

  const selectYear = useCallback((year: number) => {
    setManualYearSelection(true)
    setSelectedYear(year)
  }, [])

  const resetToCurrentYear = useCallback(() => {
    setManualYearSelection(true)
    setSelectedYear(currentCalendarYear)
  }, [currentCalendarYear])

  const selectDay = useCallback(
    (dateKey: string) => {
      setSelectedDateState({
        dateKey,
        scopeKey: selectionScopeKey,
      })
    },
    [selectionScopeKey],
  )

  return {
    calendarDays,
    calendarWeeks,
    canResetToCurrentYear,
    hasCalendarVisits,
    maxVisits,
    monthLabels,
    newerYear,
    olderYear,
    resetToCurrentYear,
    selectDay,
    selectYear,
    selectedDay,
    selectedDayDetail: selectedDayResult.data?.data ?? null,
    selectedDayError: selectedDayResult.error,
    selectedDayLoading: selectedDayResult.loading,
    selectedYear,
    trendError: trendResult.error,
    trendLoading: trendResult.loading,
    visibleRangeHint,
    visitSummary,
    waitingForYearRealignment,
    weekdayLabels,
    yearOptions,
  }
}
