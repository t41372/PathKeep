/**
 * @file year-heatmap-card.tsx
 * @description Dashboard "A year in pages" card. Fetches daily discovery-trend points and renders them in the paper YearHeatmap.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Query `getDiscoveryTrend(range, profileId, 'day')` for the rolling
 *   365-day window when archiveReady, and map the response onto the
 *   YearHeatmap's `(dateKey, totalVisits)` shape.
 * - Render honest loading / empty / error states without inventing fake
 *   heatmap cells.
 * - Surface the longest recent streak in the card header so the user has a
 *   single-glance "I've been consistent for N days" signal.
 *
 * ## Not responsible for
 * - Cell-level bucketing — `buildYearHeatmapCells` owns that.
 * - Deep-link routing into the Browse contact-sheet view — the parent route
 *   passes `onSelectDate`.
 *
 * ## Performance notes
 * - 365 cells per render. The fetch is memoized by the `useEffect` deps so
 *   profile-scope changes are the only re-fetch trigger.
 */

import { useEffect, useMemo, useState } from 'react'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import * as coreIntelligenceApi from '@/lib/core-intelligence/api'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useProfileScope } from '@/lib/profile-scope-context'
import {
  YearHeatmap,
  type YearHeatmapCopy,
} from '@/components/heatmap/year-heatmap'
import {
  buildYearHeatmapCells,
  longestRecentStreak,
  type DailyVisitPoint,
} from '@/components/heatmap/year-heatmap-helpers'
import { dashboardHeatmapRange } from './dashboard-helpers'

export interface DashboardYearHeatmapCardProps {
  archiveReady: boolean
  onOpenInsights: () => void
  onSelectDate: (dateKey: string) => void
}

const HEATMAP_DAYS = 365

/**
 * Stable per-day token in the user's local timezone. Used as a dependency
 * key so memos / fetches re-run exactly when midnight crosses, regardless
 * of how long the dashboard has been mounted.
 */
function localDayKey(now: Date): string {
  const year = now.getFullYear()
  const month = `${now.getMonth() + 1}`.padStart(2, '0')
  const day = `${now.getDate()}`.padStart(2, '0')
  return `${year}-${month}-${day}`
}

export function DashboardYearHeatmapCard({
  archiveReady,
  onOpenInsights,
  onSelectDate,
}: DashboardYearHeatmapCardProps) {
  const { t, language } = useI18n()
  const { activeProfileId } = useProfileScope()

  const [points, setPoints] = useState<DailyVisitPoint[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // Local-day token that the fetch effect and `startDate` memo depend on
  // so the heatmap window rolls over to "today" the moment midnight
  // passes. Without this, a dashboard left mounted overnight kept showing
  // yesterday as the rightmost cell, and any visit recorded after midnight
  // would land off-grid until the user navigated away and back.
  const [todayKey, setTodayKey] = useState(() => localDayKey(new Date()))
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>
    function schedule() {
      const now = new Date()
      // +1s grace so the wake-up lands on the new day.
      const nextMidnight = new Date(
        now.getFullYear(),
        now.getMonth(),
        now.getDate() + 1,
        0,
        0,
        1,
      )
      timer = setTimeout(() => {
        setTodayKey(localDayKey(new Date()))
        schedule()
      }, nextMidnight.getTime() - now.getTime())
    }
    schedule()
    // Cleanup always has a timer to clear (the effect calls `schedule()`
    // synchronously, which assigns `timer`), so no `if (timer)` guard is
    // needed — a guard here would be defensive dead code.
    return () => clearTimeout(timer)
  }, [])

  useEffect(() => {
    if (!archiveReady) {
      setPoints([])
      setLoading(false)
      setError(null)
      return
    }
    let cancelled = false
    setLoading(true)
    void (async () => {
      try {
        const range = dashboardHeatmapRange(new Date())
        const result = await coreIntelligenceApi.getDiscoveryTrend(
          range,
          activeProfileId,
          'day',
        )
        if (!cancelled) {
          setPoints(
            (result.data?.points ?? []).map((point) => ({
              dateKey: point.dateKey,
              totalVisits: point.totalVisits,
            })),
          )
          setError(null)
        }
      } catch (nextError) {
        if (!cancelled) {
          setPoints([])
          setError(describeError(nextError, 'get_discovery_trend'))
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [activeProfileId, archiveReady, t, todayKey])

  const startDate = useMemo(() => {
    // `dashboardHeatmapRange` returns YYYY-MM-DD strings already in the
    // user's local time. `new Date('YYYY-MM-DD')` would parse them as UTC,
    // which in negative timezones (e.g. America/Phoenix UTC-7) becomes the
    // previous local day — shifting every heatmap cell and click target by
    // one. Parse the parts back as a local Date instead.
    const range = dashboardHeatmapRange(new Date())
    const [year, month, day] = range.start.split('-').map(Number)
    return new Date(year, month - 1, day)
    // `todayKey` ticks at local midnight so the window slides forward
    // without needing the user to reload the dashboard.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [todayKey])
  const cells = useMemo(
    () => buildYearHeatmapCells(points, startDate, HEATMAP_DAYS),
    [points, startDate],
  )
  const streak = useMemo(() => longestRecentStreak(cells), [cells])

  const copy = useMemo<YearHeatmapCopy>(
    () => buildHeatmapCopy(language, t),
    [language, t],
  )

  return (
    <PaperCard className="mb-4" testId="dashboard-year-heatmap">
      <PaperCardHeader
        title={t('dashboard.yearInPagesTitle')}
        compact
        right={
          <PaperCardBadge onClick={onOpenInsights}>
            {t('dashboard.allInsights')} →
          </PaperCardBadge>
        }
      />
      <PaperCardBody className="px-[18px] pb-[14px] pt-[10px]">
        {!archiveReady ? (
          <p
            className="m-0 font-serif text-[13.5px] italic leading-[1.55] text-ink-muted"
            data-testid="dashboard-year-empty"
          >
            {t('dashboard.yearInPagesEmpty')}
          </p>
        ) : loading ? (
          <div
            className="border-border-light h-[100px] animate-pulse rounded-paper border bg-hover"
            data-testid="dashboard-year-loading"
            aria-busy="true"
            aria-label={t('common.loading')}
          />
        ) : error ? (
          <p
            className="m-0 font-serif text-[13.5px] italic leading-[1.55] text-danger"
            data-testid="dashboard-year-error"
          >
            {error}
          </p>
        ) : (
          <div className="flex flex-col gap-3">
            <div
              className="flex items-baseline justify-between gap-3 text-ink-faint"
              data-testid="dashboard-year-streak"
            >
              <span className="font-mono text-[10px] uppercase tracking-[0.07em]">
                {t('dashboard.heatmapStreakLabel')}
              </span>
              <span className="font-serif text-[15px] text-ink">
                {streak > 0
                  ? t('dashboard.heatmapStreakDays', { count: streak })
                  : t('dashboard.heatmapStreakNone')}
              </span>
            </div>
            <YearHeatmap
              cells={cells}
              copy={copy}
              onSelectDate={(date) => onSelectDate(date)}
              testId="dashboard-year-heatmap-grid"
            />
          </div>
        )}
      </PaperCardBody>
    </PaperCard>
  )
}

function buildHeatmapCopy(
  language: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
): YearHeatmapCopy {
  const monthFormatter = new Intl.DateTimeFormat(
    language === 'en' ? 'en-US' : language,
    { month: 'short' },
  )
  const dayFormatter = new Intl.DateTimeFormat(
    language === 'en' ? 'en-US' : language,
    { weekday: 'short' },
  )
  // Reference monday is 2025-12-29; cycling outward gives us Sun..Sat labels
  // for the active locale without hard-coding English month/day strings.
  const monthLabels = Array.from({ length: 12 }, (_, index) =>
    monthFormatter.format(new Date(2025, index, 1)),
  ) as YearHeatmapCopy['monthLabels']
  const dayLabels = Array.from({ length: 7 }, (_, index) =>
    dayFormatter.format(new Date(2025, 11, 28 + index)),
  ) as YearHeatmapCopy['dayLabels']
  return {
    legendLess: t('dashboard.heatmapLegendLess'),
    legendMore: t('dashboard.heatmapLegendMore'),
    monthLabels,
    dayLabels,
    cellTooltip: (date, count) =>
      t('dashboard.heatmapCellTooltip', { date, count }),
  }
}
