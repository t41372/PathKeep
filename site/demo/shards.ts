/**
 * Where in the recording a call lives. What the screens need loads with the
 * demo (`recording/index.json`); the pages a visitor opens one at a time
 * live in files of their own, fetched when one is opened: a month of days
 * per file, a site per file. Every day and every site is recorded, so any
 * one a visitor picks is there.
 *
 * `record.ts` files calls by this; `backend.ts` looks them up by it.
 */

/** What a day's page in Insights asks for. */
const dayCommands = new Set([
  'get_day_insights',
  'get_sessions',
  'get_search_queries',
  'get_compare_sets',
])

type Request = {
  date?: string
  dateRange?: { start?: string; end?: string }
  registrableDomain?: string
}

const isDay = (value: unknown): value is string =>
  typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value)

/** The file a call belongs in (`days/2026-04`, `site/strava.com`), or null for the main one. */
export function shardOf(command: string, args: unknown): string | null {
  const request = (args as { request?: Request } | null)?.request
  if (!request) return null
  if (dayCommands.has(command)) {
    const range = request.dateRange
    const day =
      request.date ?? (range?.start === range?.end ? range?.start : undefined)
    if (isDay(day)) return `days/${day.slice(0, 7)}`
  }
  if (command === 'get_domain_deep_dive' && request.registrableDomain)
    return `site/${encodeURIComponent(request.registrableDomain)}`
  return null
}
