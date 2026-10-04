/**
 * Where Insights links go. One place builds every drill-in and History URL so
 * the routes in `src/app/router.tsx`, the History URL contract and these
 * links cannot drift apart.
 */
import { rangeDates, type RangeId } from './range'

function withRange(path: string, range?: RangeId, extra?: URLSearchParams) {
  const params = new URLSearchParams(extra)
  if (range) params.set('range', range)
  const query = params.toString()
  return query ? `${path}?${query}` : path
}

export const insightsPath = (range?: RangeId) => withRange('/insights', range)

/** A local calendar day, `YYYY-MM-DD`. */
export const dayPath = (date: string) => `/insights/day/${date}`

export const sitePath = (domain: string, range?: RangeId) =>
  withRange(`/insights/site/${encodeURIComponent(domain)}`, range)

/** A search, by the text the user typed. */
export const searchPath = (query: string, range?: RangeId) =>
  withRange(`/insights/search/${encodeURIComponent(query)}`, range)

/**
 * A page the user keeps reopening, by canonical URL. Re-find scores are per
 * browser profile; `profile` picks the row the link came from.
 */
export const pagePath = (
  canonicalUrl: string,
  range?: RangeId,
  profile?: string | null,
) =>
  withRange(
    `/insights/page/${encodeURIComponent(canonicalUrl)}`,
    range,
    profile ? new URLSearchParams({ profile }) : undefined,
  )

/** History's date filter for a range: `start..end`. */
export function historyDateParam(range: RangeId) {
  const { start, end } = rangeDates(range)
  return `${start}..${end}`
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
