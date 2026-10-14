/**
 * Data for Insights, one query per card, keyed by the scope. A card in one of
 * the two opening batches (`overview.ts`) waits for its batch, which fills its
 * cache entry; it reads by itself only when the batch left it empty, or after
 * the scope changes. So the cache holds each section exactly as its read
 * returns it, and cards shape it with `select`.
 */
import { useQuery } from '@tanstack/react-query'
import {
  insightsClient,
  pollWhileStale,
  type Section,
} from '@/lib/backend-client/insights'
import type {
  HabitPattern,
  ReopenedInvestigation,
} from '@/lib/core-intelligence/types'
import { queryKeys } from '@/lib/query'
import { useBandSettled, type Band } from './bands'
import { chartDates, rangeBars } from './buckets'
import { scopeKey, type Scope } from './range'

export const insightsKey = (scope: Scope, ...parts: unknown[]) => [
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

/**
 * One section read: waits for `band`, keeps the previous scope's result on
 * screen until the new one arrives, and polls while the section is updating.
 */
function useSection<T, R = Section<T>>(
  scope: Scope,
  name: string,
  band: Band | null,
  read: () => Promise<Section<T>>,
  select?: (section: Section<T>) => R,
) {
  return useQuery({
    queryKey: insightsKey(scope, name),
    queryFn: read,
    select,
    enabled: useBandSettled(band),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

export function useDigest(scope: Scope) {
  return useSection(scope, 'digest', 'primary', () =>
    insightsClient.digest(request(scope)),
  )
}

/** Pages and searches per day, week or month (see `buckets.ts`). */
export function useDailyActivity(scope: Scope) {
  return useQuery({
    queryKey: insightsKey(scope, 'daily'),
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

export type SiteOrder = 'visits' | 'unique_days' | 'average_daily_visits'

/**
 * The top 40 sites, by visits (the order the primary batch reads) or by days
 * visited or visits per day. Cards show as many as fit.
 */
export function useTopSites(scope: Scope, order: SiteOrder = 'visits') {
  return useSection(
    scope,
    order === 'visits' ? 'top-sites' : `top-sites-${order}`,
    order === 'visits' ? 'primary' : null,
    () => insightsClient.topSites(request(scope), 40, null, order),
  )
}

export function useRhythm(scope: Scope) {
  return useSection(scope, 'rhythm', null, () =>
    insightsClient.rhythm(request(scope)),
  )
}

/**
 * Most frequent searches in the range, shown as the user last typed them.
 * The counts cover the selected range only, so they never exceed the
 * "Searches" figure above them.
 */
export function useFrequentSearches(scope: Scope, limit = 10) {
  return useQuery({
    queryKey: insightsKey(scope, 'searches', limit),
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

/** As many as the overview batch reads (`intelligence_overview.rs`), so both fill one cache. */
const REFIND_LIST_LENGTH = 20

export function useRefindPages(scope: Scope) {
  return useSection(scope, 'refind', 'primary', () =>
    insightsClient.refindPages(request(scope), REFIND_LIST_LENGTH),
  )
}

export function useActivityMix(scope: Scope) {
  return useSection(scope, 'activity-mix', 'primary', () =>
    insightsClient.activityMix(request(scope)),
  )
}

export function useSearchEngines(scope: Scope) {
  return useSection(scope, 'search-engines', 'primary', () =>
    insightsClient.searchEngines(request(scope)),
  )
}

export function useSearchConcepts(scope: Scope) {
  return useSection(scope, 'search-concepts', 'primary', () =>
    insightsClient.searchConcepts(request(scope), 50),
  )
}

export function useQueryFamilies(scope: Scope) {
  return useSection(scope, 'query-families', 'primary', () =>
    insightsClient.queryFamilies(request(scope), 10),
  )
}

/** New sites per day; cards group the days like the activity chart. */
export function useNewSites(scope: Scope) {
  return useSection(scope, 'new-sites', 'primary', () =>
    insightsClient.discoveryTrend(request(scope)),
  )
}

/** Pages and searches the user came back to on several days. */
export function useThreads(scope: Scope, limit = 6) {
  return useSection(
    scope,
    'threads',
    'secondary',
    () => insightsClient.reopenedInvestigations(request(scope), 10),
    (result) => oneThreadPerAnchor(result, limit),
  )
}

/** Rows are per browser profile; keep one per anchor (the one seen most). */
function oneThreadPerAnchor(
  result: Section<{ items: ReopenedInvestigation[] }>,
  limit: number,
) {
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
}

export function useBreadth(scope: Scope) {
  return useSection(scope, 'breadth', 'secondary', () =>
    insightsClient.breadth(request(scope)),
  )
}

/** Sites visited on a regular rhythm; stopped ones first. */
export function useHabits(scope: Scope, limit = 6) {
  return useSection(
    scope,
    'habits',
    'primary',
    () => insightsClient.habits(request(scope)),
    (result) => stoppedFirst(result, limit),
  )
}

function stoppedFirst(result: Section<HabitPattern[]>, limit: number) {
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
}

export function useInterruptedHabits(scope: Scope) {
  return useSection(scope, 'interrupted-habits', 'primary', () =>
    insightsClient.interruptedHabits(scope.profileId),
  )
}

/** Three sites in a row, most frequent first (what the secondary batch reads). */
export function usePaths(scope: Scope) {
  return useSection(scope, 'paths', 'secondary', () =>
    insightsClient.paths(request(scope), 3, 15),
  )
}

/** Searches per engine, how often reworded, and the searches come back to most. */
export function useSearchEffectiveness(scope: Scope) {
  return useSection(scope, 'effectiveness', 'secondary', () =>
    insightsClient.searchEffectiveness(request(scope)),
  )
}

/** The ten sites searches ended on most (what the secondary batch reads). */
export function useSearchLandings(scope: Scope) {
  return useSection(scope, 'search-landings', 'secondary', () =>
    insightsClient.searchLandings(request(scope), 10),
  )
}

/** Searches that took several tries (what the secondary batch reads: 10). */
export function useFriction(scope: Scope) {
  return useSection(scope, 'friction', 'secondary', () =>
    insightsClient.friction(request(scope), 10),
  )
}

/** The six newest sets of pages compared within one search. */
export function useCompareSets(scope: Scope) {
  return useSection(scope, 'compare-sets', 'secondary', () =>
    insightsClient.compareSets(request(scope), 6),
  )
}

export function useBrowsers(scope: Scope) {
  return useSection(scope, 'browsers', 'secondary', () =>
    insightsClient.browsers(request(scope)),
  )
}
