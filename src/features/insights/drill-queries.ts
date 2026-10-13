/**
 * Data for the Insights drill-ins (day, site, search, page). Every query is
 * one backend aggregate or one bounded detail read; nothing here walks visit
 * rows on the client.
 */
import {
  useInfiniteQuery,
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query'
import { useMemo } from 'react'
import { insightsClient, pollWhileStale } from '@/lib/backend-client/insights'
import type { TrailSummary } from '@/lib/core-intelligence/types'
import { queryKeys } from '@/lib/query'
import { request } from './queries'
import { scopeKey, type Scope } from './range'
import { useInsightsZone } from './zone'

const key = (...parts: unknown[]) => [
  ...queryKeys.archiveData,
  'insights',
  ...parts,
]

/** Where a drill-in's key names its subject: `key(kind, subject, ...)`. */
const SUBJECT = key().length + 1

/**
 * Keeps showing what a drill-in showed while it reads another range or
 * profile of the same subject; another site or page starts blank, never with
 * the previous one's numbers under its name.
 */
const keepPreviousOf =
  (subject: string) =>
  <T>(previous: T | undefined, query?: { queryKey: readonly unknown[] }) =>
    query?.queryKey[SUBJECT] === subject ? previous : undefined

/**
 * A day's reads are keyed by the zone Insights are in, like a scope's
 * (`scopeKey`): after a rebuild in another zone the same date holds other
 * visits, and a day read before must not stand in for it.
 */
export function useDay(date: string, enabled = true) {
  const { shown } = useInsightsZone()
  return useQuery({
    queryKey: key('day', date, shown),
    queryFn: () => insightsClient.day(date),
    enabled,
    refetchInterval: pollWhileStale,
  })
}

const dayScope = (date: string) => ({ dateRange: { start: date, end: date } })

export function useDaySessions(date: string, enabled = true) {
  const { shown } = useInsightsZone()
  return useQuery({
    queryKey: key('day', date, shown, 'sessions'),
    queryFn: () => insightsClient.sessions(dayScope(date)),
    enabled,
  })
}

export function useSessionDetail(sessionId: string, enabled: boolean) {
  const { shown } = useInsightsZone()
  return useQuery({
    queryKey: key('session', sessionId, shown),
    queryFn: () => insightsClient.sessionDetail(sessionId),
    enabled,
  })
}

export function useDaySearches(date: string, enabled = true) {
  const { shown } = useInsightsZone()
  return useQuery({
    queryKey: key('day', date, shown, 'searches'),
    queryFn: () => insightsClient.searchQueries(dayScope(date)),
    enabled,
    refetchInterval: pollWhileStale,
  })
}

export function useSite(domain: string, scope: Scope) {
  return useQuery({
    queryKey: key('site', domain, ...scopeKey(scope)),
    queryFn: () => insightsClient.site(domain, request(scope)),
    placeholderData: keepPreviousOf(domain),
    refetchInterval: (query) =>
      // A category just set waits for Insights to be built with it.
      query.state.data?.data.categoryPending ? 2_000 : pollWhileStale(query),
  })
}

/**
 * Sets a site's category (`null`: back to PathKeep's lists), then reads
 * Insights again; it stays pending until they are read, so the site page
 * never shows the old choice as settled in between.
 */
export function useSetSiteCategory(domain: string) {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (category: string | null) =>
      insightsClient.setSiteCategory(domain, category),
    onSuccess: () =>
      client.invalidateQueries({ queryKey: [...queryKeys.archiveData] }),
  })
}

/** The sites in one category in the scope, most visits first, a page at a time. */
export function useSitesInCategory(
  category: string,
  scope: Scope,
  enabled: boolean,
) {
  return useInfiniteQuery({
    queryKey: key('sites-in-category', category, ...scopeKey(scope)),
    queryFn: ({ pageParam }) =>
      insightsClient.sitesInCategory(category, request(scope), 20, pageParam),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.data.nextCursor ?? undefined,
    enabled,
  })
}

/**
 * Families are kept per browser profile and search engine, so one search
 * typed in two browsers is two families. The backend finds every family
 * holding the text, across all time (the totals say "in total"); each one's
 * detail is then read for the chosen range.
 */
export function useSearch(query: string, scope: Scope) {
  const families = useQuery({
    queryKey: key('search-families', query, scope.profileId),
    queryFn: () => insightsClient.searchFamilies(query, scope.profileId),
    refetchInterval: pollWhileStale,
  })
  const matches = useMemo(
    () => families.data?.data.families ?? [],
    [families.data],
  )
  const details = useQueries({
    queries: matches.map((family) => ({
      queryKey: key('family', family.familyId, ...scopeKey(scope)),
      queryFn: () =>
        insightsClient.queryFamilyDetail(family.familyId, request(scope)),
    })),
    combine: (results) => {
      const seen = new Map<string, TrailSummary>()
      for (const result of results) {
        for (const trail of result.data?.data.relatedTrails.items ?? []) {
          seen.set(trail.trailId, trail)
        }
      }
      return {
        trails: [...seen.values()].sort(
          (a, b) => b.firstVisitMs - a.firstVisitMs,
        ),
        pending: results.some((result) => result.isPending),
        error: results.find((result) => result.isError)?.error ?? null,
        retry: () => results.forEach((result) => void result.refetch()),
      }
    },
  })
  return { families, matches, ...details }
}

export function useTrailDetail(trailId: string, enabled: boolean) {
  return useQuery({
    queryKey: key('trail', trailId),
    queryFn: () => insightsClient.trailDetail(trailId),
    enabled,
  })
}

/**
 * Re-find scores are per browser profile: `scores` picks whose, falling back
 * to the profile the screen is filtered to.
 */
export function useRefindPage(
  canonicalUrl: string,
  scope: Scope,
  scores: string | null,
) {
  const profileId = scores ?? scope.profileId
  return useQuery({
    queryKey: key(
      'page',
      canonicalUrl,
      scope.dates.start,
      scope.dates.end,
      profileId,
    ),
    queryFn: () =>
      insightsClient.refindPageDetail(canonicalUrl, {
        dateRange: scope.dates,
        profileId,
      }),
    placeholderData: keepPreviousOf(canonicalUrl),
    refetchInterval: pollWhileStale,
  })
}
