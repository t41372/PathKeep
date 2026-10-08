/**
 * Where Insights links go. One place builds every drill-in and History URL so
 * the routes in `src/app/router.tsx`, the History URL contract and these
 * links cannot drift apart.
 */
import { scopeParams, type Scope } from './range'

/** A path that keeps the scope the user was looking at. */
function withScope(path: string, scope?: Scope, extra?: URLSearchParams) {
  const params = scope
    ? scopeParams(scope, new URLSearchParams(extra))
    : new URLSearchParams(extra)
  const query = params.toString()
  return query ? `${path}?${query}` : path
}

export const insightsPath = (scope?: Scope) => withScope('/insights', scope)

/** A local calendar day, `YYYY-MM-DD`. */
export const dayPath = (date: string) => `/insights/day/${date}`

export const sitePath = (domain: string, scope?: Scope) =>
  withScope(`/insights/site/${encodeURIComponent(domain)}`, scope)

/** A search, by the text the user typed. */
export const searchPath = (query: string, scope?: Scope) =>
  withScope(`/insights/search/${encodeURIComponent(query)}`, scope)

/**
 * A page the user keeps reopening, by canonical URL. Re-find scores are per
 * browser profile; `scores` picks the profile of the row the link came from.
 */
export const pagePath = (
  canonicalUrl: string,
  scope?: Scope,
  scores?: string | null,
) =>
  withScope(
    `/insights/page/${encodeURIComponent(canonicalUrl)}`,
    scope,
    scores ? new URLSearchParams({ scores }) : undefined,
  )

/** History's date filter for a scope: `start..end`, none for all time. */
export function historyDateParam(scope: Scope) {
  return scope.range === 'all'
    ? undefined
    : `${scope.dates.start}..${scope.dates.end}`
}

/** A History URL with the given filters (see the History URL contract). */
export function historyPath(filters: {
  q?: string
  date?: string
  domain?: string
  visit?: number
}) {
  const params = new URLSearchParams()
  if (filters.q) params.set('q', filters.q)
  if (filters.date) params.set('date', filters.date)
  if (filters.domain) params.set('domain', filters.domain)
  if (filters.visit) params.set('visit', String(filters.visit))
  const query = params.toString()
  return query ? `/history?${query}` : '/history'
}
