/** Data for Insights, one query per card, all keyed by the selected range. */
import { useQuery } from '@tanstack/react-query'
import { insightsClient, pollWhileStale } from '@/lib/backend-client/insights'
import type { ReopenedInvestigation } from '@/lib/core-intelligence/types'
import { queryKeys } from '@/lib/query'
import { chartDates, rangeBars } from './buckets'
import { scopeKey, type Scope } from './range'

const key = (scope: Scope, ...parts: unknown[]) => [
  ...queryKeys.archiveData,
  'insights',
  ...scopeKey(scope),
  ...parts,
]

/** The request fields every range read takes. */
export function request(scope: Scope) {
  return { dateRange: scope.dates, profileId: scope.profileId }
}

const keepPrevious = <T>(previous: T | undefined) => previous

export function useDigest(scope: Scope) {
  return useQuery({
    queryKey: key(scope, 'digest'),
    queryFn: () => insightsClient.digest(request(scope)),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

/** Pages and searches per day, week or month (see `buckets.ts`). */
export function useDailyActivity(scope: Scope) {
  return useQuery({
    queryKey: key(scope, 'daily'),
    queryFn: async () => {
      const [trend, mix] = await Promise.all([
        insightsClient.discoveryTrend(request(scope)),
        insightsClient.activityMixTrend(request(scope)),
      ])
      const byDay = new Map(
        trend.data.points.map((point) => [
          point.dateKey,
          { pages: point.totalVisits, searches: 0 },
        ]),
      )
      for (const point of mix.points) {
        const day = byDay.get(point.dateKey)
        if (day)
          day.searches =
            point.categories.find((entry) => entry.domainCategory === 'search')
              ?.visitCount ?? 0
      }
      return {
        meta: trend.meta,
        ...rangeBars(chartDates(scope, byDay.keys()), byDay, {
          pages: 0,
          searches: 0,
        }),
      }
    },
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

export function useTopSites(scope: Scope) {
  return useQuery({
    queryKey: key(scope, 'top-sites'),
    queryFn: () => insightsClient.topSites(request(scope), 6),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

export function useRhythm(scope: Scope) {
  return useQuery({
    queryKey: key(scope, 'rhythm'),
    queryFn: () => insightsClient.rhythm(request(scope)),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

/**
 * Most frequent searches in the range, shown as the user last typed them.
 * The counts cover the selected range only, so they never exceed the
 * "Searches" figure above them.
 */
export function useFrequentSearches(scope: Scope, limit = 10) {
  return useQuery({
    queryKey: key(scope, 'searches', limit),
    queryFn: async () => {
      const result = await insightsClient.frequentSearches(
        request(scope),
        limit,
      )
      return {
        meta: result.meta,
        data: result.data.items.map((entry) => ({
          query: entry.query,
          count: entry.searchCount,
        })),
      }
    },
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

export function useRefindPages(scope: Scope) {
  return useQuery({
    queryKey: key(scope, 'refind'),
    queryFn: () => insightsClient.refindPages(request(scope), 5),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

/**
 * Pages and searches the user came back to on several days. Rows are per
 * browser profile; keep one per anchor (the one seen most).
 */
export function useThreads(scope: Scope, limit = 6) {
  return useQuery({
    queryKey: key(scope, 'threads'),
    queryFn: async () => {
      const result = await insightsClient.reopenedInvestigations(request(scope))
      const byAnchor = new Map<string, ReopenedInvestigation>()
      for (const item of result.data.items) {
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

export function useBreadth(scope: Scope) {
  return useQuery({
    queryKey: key(scope, 'breadth'),
    queryFn: () => insightsClient.breadth(request(scope)),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

/** Sites visited on a regular rhythm; stopped ones first. */
export function useHabits(scope: Scope, limit = 6) {
  return useQuery({
    queryKey: key(scope, 'habits'),
    queryFn: async () => {
      const result = await insightsClient.habits(request(scope))
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

export function useBrowsers(scope: Scope) {
  return useQuery({
    queryKey: key(scope, 'browsers'),
    queryFn: () => insightsClient.browsers(request(scope)),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}
