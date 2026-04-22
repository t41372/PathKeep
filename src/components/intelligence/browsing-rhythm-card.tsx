/**
 * @file browsing-rhythm-card.tsx
 * @description Shared calendar-based browsing rhythm card used by Dashboard and `/intelligence`.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Own the selected-year and selected-day state for the shared browsing-rhythm surface.
 * - Fetch and memoize the discovery-trend and day-insight payloads needed by the card.
 * - Compose the extracted year controls, calendar grid, and day-detail renderers.
 *
 * ## Not responsible for
 * - Implementing the shared day-detail primitives or the discovery-trend math helpers.
 * - Defining route-level navigation destinations.
 * - Owning the `/intelligence` or dashboard section chrome around the card.
 *
 * ## Dependencies
 * - Depends on core-intelligence data hooks and the shared browsing-rhythm subcomponents.
 * - Relies on the locale-aware summary and calendar helpers in the sibling helper module.
 *
 * ## Performance notes
 * - Keeps the heavy date-grid derivation in memoized helpers so the render tree stays cheap on large archives.
 */

import './browsing-rhythm-card.css'

import { useEffect, useMemo, useState } from 'react'
import {
  BrowsingRhythmCalendarGrid,
  BrowsingRhythmYearControls,
} from './browsing-rhythm-calendar'
import {
  buildCalendarWeeks,
  buildMonthLabels,
  buildVisitSummary,
  buildYearOptions,
  extractPointYear,
  type BrowsingRhythmTranslator as BrowsingRhythmTranslatorType,
} from './browsing-rhythm-card-helpers'
import { BrowsingRhythmDayDetail } from './browsing-rhythm-day-detail'
import * as api from '../../lib/core-intelligence/api'
import {
  dateRangeForCalendarYear,
  useAsyncData,
  type DateRange,
  type TimeRangePreset,
} from '../../lib/core-intelligence'

/**
 * Shared translator signature for the browsing-rhythm card and its extracted subviews.
 */
export type BrowsingRhythmTranslator = BrowsingRhythmTranslatorType

interface BrowsingRhythmCardProps {
  dateRange?: DateRange
  dayDomainHref?: (domain: string, date: string) => string
  dayHref: (date: string) => string
  language: string
  mode: 'range' | 'year'
  profileId?: string | null
  showCurrentYearShortcut?: boolean
  summaryPreset?: TimeRangePreset | 'calendar-year'
  t: BrowsingRhythmTranslator
  yearNavigation?: 'select' | 'pager'
}

/**
 * Reusable calendar-based browsing rhythm card shared by Dashboard and
 * `/intelligence`.
 *
 * The accepted contract is a real-date calendar heatmap backed by
 * `getDiscoveryTrend(..., 'day')`. Selecting a day keeps the user in context
 * and lazy-loads a compact inline preview; navigation to the first-class day
 * insights route only happens through the explicit detail CTA.
 */
export function BrowsingRhythmCard({
  dateRange,
  dayDomainHref,
  dayHref,
  language,
  mode,
  profileId,
  showCurrentYearShortcut = false,
  summaryPreset,
  t,
  yearNavigation = 'select',
}: BrowsingRhythmCardProps) {
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
    () => api.getDiscoveryTrend(effectiveDateRange, profileId, 'day'),
    [effectiveDateRange.start, effectiveDateRange.end, mode, profileId],
    {
      getCached: () =>
        api.peekDiscoveryTrend(effectiveDateRange, profileId, 'day'),
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
  const weekdayLabels = [
    t('dow_sun'),
    t('dow_mon'),
    t('dow_tue'),
    t('dow_wed'),
    t('dow_thu'),
    t('dow_fri'),
    t('dow_sat'),
  ]
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

  return (
    <div className="browsing-rhythm-card">
      <BrowsingRhythmYearControls
        canResetToCurrentYear={canResetToCurrentYear}
        mode={mode}
        newerYear={newerYear}
        olderYear={olderYear}
        onResetToCurrentYear={() => {
          setManualYearSelection(true)
          setSelectedYear(currentCalendarYear)
        }}
        onSelectYear={(year) => {
          setManualYearSelection(true)
          setSelectedYear(year)
        }}
        selectedYear={selectedYear}
        t={t}
        yearNavigation={yearNavigation}
        yearOptions={yearOptions}
      />

      {trendResult.loading || waitingForYearRealignment ? (
        <div className="browsing-rhythm-card__skeleton" />
      ) : trendResult.error ? (
        <div className="browsing-rhythm-card__empty">
          <p className="browsing-rhythm-card__empty-text">
            {trendResult.error}
          </p>
        </div>
      ) : mode !== 'year' && !hasCalendarVisits ? (
        <div className="browsing-rhythm-card__empty">
          <p className="browsing-rhythm-card__empty-text">{t('rhythmEmpty')}</p>
        </div>
      ) : (
        <>
          <p
            className="browsing-rhythm-card__summary"
            data-testid="browsing-rhythm-summary"
          >
            {visitSummary}
          </p>
          <BrowsingRhythmCalendarGrid
            calendarWeeks={calendarWeeks}
            maxVisits={maxVisits}
            monthLabels={monthLabels}
            onSelectDay={(dateKey) => {
              setSelectedDateState({
                dateKey,
                scopeKey: selectionScopeKey,
              })
            }}
            selectedDateKey={selectedDay?.dateKey ?? null}
            t={t}
            weekdayLabels={weekdayLabels}
          />

          {selectedDay ? (
            <BrowsingRhythmDayDetail
              dateKey={selectedDay.dateKey}
              dayDomainHref={dayDomainHref}
              dayHref={dayHref}
              detail={selectedDayResult.data?.data ?? null}
              error={selectedDayResult.error}
              language={language}
              loading={selectedDayResult.loading}
              t={t}
            />
          ) : calendarDays.some((cell) => cell.totalVisits > 0) ? (
            <div className="browsing-rhythm-card__empty browsing-rhythm-card__empty--prompt">
              <p className="browsing-rhythm-card__empty-text">
                {t('rhythmSelectDayPrompt')}
              </p>
            </div>
          ) : null}
        </>
      )}
    </div>
  )
}
