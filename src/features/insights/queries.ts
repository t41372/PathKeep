/** Data for Insights, one query per card, all keyed by the selected range. */
import { useQuery } from '@tanstack/react-query'
import {
  insightsClient,
  lastDays,
  pollWhileStale,
} from '@/lib/backend-client/insights'
import { queryKeys } from '@/lib/query'

export const rangeDays = { d7: 7, d30: 30, d90: 90, y1: 365 } as const
export type RangeId = keyof typeof rangeDays

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
