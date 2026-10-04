/** Data for Insights, one query per card, all keyed by the selected range. */
import { useQuery } from '@tanstack/react-query'
import {
  insightsClient,
  lastDays,
  pollWhileStale,
} from '@/lib/backend-client/insights'
import type { ReopenedInvestigation } from '@/lib/core-intelligence/types'
import { queryKeys } from '@/lib/query'
import { rangeDays, type RangeId } from './range'

export { rangeDays, type RangeId }

const key = (range: RangeId, ...parts: unknown[]) => [
  ...queryKeys.archiveData,
  'insights',
  range,
  ...parts,
]

function scope(range: RangeId) {
  return { dateRange: lastDays(rangeDays[range]) }
}

const keepPrevious = <T>(previous: T | undefined) => previous

export function useDigest(range: RangeId) {
  return useQuery({
    queryKey: key(range, 'digest'),
    queryFn: () => insightsClient.digest(scope(range)),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

/** Pages and searches per day (per week for the one-year range). */
export function useDailyActivity(range: RangeId) {
  const granularity = range === 'y1' ? 'week' : 'day'
  return useQuery({
    queryKey: key(range, 'daily'),
    queryFn: async () => {
      const [trend, mix] = await Promise.all([
        insightsClient.discoveryTrend(scope(range), granularity),
        insightsClient.activityMixTrend(scope(range), granularity),
      ])
      const searches = new Map(
        mix.points.map((point) => [
          point.dateKey,
          point.categories.find((entry) => entry.domainCategory === 'search')
            ?.visitCount ?? 0,
        ]),
      )
      return {
        meta: trend.meta,
        points: trend.data.points.map((point) => ({
          dateKey: point.dateKey,
          pages: point.totalVisits,
          searches: searches.get(point.dateKey) ?? 0,
        })),
      }
    },
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

export function useTopSites(range: RangeId) {
  return useQuery({
    queryKey: key(range, 'top-sites'),
    queryFn: () => insightsClient.topSites(scope(range), 6),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

export function useRhythm(range: RangeId) {
  return useQuery({
    queryKey: key(range, 'rhythm'),
    queryFn: () => insightsClient.rhythm(scope(range)),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

/**
 * Most frequent searches in the range, shown as the user last typed them.
 * The counts cover the selected range only, so they never exceed the
 * "Searches" figure above them.
 */
export function useFrequentSearches(range: RangeId, limit = 10) {
  return useQuery({
    queryKey: key(range, 'searches', limit),
    queryFn: async () => {
      const result = await insightsClient.frequentSearches(scope(range), limit)
      return {
        meta: result.meta,
        data: result.data.map((entry) => ({
          query: entry.query,
          count: entry.searchCount,
        })),
      }
    },
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

export function useRefindPages(range: RangeId) {
  return useQuery({
    queryKey: key(range, 'refind'),
    queryFn: () => insightsClient.refindPages(scope(range), 5),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

/**
 * Pages and searches the user came back to on several days. Rows are per
 * browser profile; keep one per anchor (the one seen most).
 */
export function useThreads(range: RangeId, limit = 6) {
  return useQuery({
    queryKey: key(range, 'threads'),
    queryFn: async () => {
      const result = await insightsClient.reopenedInvestigations(scope(range))
      const byAnchor = new Map<string, ReopenedInvestigation>()
      for (const item of result.data) {
        const anchor = `${item.anchorType}:${item.anchorId}:${item.anchorLabel}`
        const kept = byAnchor.get(anchor)
        if (!kept || item.occurrenceCount > kept.occurrenceCount) {
          byAnchor.set(anchor, item)
        }
      }
      return {
        meta: result.meta,
        data: [...byAnchor.values()]
          .sort((a, b) => b.lastSeenAt.localeCompare(a.lastSeenAt))
          .slice(0, limit),
      }
    },
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

export function useBreadth(range: RangeId) {
  return useQuery({
    queryKey: key(range, 'breadth'),
    queryFn: () => insightsClient.breadth(scope(range)),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

/** Sites visited on a regular rhythm; stopped ones first. */
export function useHabits(range: RangeId, limit = 6) {
  return useQuery({
    queryKey: key(range, 'habits'),
    queryFn: async () => {
      const result = await insightsClient.habits(scope(range))
      return {
        meta: result.meta,
        total: result.data.length,
        data: [...result.data]
          .sort(
            (a, b) =>
              Number(b.isInterrupted) - Number(a.isInterrupted) ||
              b.visitCount - a.visitCount,
          )
          .slice(0, limit),
      }
    },
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

export function useBrowsers(range: RangeId) {
  return useQuery({
    queryKey: key(range, 'browsers'),
    queryFn: () => insightsClient.browsers(scope(range)),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}
