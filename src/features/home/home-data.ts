/** Derived data shared by Home's cards. */
import { useMemo } from 'react'
import { localDateKey } from '@/lib/backend-client/insights'
import type { DiscoveryTrendPoint } from '@/lib/core-intelligence/types'
import { useSnapshot } from '@/lib/queries/app'
import { useSourceStats } from './queries'

/** Fills days the backend omitted (no visits) so the series is continuous. */
export function dailySeries(points: DiscoveryTrendPoint[], days: number) {
  const byDay = new Map(
    points.map((point) => [point.dateKey, point.totalVisits]),
  )
  const today = new Date()
  return Array.from({ length: days }, (_, index) => {
    const date = new Date(today)
    date.setDate(today.getDate() - (days - 1 - index))
    const dateKey = localDateKey(date)
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
