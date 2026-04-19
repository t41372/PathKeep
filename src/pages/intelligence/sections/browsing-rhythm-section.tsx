/**
 * Calendar-style browsing rhythm section for `/intelligence`.
 *
 * Why this file exists:
 * - The main rhythm view now uses real calendar dates instead of a weekday ×
 *   hour surrogate, so the day-heatmap and same-day detail logic deserve a
 *   focused module.
 * - Keeping the date-grid view-model here makes it easier to reason about
 *   truthfulness across week / month / quarter / year windows.
 *
 * Main declarations:
 * - `BrowsingRhythmSection`
 */

import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { IntelligenceSectionMeta } from '../../../components/intelligence/section-meta'
import {
  useAsyncData,
  type DateRange,
  type DigestSummary,
  type DiscoveryTrendPoint,
  type RhythmHeatmap,
  type TopSite,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import type { ResolvedLanguage } from '../../../lib/i18n'
import { IntelligenceSectionBody } from './section-body'
import { formatHourRange, formatNumber, singleDayRange, type T } from './shared'

interface CalendarDayCell {
  date: Date
  dateKey: string
  inRange: boolean
  newDomainCount: number
  totalVisits: number
}

export function BrowsingRhythmSection({
  dateRange,
  domainHref,
  language,
  profileId,
  scopeLabel,
  t,
}: {
  dateRange: DateRange
  domainHref: (domain: string) => string
  language: ResolvedLanguage
  profileId: string | null
  scopeLabel: string
  t: T
}) {
  const selectionScopeKey = `${dateRange.start}:${dateRange.end}:${
    profileId ?? 'archive-wide'
  }`
  const [selectedDateState, setSelectedDateState] = useState<{
    dateKey: string
    scopeKey: string
  } | null>(null)
  const trendResult = useAsyncData(
    () => api.getDiscoveryTrend(dateRange, profileId, 'day'),
    [dateRange, profileId],
  )
  const points = useMemo(
    () => trendResult.data?.data.points ?? [],
    [trendResult.data],
  )
  const pointByDate = useMemo(
    () => new Map(points.map((point) => [point.dateKey, point])),
    [points],
  )
  const calendarWeeks = useMemo(
    () => buildCalendarWeeks(dateRange, pointByDate),
    [dateRange, pointByDate],
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
  const selectedDateRange = selectedDay
    ? singleDayRange(selectedDay.dateKey)
    : null
  const selectedDigest = useAsyncData(
    () =>
      selectedDateRange
        ? api.getDigestSummary(selectedDateRange, profileId)
        : Promise.resolve(null),
    [profileId, selectedDay?.dateKey],
  )
  const selectedTopSites = useAsyncData(
    () =>
      selectedDateRange
        ? api.getTopSites(selectedDateRange, profileId, 'visit_count', 5)
        : Promise.resolve(null),
    [profileId, selectedDay?.dateKey],
  )
  const selectedRhythm = useAsyncData(
    () =>
      selectedDateRange
        ? api.getBrowsingRhythm(selectedDateRange, profileId)
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

  return (
    <section className="intelligence-section rhythm-section">
      <div className="intelligence-section__title-row">
        <h2 className="intelligence-section__title">{t('rhythmTitle')}</h2>
        <span className="rhythm-calendar__legend">{t('rhythmLegend')}</span>
      </div>
      {trendResult.data ? (
        <IntelligenceSectionMeta
          meta={trendResult.data.meta}
          scopeLabel={scopeLabel}
        />
      ) : null}
      <p className="intelligence-section__help">{t('rhythmHelp')}</p>
      {trendResult.loading ? (
        <div className="intelligence-skeleton intelligence-skeleton--heatmap" />
      ) : calendarDays.length === 0 ? (
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">{t('rhythmEmpty')}</p>
        </div>
      ) : (
        <IntelligenceSectionBody className="rhythm-panel" variant="workbench">
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
                          onClick={() =>
                            setSelectedDateState({
                              dateKey: cell.dateKey,
                              scopeKey: selectionScopeKey,
                            })
                          }
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
              digest={selectedDigest.data?.data ?? null}
              digestLoading={selectedDigest.loading}
              domainHref={domainHref}
              point={pointByDate.get(selectedDay.dateKey) ?? null}
              rhythm={selectedRhythm.data?.data ?? null}
              rhythmLoading={selectedRhythm.loading}
              topSites={selectedTopSites.data?.data ?? []}
              topSitesLoading={selectedTopSites.loading}
              t={t}
            />
          ) : calendarDays.some((cell) => cell.totalVisits > 0) ? (
            <div className="intelligence-empty">
              <p className="intelligence-empty__text">
                {t('rhythmSelectDayPrompt')}
              </p>
            </div>
          ) : null}
        </IntelligenceSectionBody>
      )}
    </section>
  )
}

function BrowsingRhythmDayDetail({
  dateKey,
  digest,
  digestLoading,
  domainHref,
  point,
  rhythm,
  rhythmLoading,
  topSites,
  topSitesLoading,
  t,
}: {
  dateKey: string
  digest: DigestSummary | null
  digestLoading: boolean
  domainHref: (domain: string) => string
  point: DiscoveryTrendPoint | null
  rhythm: RhythmHeatmap | null
  rhythmLoading: boolean
  topSites: TopSite[]
  topSitesLoading: boolean
  t: T
}) {
  const hourCells = useMemo(() => buildHourCells(rhythm), [rhythm])
  const maxHourlyCount = Math.max(
    ...hourCells.map((cell) => cell.visitCount),
    1,
  )

  return (
    <div className="rhythm-day-detail">
      <div className="rhythm-day-detail__header">
        <div>
          <h3 className="rhythm-day-detail__title">
            {t('rhythmDaySummaryTitle', { date: dateKey })}
          </h3>
          <p className="rhythm-day-detail__subtitle">
            {t('rhythmDayVisits', { count: point?.totalVisits ?? 0 })}
          </p>
        </div>
        <span className="rhythm-day-detail__badge">
          {t('rhythmDayNewSites', { count: point?.newDomainCount ?? 0 })}
        </span>
      </div>

      <div className="rhythm-day-detail__hours">
        <span className="rhythm-day-detail__stat-label">
          {t('rhythmDayHoursTitle')}
        </span>
        {rhythmLoading ? (
          <div className="intelligence-skeleton intelligence-skeleton--bar" />
        ) : rhythm && hourCells.some((cell) => cell.visitCount > 0) ? (
          <div
            className="rhythm-hour-strip"
            role="img"
            aria-label={t('rhythmHourStripLabel', { date: dateKey })}
          >
            <div className="rhythm-hour-strip__grid">
              {hourCells.map((cell) => (
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

      {digestLoading ? (
        <div className="intelligence-skeleton intelligence-skeleton--card" />
      ) : digest ? (
        <div className="rhythm-day-detail__stats">
          <div className="rhythm-day-detail__stat">
            <span className="rhythm-day-detail__stat-label">
              {t('digestVisits')}
            </span>
            <strong>{formatNumber(digest.totalVisits.value)}</strong>
          </div>
          <div className="rhythm-day-detail__stat">
            <span className="rhythm-day-detail__stat-label">
              {t('digestSearches')}
            </span>
            <strong>{formatNumber(digest.totalSearches.value)}</strong>
          </div>
          <div className="rhythm-day-detail__stat">
            <span className="rhythm-day-detail__stat-label">
              {t('digestDeepRead')}
            </span>
            <strong>{formatNumber(digest.deepReadPages.value)}</strong>
          </div>
          <div className="rhythm-day-detail__stat">
            <span className="rhythm-day-detail__stat-label">
              {t('digestNewSites')}
            </span>
            <strong>{formatNumber(digest.newDomains.value)}</strong>
          </div>
        </div>
      ) : null}

      <div className="rhythm-day-detail__sites">
        <span className="rhythm-day-detail__stat-label">
          {t('topSitesTitle')}
        </span>
        {topSitesLoading ? (
          <div className="intelligence-skeleton intelligence-skeleton--card" />
        ) : topSites.length > 0 ? (
          <div className="rhythm-day-detail__site-list">
            {topSites.map((site) => (
              <Link
                key={site.registrableDomain}
                className="rhythm-day-detail__site-chip"
                to={domainHref(site.registrableDomain)}
              >
                {site.displayName ?? site.registrableDomain}
              </Link>
            ))}
          </div>
        ) : (
          <p className="rhythm-day-detail__empty">{t('rhythmDayNoSites')}</p>
        )}
      </div>
    </div>
  )
}

function buildHourCells(rhythm: RhythmHeatmap | null) {
  const byHour = new Map<number, number>()
  for (const cell of rhythm?.cells ?? []) {
    byHour.set(cell.hour, cell.visitCount)
  }

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

function buildMonthLabels(
  weeks: CalendarDayCell[][],
  language: ResolvedLanguage,
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

function localeFromLanguage(language: ResolvedLanguage) {
  if (language === 'zh-CN') return 'zh-CN'
  if (language === 'zh-TW') return 'zh-TW'
  return 'en-US'
}
