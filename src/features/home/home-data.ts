/** Derived data shared by Home's cards. */
import { useMemo } from 'react'
import { isStale, type Section } from '@/lib/backend-client/insights'
import type { DiscoveryTrendPoint } from '@/lib/core-intelligence/types'
import { addDays } from '@/lib/time-zone'
import { useSnapshot } from '@/lib/queries/app'
import { useSourceStats } from './queries'

/**
 * The `days` days ending `today` (Insights' today), filling days the backend
 * omitted (no visits) so the series is continuous.
 */
export function dailySeries(
  points: DiscoveryTrendPoint[],
  days: number,
  today: string,
) {
  const byDay = new Map(
    points.map((point) => [point.dateKey, point.totalVisits]),
  )
  return Array.from({ length: days }, (_, index) => {
    const dateKey = addDays(today, index - (days - 1))
    return { dateKey, visits: byDay.get(dateKey) ?? 0 }
  })
}

export interface BrowserSummary {
  browserName: string
  visits: number
  selected: boolean
}

/** Folds profiles into one row per browser: total visits, and whether any profile is backed up. */
export function useBrowserSummaries() {
  const snapshot = useSnapshot()
  const stats = useSourceStats()
  return useMemo(() => {
    const selected = new Set(snapshot.config.selectedProfileIds)
    const byBrowser = new Map<string, BrowserSummary>()
    for (const profile of snapshot.browserProfiles) {
      if (!profile.historyExists) continue
      const entry = byBrowser.get(profile.browserName) ?? {
        browserName: profile.browserName,
        visits: 0,
        selected: false,
      }
      entry.selected ||= selected.has(profile.profileId)
      byBrowser.set(profile.browserName, entry)
    }
    for (const stat of stats.data ?? []) {
      const entry = byBrowser.get(stat.browserName) ?? {
        browserName: stat.browserName,
        visits: 0,
        selected: selected.has(stat.profileId),
      }
      entry.visits += stat.visitCount
      byBrowser.set(stat.browserName, entry)
    }
    const list = [...byBrowser.values()].sort(
      (a, b) => Number(b.selected) - Number(a.selected) || b.visits - a.visits,
    )
    return { list, loading: stats.isPending, failed: stats.isError }
  }, [snapshot, stats.data, stats.isPending, stats.isError])
}

/** A day-by-day trend with no visit on any day: while its section is stale, not built yet. */
export function noVisits(trend: { points: { totalVisits: number }[] }) {
  return trend.points.every((point) => point.totalVisits === 0)
}

/**
 * Whether `result` is still being built: its section is stale and holds nothing by the card's
 * own measure (see `building.tsx`).
 */
export function stillBuilding<T>(
  result: Section<T> | undefined,
  empty: (data: T) => boolean,
) {
  return result !== undefined && isStale(result) && empty(result.data)
}
