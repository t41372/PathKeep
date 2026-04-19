import './browsing-rhythm-card.css'

import { startTransition, useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { IntelligenceSectionMeta } from './section-meta'
import * as api from '../../lib/core-intelligence/api'
import {
  dateRangeForCalendarYear,
  useAsyncData,
  type DateRange,
  type DiscoveryTrendPoint,
} from '../../lib/core-intelligence'

export type BrowsingRhythmTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

interface CalendarDayCell {
  date: Date
  dateKey: string
  inRange: boolean
  newDomainCount: number
  totalVisits: number
}

interface BrowsingRhythmCardProps {
  dateRange?: DateRange
  dayHref: (date: string) => string
  language: string
  mode: 'range' | 'year'
  profileId?: string | null
  scopeLabel?: string
  t: BrowsingRhythmTranslator
}

/**
 * Reusable calendar-based browsing rhythm card shared by Dashboard and
 * `/intelligence`.
 *
 * The accepted contract is a real-date calendar heatmap backed by
 * `getDiscoveryTrend(..., 'day')`. Day cells are navigation-first: they take
 * users into the dedicated day insights route instead of rendering inline
 * detail inside the overview card.
 */
export function BrowsingRhythmCard({
  dateRange,
  dayHref,
  language,
  mode,
  profileId,
  scopeLabel,
  t,
}: BrowsingRhythmCardProps) {
  const navigate = useNavigate()
  const [selectedYear, setSelectedYear] = useState(() =>
    new Date().getFullYear(),
  )
  const [manualYearSelection, setManualYearSelection] = useState(false)
  const effectiveDateRange = useMemo(() => {
    if (mode === 'year') {
      return dateRangeForCalendarYear(selectedYear)
    }
    return dateRange ?? dateRangeForCalendarYear(selectedYear)
  }, [dateRange, mode, selectedYear])
  const trendResult = useAsyncData(
    () => api.getDiscoveryTrend(effectiveDateRange, profileId, 'day'),
    [effectiveDateRange.start, effectiveDateRange.end, mode, profileId],
  )
  const points = useMemo(
    () => trendResult.data?.data.points ?? [],
    [trendResult.data],
  )
  const availableYears = useMemo(
    () =>
      [...(trendResult.data?.data.availableYears ?? [])].sort(
        (left, right) => right - left,
      ),
    [trendResult.data],
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
    availableYears.length > 0 &&
    !availableYears.includes(selectedYear)

  useEffect(() => {
    if (mode !== 'year' || availableYears.length === 0) {
      return
    }

    const nextYear =
      manualYearSelection && availableYears.includes(selectedYear)
        ? selectedYear
        : availableYears[0]

    if (nextYear !== selectedYear || !availableYears.includes(selectedYear)) {
      startTransition(() => {
        setSelectedYear(nextYear)
        if (!availableYears.includes(selectedYear)) {
          setManualYearSelection(false)
        }
      })
    }
  }, [availableYears, manualYearSelection, mode, selectedYear])

  return (
    <div className="browsing-rhythm-card">
      <div className="browsing-rhythm-card__controls">
        <span className="browsing-rhythm-card__legend">
          {t('rhythmLegend')}
        </span>
        {mode === 'year' && availableYears.length > 1 ? (
          <label className="browsing-rhythm-card__selector">
            <span>{t('rhythmYearLabel')}</span>
            <select
              aria-label={t('rhythmYearAria', {
                year: selectedYear,
              })}
              className="browsing-rhythm-card__select"
              data-testid="browsing-rhythm-year-select"
              value={selectedYear}
              onChange={(event) => {
                setManualYearSelection(true)
                setSelectedYear(Number(event.target.value))
              }}
            >
              {availableYears.map((year) => (
                <option key={year} value={year}>
                  {year}
                </option>
              ))}
            </select>
          </label>
        ) : null}
      </div>

      {scopeLabel && trendResult.data ? (
        <IntelligenceSectionMeta
          meta={trendResult.data.meta}
          scopeLabel={scopeLabel}
        />
      ) : null}

      {trendResult.loading || waitingForYearRealignment ? (
        <div className="browsing-rhythm-card__skeleton" />
      ) : trendResult.error ? (
        <div className="browsing-rhythm-card__empty">
          <p className="browsing-rhythm-card__empty-text">
            {trendResult.error}
          </p>
        </div>
      ) : calendarDays.length === 0 ? (
        <div className="browsing-rhythm-card__empty">
          <p className="browsing-rhythm-card__empty-text">
            {mode === 'year' ? t('rhythmYearEmpty') : t('rhythmEmpty')}
          </p>
        </div>
      ) : (
        <>
          <div className="rhythm-calendar-shell">
            <div className="rhythm-calendar__months">
              <span className="rhythm-calendar__months-spacer" />
              <div className="rhythm-calendar__months-track">
                {monthLabels.map((label, index) => (
                  <span
                    key={`${label}:${index}`}
                    className="rhythm-calendar__month"
                  >
                    {label}
                  </span>
                ))}
              </div>
            </div>
            <div
              className="rhythm-calendar"
              role="grid"
              aria-label={t('rhythmLabel')}
            >
              <div className="rhythm-calendar__weekday-rail" aria-hidden="true">
                {weekdayLabels.map((label) => (
                  <span key={label} className="rhythm-calendar__weekday">
                    {label}
                  </span>
                ))}
              </div>
              <div className="rhythm-calendar__weeks">
                {calendarWeeks.map((week, weekIndex) => (
                  <div key={weekIndex} className="rhythm-calendar__week">
                    {week.map((cell) => {
                      const level = heatLevel(cell.totalVisits, maxVisits)

                      return (
                        <button
                          key={cell.dateKey}
                          type="button"
                          disabled={!cell.inRange}
                          className={`rhythm-calendar__day${
                            cell.inRange ? '' : ' rhythm-calendar__day--outside'
                          }`}
                          data-level={level}
                          aria-label={t('rhythmDayTooltip', {
                            date: cell.dateKey,
                            count: cell.totalVisits,
                            newDomains: cell.newDomainCount,
                          })}
                          onClick={() => void navigate(dayHref(cell.dateKey))}
                        />
                      )
                    })}
                  </div>
                ))}
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  )
}

function buildCalendarWeeks(
  dateRange: DateRange,
  points: Map<string, DiscoveryTrendPoint>,
) {
  const start = parseDateKey(dateRange.start)
  const end = parseDateKey(dateRange.end)
  const calendarStart = startOfWeek(start)
  const calendarEnd = endOfWeek(end)
  const weeks: CalendarDayCell[][] = []
  let cursor = calendarStart

  while (cursor.getTime() <= calendarEnd.getTime()) {
    const week: CalendarDayCell[] = []
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

function buildMonthLabels(weeks: CalendarDayCell[][], language: string) {
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

function heatLevel(count: number, maxCount: number) {
  if (count <= 0 || maxCount <= 0) {
    return 0
  }

  const ratio = count / maxCount
  if (ratio >= 0.75) return 4
  if (ratio >= 0.5) return 3
  if (ratio >= 0.25) return 2
  return 1
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
