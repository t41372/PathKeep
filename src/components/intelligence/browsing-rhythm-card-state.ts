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

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
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
  const [selectedYearState, setSelectedYear] = useState(
    () => currentCalendarYear,
  )
  const [manualYearSelection, setManualYearSelection] = useState(false)
  const [knownDataYears, setKnownDataYears] = useState<number[]>([])
  const selectedYear = manualYearSelection
    ? selectedYearState
    : currentCalendarYear
  const effectiveDateRange = useMemo(() => {
    if (mode === 'year') {
      return dateRangeForCalendarYear(selectedYear)
    }
    return dateRange ?? dateRangeForCalendarYear(selectedYear)
  }, [dateRange, mode, selectedYear])
  const trendRangeKey = `${mode}:${effectiveDateRange.start}:${effectiveDateRange.end}:${
    // Stryker disable next-line StringLiteral: this sentinel is internal only; tests assert null and undefined both stay in the same archive-wide scope.
    profileId ?? 'archive-wide'
  }`
  const lastRefreshTokenRef = useRef(refreshToken)
  const [forceRefreshRequest, setForceRefreshRequest] = useState<{
    nonce: number
    rangeKey: string
  } | null>(null)
  useEffect(() => {
    if (refreshToken === lastRefreshTokenRef.current) {
      return
    }
    lastRefreshTokenRef.current = refreshToken
    if (refreshToken === null || refreshToken === undefined) {
      return
    }
    // Stryker disable ConditionalExpression,BlockStatement,BooleanLiteral: this only suppresses an unmounted async state write; React exposes no observable state for it.
    let cancelled = false
    queueMicrotask(() => {
      if (cancelled) {
        return
      }
      setForceRefreshRequest((current) => ({
        // Stryker disable next-line ArithmeticOperator: refresh only requires a changed nonce; increasing versus decreasing is not observable.
        nonce: (current?.nonce ?? 0) + 1,
        rangeKey: trendRangeKey,
      }))
    })
    return () => {
      cancelled = true
    }
    // Stryker restore ConditionalExpression,BlockStatement,BooleanLiteral
  }, [refreshToken, trendRangeKey])
  const forceTrendRefresh = forceRefreshRequest?.rangeKey === trendRangeKey
  const selectionScopeKey = `${mode}:${effectiveDateRange.start}:${effectiveDateRange.end}:${
    // Stryker disable next-line StringLiteral: this sentinel is internal only; tests assert null and undefined both stay in the same archive-wide scope.
    profileId ?? 'archive-wide'
  }`
  const [selectedDateState, setSelectedDateState] = useState<{
    dateKey: string
    scopeKey: string
  } | null>(null)
  const trendResult = useAsyncData(
    () =>
      api.getDiscoveryTrend(
        effectiveDateRange,
        profileId,
        'day',
        forceTrendRefresh ? { force: true } : undefined,
      ),
    [
      effectiveDateRange.start,
      effectiveDateRange.end,
      mode,
      profileId,
      forceTrendRefresh,
      forceRefreshRequest?.nonce,
    ],
    {
      getCached: () =>
        !forceTrendRefresh
          ? api.peekDiscoveryTrend(effectiveDateRange, profileId, 'day')
          : null,
    },
  )
  const points = useMemo(
    () => trendResult.data?.data.points ?? [],
    [trendResult.data],
  )
  const loadedDataYears = useMemo(
    () =>
      Array.from(
        new Set([
          ...(trendResult.data?.data.availableYears ?? []),
          ...points
            .map((point) => extractPointYear(point.dateKey))
            .filter((year): year is number => year !== null),
        ]),
      ),
    [points, trendResult.data],
  )
  useEffect(() => {
    // Stryker disable next-line ConditionalExpression,BlockStatement: this avoids a no-op state update; empty known-year updates are performance-only.
    if (loadedDataYears.length === 0) {
      return
    }
    let cancelled = false
    queueMicrotask(() => {
      // Stryker disable next-line ConditionalExpression,BlockStatement: React drops unmounted state updates, but the guard avoids scheduling them.
      if (cancelled) {
        return
      }
      setKnownDataYears((current) =>
        Array.from(new Set([...current, ...loadedDataYears])),
      )
    })
    // Stryker disable BlockStatement,BooleanLiteral: cleanup only prevents an unmounted async state write; React exposes no runtime state to assert.
    return () => {
      cancelled = true
    }
    // Stryker restore BlockStatement,BooleanLiteral
  }, [loadedDataYears])
  const dataYears = useMemo(
    () =>
      Array.from(
        new Set([...knownDataYears, ...loadedDataYears, selectedYear]),
      ),
    [knownDataYears, loadedDataYears, selectedYear],
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
  // Stryker disable ConditionalExpression: year mode always builds a full selected calendar-year range, so both fallback paths format the same year summary.
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
  // Stryker restore ConditionalExpression
  const visibleRangeHint = useMemo(() => {
    if (mode !== 'year') {
      return null
    }

    const occupiedDays = calendarDays.filter((cell) => cell.totalVisits > 0)
    if (occupiedDays.length === 0) {
      return null
    }

    const start = occupiedDays[0].dateKey
    const end = occupiedDays[occupiedDays.length - 1].dateKey

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
  const waitingForYearRealignment = false
  const selectedYearIndex = yearOptions.indexOf(selectedYear)
  const newerYear =
    selectedYearIndex > 0 ? yearOptions[selectedYearIndex - 1] : null
  const olderYear =
    selectedYearIndex < yearOptions.length - 1
      ? yearOptions[selectedYearIndex + 1]
      : null
  const canResetToCurrentYear =
    showCurrentYearShortcut &&
    mode === 'year' &&
    selectedYear !== currentCalendarYear

  // Stryker disable ArrayDeclaration: these callbacks either use stable React setters or clear manual mode before selectedYearState can matter.
  const selectYear = useCallback((year: number) => {
    setManualYearSelection(true)
    setSelectedYear(year)
  }, [])

  const resetToCurrentYear = useCallback(() => {
    setManualYearSelection(false)
    setSelectedYear(currentCalendarYear)
  }, [currentCalendarYear])
  // Stryker restore ArrayDeclaration

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
