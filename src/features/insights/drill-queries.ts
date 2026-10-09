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
import { request } from './queries'
import { scopeKey, type Scope } from './range'

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

export function useSite(domain: string, scope: Scope) {
  return useQuery({
    queryKey: key('site', domain, ...scopeKey(scope)),
    queryFn: () => insightsClient.site(domain, request(scope)),
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
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
    placeholderData: keepPrevious,
    refetchInterval: pollWhileStale,
  })
}
