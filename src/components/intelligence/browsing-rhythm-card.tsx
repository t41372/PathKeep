import './browsing-rhythm-card.css'

import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import * as api from '../../lib/core-intelligence/api'
import {
  dateRangeForCalendarYear,
  useAsyncData,
  type DateRange,
  type DayInsights,
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
  dayDomainHref?: (domain: string, date: string) => string
  dayHref: (date: string) => string
  language: string
  mode: 'range' | 'year'
  profileId?: string | null
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
  t,
  yearNavigation = 'select',
}: BrowsingRhythmCardProps) {
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
    availableYears.length > 0 &&
    !availableYears.includes(selectedYear)
  const selectedYearIndex = availableYears.indexOf(selectedYear)
  const newerYear =
    selectedYearIndex > 0 ? availableYears[selectedYearIndex - 1] : null
  const olderYear =
    selectedYearIndex >= 0 && selectedYearIndex < availableYears.length - 1
      ? availableYears[selectedYearIndex + 1]
      : null

  useEffect(() => {
    if (mode !== 'year' || availableYears.length === 0) {
      return
    }

    const hasSelectedYear = availableYears.includes(selectedYear)
    const nextYear =
      manualYearSelection && hasSelectedYear ? selectedYear : availableYears[0]

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
  }, [availableYears, manualYearSelection, mode, selectedYear])

  return (
    <div className="browsing-rhythm-card">
      <div className="browsing-rhythm-card__controls">
        <span className="browsing-rhythm-card__legend">
          {t('rhythmLegend')}
        </span>
        {mode === 'year' && availableYears.length > 1 ? (
          yearNavigation === 'pager' ? (
            <div
              className="browsing-rhythm-card__year-pager"
              data-testid="browsing-rhythm-year-pager"
            >
              <button
                aria-label={t('rhythmPreviousYearAria', {
                  year: newerYear ?? selectedYear,
                })}
                className="browsing-rhythm-card__year-button"
                data-testid="browsing-rhythm-year-previous"
                disabled={newerYear === null}
                type="button"
                onClick={() => {
                  if (newerYear !== null) {
                    setManualYearSelection(true)
                    setSelectedYear(newerYear)
                  }
                }}
              >
                {'<'}
              </button>
              <div
                aria-live="polite"
                className="browsing-rhythm-card__year-current"
                data-testid="browsing-rhythm-year-label"
              >
                <span className="browsing-rhythm-card__year-caption">
                  {t('rhythmYearLabel')}
                </span>
                <strong className="browsing-rhythm-card__year-value">
                  {selectedYear}
                </strong>
              </div>
              <button
                aria-label={t('rhythmNextYearAria', {
                  year: olderYear ?? selectedYear,
                })}
                className="browsing-rhythm-card__year-button"
                data-testid="browsing-rhythm-year-next"
                disabled={olderYear === null}
                type="button"
                onClick={() => {
                  if (olderYear !== null) {
                    setManualYearSelection(true)
                    setSelectedYear(olderYear)
                  }
                }}
              >
                {'>'}
              </button>
            </div>
          ) : (
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
          )
        ) : null}
      </div>

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
                      const isSelected = selectedDay?.dateKey === cell.dateKey

                      return (
                        <button
                          key={cell.dateKey}
                          type="button"
                          disabled={!cell.inRange}
                          className={`rhythm-calendar__day${
                            cell.inRange ? '' : ' rhythm-calendar__day--outside'
                          }${
                            isSelected ? ' rhythm-calendar__day--active' : ''
                          }`}
                          data-level={level}
                          aria-label={t('rhythmDayTooltip', {
                            date: cell.dateKey,
                            count: cell.totalVisits,
                            newDomains: cell.newDomainCount,
                          })}
                          onClick={() => {
                            setSelectedDateState({
                              dateKey: cell.dateKey,
                              scopeKey: selectionScopeKey,
                            })
                          }}
                        />
                      )
                    })}
                  </div>
                ))}
              </div>
            </div>
          </div>

          {selectedDay ? (
            <BrowsingRhythmDayDetail
              dateKey={selectedDay.dateKey}
              dayDomainHref={dayDomainHref}
              dayHref={dayHref}
              detail={selectedDayResult.data?.data ?? null}
              error={selectedDayResult.error}
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

function BrowsingRhythmDayDetail({
  dateKey,
  dayDomainHref,
  dayHref,
  detail,
  error,
  loading,
  t,
}: {
  dateKey: string
  dayDomainHref?: (domain: string, date: string) => string
  dayHref: (date: string) => string
  detail: DayInsights | null
  error: string | null
  loading: boolean
  t: BrowsingRhythmTranslator
}) {
  const hourlyCells = useMemo(
    () => buildHourCells(detail?.hourlyActivity ?? []),
    [detail?.hourlyActivity],
  )
  const maxHourlyCount = Math.max(
    ...hourlyCells.map((cell) => cell.visitCount),
    1,
  )
  const activityMix =
    detail?.activityMix.categories.slice(0, 4).map((category) => ({
      ...category,
      sharePercent: Math.round(category.share * 100),
    })) ?? []

  return (
    <div className="rhythm-day-detail" data-testid="browsing-rhythm-day-detail">
      <div className="rhythm-day-detail__header">
        <div>
          <h3 className="rhythm-day-detail__title">
            {t('rhythmDaySummaryTitle', { date: dateKey })}
          </h3>
          <p className="rhythm-day-detail__subtitle">
            {detail
              ? t('rhythmDayVisits', {
                  count: detail.digestSummary.totalVisits.value,
                })
              : t('rhythmDetailLoading')}
          </p>
        </div>
        <div className="rhythm-day-detail__actions">
          {detail ? (
            <span className="rhythm-day-detail__badge">
              {t('rhythmDayNewSites', {
                count: detail.digestSummary.newDomains.value,
              })}
            </span>
          ) : null}
          <Link className="rhythm-day-detail__link" to={dayHref(dateKey)}>
            {t('rhythmViewDetails')}
          </Link>
        </div>
      </div>

      {loading ? (
        <div className="rhythm-day-detail__grid">
          <div className="browsing-rhythm-card__mini-skeleton" />
          <div className="browsing-rhythm-card__mini-skeleton" />
          <div className="browsing-rhythm-card__mini-skeleton" />
        </div>
      ) : error ? (
        <p className="rhythm-day-detail__empty">{error}</p>
      ) : detail ? (
        <div className="rhythm-day-detail__grid">
          <div className="rhythm-day-detail__panel">
            <span className="rhythm-day-detail__stat-label">
              {t('rhythmDayHoursTitle')}
            </span>
            {hourlyCells.some((cell) => cell.visitCount > 0) ? (
              <div
                className="rhythm-hour-strip"
                role="img"
                aria-label={t('rhythmHourStripLabel', { date: dateKey })}
              >
                <div className="rhythm-hour-strip__grid">
                  {hourlyCells.map((cell) => (
                    <span
                      key={cell.hour}
                      className="rhythm-hour-strip__cell"
                      data-level={heatLevel(cell.visitCount, maxHourlyCount)}
                      title={t('rhythmHourTooltip', {
                        hour: formatHourRange(cell.hour),
                        count: cell.visitCount,
                      })}
                    />
                  ))}
                </div>
                <div className="rhythm-hour-strip__labels" aria-hidden="true">
                  {[0, 6, 12, 18, 23].map((hour) => (
                    <span key={hour}>{hour}</span>
                  ))}
                </div>
              </div>
            ) : (
              <p className="rhythm-day-detail__empty">
                {t('rhythmDayNoHourlyData')}
              </p>
            )}
          </div>

          <div className="rhythm-day-detail__panel">
            <span className="rhythm-day-detail__stat-label">
              {t('dayInsightsTopSitesTitle')}
            </span>
            {detail.topSites.length > 0 ? (
              <div className="rhythm-day-detail__site-list">
                {detail.topSites.slice(0, 6).map((site) => {
                  const label = site.displayName ?? site.registrableDomain

                  return dayDomainHref ? (
                    <Link
                      key={site.registrableDomain}
                      className="rhythm-day-detail__site-chip"
                      to={dayDomainHref(site.registrableDomain, dateKey)}
                    >
                      {label}
                    </Link>
                  ) : (
                    <span
                      key={site.registrableDomain}
                      className="rhythm-day-detail__site-chip"
                    >
                      {label}
                    </span>
                  )
                })}
              </div>
            ) : (
              <p className="rhythm-day-detail__empty">
                {t('rhythmDayNoSites')}
              </p>
            )}
          </div>

          <div className="rhythm-day-detail__panel">
            <span className="rhythm-day-detail__stat-label">
              {t('dayInsightsActivityMixTitle')}
            </span>
            {activityMix.length > 0 ? (
              <div className="rhythm-day-detail__mix-list">
                {activityMix.map((category) => (
                  <div
                    key={category.domainCategory}
                    className="rhythm-day-detail__mix-row"
                  >
                    <div className="rhythm-day-detail__mix-summary">
                      <span className="rhythm-day-detail__mix-label">
                        {t(`category_${category.domainCategory}`)}
                      </span>
                      <span className="rhythm-day-detail__mix-value">
                        {category.sharePercent}%
                      </span>
                    </div>
                    <span className="rhythm-day-detail__mix-bar">
                      <span
                        className="rhythm-day-detail__mix-fill"
                        data-category={category.domainCategory}
                        style={{ width: `${category.sharePercent}%` }}
                      />
                    </span>
                    <span className="rhythm-day-detail__mix-count">
                      {formatNumber(category.visitCount)} {t('visits')}
                    </span>
                  </div>
                ))}
              </div>
            ) : (
              <p className="rhythm-day-detail__empty">
                {t('activityMixEmpty')}
              </p>
            )}
          </div>
        </div>
      ) : null}
    </div>
  )
}

function buildHourCells(
  buckets: DayInsights['hourlyActivity'],
): Array<{ hour: number; visitCount: number }> {
  const byHour = new Map(
    buckets.map((bucket) => [bucket.hour, bucket.visitCount]),
  )

  return Array.from({ length: 24 }, (_, hour) => ({
    hour,
    visitCount: byHour.get(hour) ?? 0,
  }))
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

function formatNumber(value: number) {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}

function formatHourRange(hour: number) {
  const start = String(hour).padStart(2, '0')
  const end = String((hour + 1) % 24).padStart(2, '0')
  return `${start}:00-${end}:00`
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
