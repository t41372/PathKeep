import './browsing-rhythm-card.css'

import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  RhythmActivityProportionBar,
  RhythmHourStrip,
} from './browsing-rhythm-detail'
import * as api from '../../lib/core-intelligence/api'
import {
  dateRangeForCalendarYear,
  useAsyncData,
  type DateRange,
  type DayInsights,
  type DiscoveryTrendPoint,
  type TimeRangePreset,
} from '../../lib/core-intelligence'
import { intelligenceCategoryLabel } from '../../pages/intelligence/copy'

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
      <div className="browsing-rhythm-card__controls">
        <span className="browsing-rhythm-card__legend">
          {t('rhythmLegend')}
        </span>
        {mode === 'year' ? (
          yearNavigation === 'pager' ? (
            <div className="browsing-rhythm-card__year-actions">
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
              {canResetToCurrentYear ? (
                <button
                  className="browsing-rhythm-card__current-year-button"
                  data-testid="browsing-rhythm-current-year-shortcut"
                  type="button"
                  onClick={() => {
                    setManualYearSelection(true)
                    setSelectedYear(currentCalendarYear)
                  }}
                >
                  {t('rhythmCurrentYearAction')}
                </button>
              ) : null}
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
                {yearOptions.map((year) => (
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
                          title={t('rhythmDayTooltip', {
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

function BrowsingRhythmDayDetail({
  dateKey,
  dayDomainHref,
  dayHref,
  detail,
  error,
  loading,
  language,
  t,
}: {
  dateKey: string
  dayDomainHref?: (domain: string, date: string) => string
  dayHref: (date: string) => string
  detail: DayInsights | null
  error: string | null
  loading: boolean
  language: string
  t: BrowsingRhythmTranslator
}) {
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
        <div className="rhythm-day-detail__skeletons">
          <div className="browsing-rhythm-card__mini-skeleton" />
          <div className="browsing-rhythm-card__mini-skeleton" />
          <div className="browsing-rhythm-card__mini-skeleton" />
        </div>
      ) : error ? (
        <p className="rhythm-day-detail__empty">{error}</p>
      ) : detail ? (
        <div className="rhythm-day-detail__content">
          <div className="rhythm-day-detail__block">
            <span className="rhythm-day-detail__stat-label">
              {t('rhythmDayHoursTitle')}
            </span>
            {detail.hourlyActivity.some((bucket) => bucket.visitCount > 0) ? (
              <RhythmHourStrip
                date={dateKey}
                hourly={detail.hourlyActivity}
                t={t}
              />
            ) : (
              <p className="rhythm-day-detail__empty">
                {t('rhythmDayNoHourlyData')}
              </p>
            )}
          </div>

          <div className="rhythm-day-detail__block">
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

          <div className="rhythm-day-detail__block">
            <span className="rhythm-day-detail__stat-label">
              {t('dayInsightsActivityMixTitle')}
            </span>
            {detail.activityMix.categories.length > 0 ? (
              <RhythmActivityProportionBar
                categories={detail.activityMix.categories}
                categoryLabel={(domainCategory) =>
                  intelligenceCategoryLabel(
                    language as 'en' | 'zh-CN' | 'zh-TW',
                    t,
                    domainCategory,
                  )
                }
                language={language as 'en' | 'zh-CN' | 'zh-TW'}
                t={t}
              />
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

function buildVisitSummary({
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

function formatDisplayDate(dateKey: string, language: string) {
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
  if (boundary === 'end' && sameYear) {
    return `${month} ${target.getDate()}, ${target.getFullYear()}`
  }
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

function buildYearOptions(dataYears: number[], currentYear: number) {
  if (dataYears.length === 0) {
    return [currentYear]
  }

  const lowerBound = Math.min(...dataYears, currentYear)
  const upperBound = Math.max(...dataYears, currentYear)
  return Array.from(
    { length: upperBound - lowerBound + 1 },
    (_, index) => upperBound - index,
  )
}

function extractPointYear(dateKey: string) {
  const match = /^(\d{4})/.exec(dateKey)
  if (!match) {
    return null
  }
  const year = Number(match[1])
  return Number.isFinite(year) ? year : null
}
