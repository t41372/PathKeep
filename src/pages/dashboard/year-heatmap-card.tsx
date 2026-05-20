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
          setError(
            nextError instanceof Error
              ? nextError.message
              : t('dashboard.yearInPagesError'),
          )
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [activeProfileId, archiveReady, t])

  const startDate = useMemo(() => {
    const range = dashboardHeatmapRange(new Date())
    return new Date(range.start)
  }, [])
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
