/**
 * Data for the Insights drill-ins (day, site, search, page). Every query is
 * one backend aggregate or one bounded detail read; nothing here walks visit
 * rows on the client.
 */
import { useQueries, useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'
import { insightsClient, pollWhileStale } from '@/lib/backend-client/insights'
import type { TrailSummary } from '@/lib/core-intelligence/types'
import { queryKeys } from '@/lib/query'
import { rangeDates, type RangeId } from './range'

const key = (...parts: unknown[]) => [
  ...queryKeys.archiveData,
  'insights',
  ...parts,
]

const keepPrevious = <T>(previous: T | undefined) => previous

export function useDay(date: string, enabled = true) {
  return useQuery({
    queryKey: key('day', date),
    queryFn: () => insightsClient.day(date),
    enabled,
    refetchInterval: pollWhileStale,
  })
}

const dayScope = (date: string) => ({ dateRange: { start: date, end: date } })

export function useDaySessions(date: string, enabled = true) {
  return useQuery({
    queryKey: key('day', date, 'sessions'),
    queryFn: () => insightsClient.sessions(dayScope(date)),
    enabled,
  })
}

export function useSessionDetail(sessionId: string, enabled: boolean) {
  return useQuery({
    queryKey: key('session', sessionId),
    queryFn: () => insightsClient.sessionDetail(sessionId),
    enabled,
  })
}

export function useDaySearches(date: string, enabled = true) {
  return useQuery({
    queryKey: key('day', date, 'searches'),
    queryFn: () => insightsClient.searchQueries(dayScope(date)),
    enabled,
    refetchInterval: pollWhileStale,
  })
}

export function useSite(domain: string, range: RangeId) {
  return useQuery({
    queryKey: key('site', domain, range),
    queryFn: () =>
      insightsClient.site(domain, { dateRange: rangeDates(range) }),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}

function sameQuery(a: string, b: string) {
  return a.trim().toLowerCase() === b.trim().toLowerCase()
}

/**
 * Families are kept per browser profile and search engine, so one search
 * typed in two browsers is two families. Find every family for the text
 * among the range's families (the same list the "Frequent searches" card
 * comes from), then load each one's detail.
 */
export function useSearch(query: string, range: RangeId) {
  const scope = { dateRange: rangeDates(range) }
  const families = useQuery({
    queryKey: key(range, 'families'),
    queryFn: () => insightsClient.queryFamilies(scope),
    refetchInterval: pollWhileStale,
  })
  const matches = useMemo(
    () =>
      (families.data?.data.families ?? []).filter(
        (family) =>
          sameQuery(family.anchorQuery, query) ||
          family.queries.some((member) => sameQuery(member, query)),
      ),
    [families.data, query],
  )
  const details = useQueries({
    queries: matches.map((family) => ({
      queryKey: key('family', family.familyId, range),
      queryFn: () => insightsClient.queryFamilyDetail(family.familyId, scope),
    })),
    combine: (results) => {
      const seen = new Map<string, TrailSummary>()
      for (const result of results) {
        for (const trail of result.data?.data.relatedTrails ?? []) {
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

export function useRefindPage(
  canonicalUrl: string,
  range: RangeId,
  profileId: string | null,
) {
  return useQuery({
    queryKey: key('page', canonicalUrl, range, profileId),
    queryFn: () =>
      insightsClient.refindPageDetail(canonicalUrl, {
        dateRange: rangeDates(range),
        profileId,
      }),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}
