/**
 * @file paper-day-insights-helpers.ts
 * @description Pure aggregators that turn a `PaperDay` into the numbers
 * the day-insights strip renders below each Browse separator.
 * @module components/explorer-paper
 *
 * ## Responsibilities
 * - Walk every visit on the day, fold into top-domain counts, activity
 *   tallies (pages / typed / links / searches), per-hour buckets, and
 *   distinct-domain count.
 * - Match Chrome's `transition_type` low byte against TYPED (1), LINK (0)
 *   and GENERATED / KEYWORD_GENERATED (5 / 10) so the activity numbers
 *   reflect the actual archive data, not the design tool's made-up shape.
 *
 * ## Not responsible for
 * - Rendering. The DOM lives in `paper-day-insights.tsx`.
 * - Fetching anything. Only `PaperDay` (already grouped) is needed.
 *
 * ## Why this helper exists
 * The design-tool source `pk-contactsheet.jsx` carries this aggregation
 * inline against synthetic visits with `type: 'typed' | 'link' | 'search'`
 * fields. Our real `HistoryEntry` has Chrome's bitmask `transition` field
 * instead, so we re-derive the same numbers but from the canonical
 * archive shape — that way the strip is honest about what's in the
 * archive instead of trying to invent classification.
 */

import type { HistoryEntry } from '@/lib/types/archive'
import type { PaperDay } from '@/pages/explorer/paper/group-entries'

/** Chrome `transition_type` low-byte constants used for activity classification. */
const TRANSITION_LINK = 0
const TRANSITION_TYPED = 1
const TRANSITION_GENERATED = 5
const TRANSITION_KEYWORD_GENERATED = 10

/**
 * Known search-engine hosts. Used as a fallback when the
 * `transition_type` bitmask isn't preserved by the source browser (some
 * Chromium forks drop it). Keeps the "searches" tally honest for users
 * whose default engine isn't Google.
 */
const SEARCH_ENGINE_HOSTS = new Set([
  'google.com',
  'www.google.com',
  'bing.com',
  'www.bing.com',
  'duckduckgo.com',
  'www.duckduckgo.com',
  'kagi.com',
  'www.kagi.com',
  'startpage.com',
  'www.startpage.com',
  'baidu.com',
  'www.baidu.com',
  'yandex.com',
  'www.yandex.com',
])

export interface DayInsightsTopDomain {
  domain: string
  visits: number
}

export interface DayInsightsTopUrl {
  url: string
  title: string | null
  visits: number
}

export interface DayInsightsSearchQuery {
  /** The cleaned, human-readable query string the user typed. */
  query: string
  /** How many times this query appeared on the day (case + whitespace folded). */
  count: number
}

export interface DayInsights {
  totalPages: number
  typedCount: number
  linkCount: number
  searchCount: number
  distinctDomains: number
  sessionCount: number
  topDomains: DayInsightsTopDomain[]
  /** Visits per local hour bucket, length 24. */
  hourBuckets: number[]
  /** Highest single-hour count; ≥ 1 so callers can divide safely. */
  hourPeak: number
  /** First visit time in ms (local), null when the day has no visits. */
  firstVisitMs: number | null
  /** Last visit time in ms (local), null when the day has no visits. */
  lastVisitMs: number | null
  /** Peak hour of the day (0..23) — local time bucket with the most visits. */
  peakHour: number | null
  /** Longest single session's duration in ms (start → end of that session). */
  longestSessionMs: number
  /** Three most-revisited individual URLs on the day. */
  topUrls: DayInsightsTopUrl[]
  /**
   * Top search queries the user typed today, deduplicated by lower-cased
   * trimmed form. Limited to 6 entries — beyond that the strip stops being
   * a memory jog and becomes a haystack. Extracted heuristically from
   * known search-engine hosts; sites we don't recognise contribute nothing.
   */
  topSearchQueries: DayInsightsSearchQuery[]
}

/**
 * Aggregates one day's visits into the shape the day-insights strip needs.
 *
 * Returns zeros / empty arrays for days with no visits; callers should
 * guard at the render boundary rather than expecting non-empty.
 */
export function aggregateDayInsights(day: PaperDay): DayInsights {
  const domainCounts = new Map<string, number>()
  const urlCounts = new Map<
    string,
    { url: string; title: string | null; visits: number }
  >()
  const searchQueryCounts = new Map<
    string,
    { query: string; count: number }
  >()
  const hourBuckets = new Array<number>(24).fill(0)
  let totalPages = 0
  let typedCount = 0
  let linkCount = 0
  let searchCount = 0
  let firstVisitMs: number | null = null
  let lastVisitMs: number | null = null
  let longestSessionMs = 0

  const trackEntry = (entry: HistoryEntry) => {
    const url = entry.url
    if (url) {
      const existing = urlCounts.get(url)
      if (existing) {
        existing.visits += 1
        // Keep the most informative title we've seen so far for this URL.
        if (!existing.title && entry.title) existing.title = entry.title
      } else {
        urlCounts.set(url, {
          url,
          title: entry.title ?? null,
          visits: 1,
        })
      }
      const query = extractSearchQuery(url)
      if (query) {
        const key = query.toLowerCase()
        const existingQuery = searchQueryCounts.get(key)
        if (existingQuery) {
          existingQuery.count += 1
        } else {
          searchQueryCounts.set(key, { query, count: 1 })
        }
      }
    }
    const ms = localMsOf(entry)
    if (ms !== null) {
      if (firstVisitMs === null || ms < firstVisitMs) firstVisitMs = ms
      if (lastVisitMs === null || ms > lastVisitMs) lastVisitMs = ms
    }
  }

  for (const session of day.sessions) {
    const sessionSpan = Math.max(0, session.endMs - session.startMs)
    if (sessionSpan > longestSessionMs) longestSessionMs = sessionSpan
    for (const block of session.blocks) {
      trackEntry(block.entry)
      accumulate(block.entry, domainCounts, hourBuckets, (deltas) => {
        totalPages += deltas.page
        typedCount += deltas.typed
        linkCount += deltas.link
        searchCount += deltas.search
      })
    }
  }

  const topDomains: DayInsightsTopDomain[] = [...domainCounts.entries()]
    .map(([domain, visits]) => ({ domain, visits }))
    .sort((a, b) => b.visits - a.visits)
    .slice(0, 4)

  const topUrls: DayInsightsTopUrl[] = [...urlCounts.values()]
    .sort((a, b) => b.visits - a.visits)
    .slice(0, 3)

  const topSearchQueries: DayInsightsSearchQuery[] = [
    ...searchQueryCounts.values(),
  ]
    .sort((a, b) => b.count - a.count || a.query.localeCompare(b.query))
    .slice(0, 6)

  let peakHour: number | null = null
  let peakCount = 0
  for (let hour = 0; hour < 24; hour += 1) {
    if (hourBuckets[hour] > peakCount) {
      peakCount = hourBuckets[hour]
      peakHour = hour
    }
  }

  return {
    totalPages,
    typedCount,
    linkCount,
    searchCount,
    distinctDomains: domainCounts.size,
    sessionCount: day.sessions.length,
    topDomains,
    hourBuckets,
    hourPeak: Math.max(peakCount, 1),
    firstVisitMs,
    lastVisitMs,
    peakHour,
    longestSessionMs,
    topUrls,
    topSearchQueries,
  }
}

/**
 * Best-effort extraction of the user's search query from a URL on a
 * recognised search engine. Returns the trimmed query string or null
 * when the URL is not a search-engine result page.
 *
 * Why we extract client-side instead of relying on the Rust intelligence
 * `get_query_families` route: that backend pipe is rich (engine bucketing,
 * dedup across families, normalization) but expensive and requires a
 * round-trip per day rendered. The Browse day-insights strip needs an
 * immediate "what did I search for today" jog at render time, so the
 * heuristic here trades exhaustiveness for zero-latency.
 */
function extractSearchQuery(rawUrl: string): string | null {
  let url: URL
  try {
    url = new URL(rawUrl)
  } catch {
    return null
  }
  const host = url.hostname.toLowerCase().replace(/^www\./, '')
  const param = SEARCH_QUERY_PARAMS_BY_HOST.get(host)
  if (!param) return null
  // Some engines (Baidu mainly) require the query path to be a search
  // page rather than a static asset; the param check + non-empty value
  // gate is sufficient for the common cases we care about.
  const value = url.searchParams.get(param)
  if (!value) return null
  const trimmed = value.trim()
  if (!trimmed) return null
  // Drop pathological lengths so a stray paste of an entire essay
  // doesn't bloat the panel.
  if (trimmed.length > 120) return null
  return trimmed
}

/**
 * Per-host search-engine query parameter map. Keys are normalised hosts
 * (lower-case, `www.` stripped); values are the URL search param that
 * carries the user's typed query.
 *
 * Kept inline here rather than imported from the visit-taxonomy crate
 * because (1) the frontend doesn't have access to that pack and (2) the
 * day-insights strip wants to render immediately at scroll time, not
 * after a round-trip. The list is intentionally short: only engines we
 * see regularly in real archives.
 */
const SEARCH_QUERY_PARAMS_BY_HOST: ReadonlyMap<string, string> = new Map([
  ['google.com', 'q'],
  ['bing.com', 'q'],
  ['duckduckgo.com', 'q'],
  ['kagi.com', 'q'],
  ['startpage.com', 'query'],
  ['ecosia.com', 'q'],
  ['brave.com', 'q'],
  ['search.brave.com', 'q'],
  ['baidu.com', 'wd'],
  ['yandex.com', 'text'],
  ['yandex.ru', 'text'],
  ['yahoo.com', 'p'],
  ['search.yahoo.com', 'p'],
  ['so.com', 'q'],
  ['sogou.com', 'query'],
])

function localMsOf(entry: HistoryEntry): number | null {
  if (Number.isFinite(entry.visitTime) && entry.visitTime > 0) {
    return entry.visitTime > 1e12 ? entry.visitTime : entry.visitTime * 1000
  }
  const parsed = Date.parse(entry.visitedAt)
  if (Number.isNaN(parsed)) return null
  return parsed
}

function accumulate(
  entry: HistoryEntry,
  domainCounts: Map<string, number>,
  hourBuckets: number[],
  emit: (deltas: {
    page: number
    typed: number
    link: number
    search: number
  }) => void,
) {
  const domain = entry.domain || extractFallbackDomain(entry.url)
  if (domain) {
    domainCounts.set(domain, (domainCounts.get(domain) ?? 0) + 1)
  }
  const hour = localHourOf(entry)
  if (Number.isFinite(hour) && hour >= 0 && hour <= 23) {
    hourBuckets[hour] += 1
  }
  const transitionLowByte = (entry.transition ?? -1) & 0xff
  const isTyped = transitionLowByte === TRANSITION_TYPED
  const isLink = transitionLowByte === TRANSITION_LINK
  const isSearch =
    transitionLowByte === TRANSITION_GENERATED ||
    transitionLowByte === TRANSITION_KEYWORD_GENERATED ||
    SEARCH_ENGINE_HOSTS.has(domain.toLowerCase())
  emit({
    page: 1,
    typed: isTyped ? 1 : 0,
    link: isLink ? 1 : 0,
    search: isSearch ? 1 : 0,
  })
}

function extractFallbackDomain(url: string | null | undefined): string {
  if (!url) return ''
  try {
    return new URL(url).hostname
  } catch {
    return ''
  }
}

function localHourOf(entry: HistoryEntry): number {
  // visitTime carries seconds (legacy Chrome) or ms (takeout / WebExtension);
  // delegate to localMsOf so the dual-format detection lives in one place.
  // Otherwise an ms value gets multiplied by 1,000, producing a year ~50,000
  // AD whose `getHours()` collapses to 0 and shows up as a phantom 12 AM peak.
  const ms = localMsOf(entry)
  return ms === null ? -1 : new Date(ms).getHours()
}

/**
 * Compact human duration; sub-hour shown as "Nm", multi-hour as "Hh Mm".
 * Uses `Intl.NumberFormat` with `style: 'unit'` for locale-correct unit
 * spelling, falling back to the bare token + `h`/`m` when a runtime
 * lacks unit-style support.
 */
export function formatDuration(ms: number, language: string): string {
  const minutes = Math.max(1, Math.round(ms / 60_000))
  if (minutes < 60) {
    return formatUnitWithLocale(minutes, 'minute', language)
  }
  const hours = Math.floor(minutes / 60)
  const remainder = minutes % 60
  const hourLabel = formatUnitWithLocale(hours, 'hour', language)
  if (remainder === 0) return hourLabel
  const minuteLabel = formatUnitWithLocale(remainder, 'minute', language)
  return `${hourLabel} ${minuteLabel}`
}

function formatUnitWithLocale(
  value: number,
  unit: 'minute' | 'hour',
  language: string,
): string {
  try {
    return new Intl.NumberFormat(language, {
      style: 'unit',
      unit,
      unitDisplay: 'narrow',
    }).format(value)
  } catch {
    return `${value}${unit === 'hour' ? 'h' : 'm'}`
  }
}
