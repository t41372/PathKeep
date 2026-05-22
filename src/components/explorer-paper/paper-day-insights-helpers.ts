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
      if (block.type === 'single') {
        trackEntry(block.entry)
        accumulate(block.entry, domainCounts, hourBuckets, (deltas) => {
          totalPages += deltas.page
          typedCount += deltas.typed
          linkCount += deltas.link
          searchCount += deltas.search
        })
      } else {
        for (const entry of block.entries) {
          trackEntry(entry)
          accumulate(entry, domainCounts, hourBuckets, (deltas) => {
            totalPages += deltas.page
            typedCount += deltas.typed
            linkCount += deltas.link
            searchCount += deltas.search
          })
        }
      }
    }
  }

  const topDomains: DayInsightsTopDomain[] = [...domainCounts.entries()]
    .map(([domain, visits]) => ({ domain, visits }))
    .sort((a, b) => b.visits - a.visits)
    .slice(0, 4)

  const topUrls: DayInsightsTopUrl[] = [...urlCounts.values()]
    .sort((a, b) => b.visits - a.visits)
    .slice(0, 3)

  let peakHour: number | null = null
  let peakCount = 0
  for (let hour = 0; hour < 24; hour += 1) {
    if (hourBuckets[hour] > peakCount) {
      peakCount = hourBuckets[hour]
      peakHour = hour
    }
  }

  const hourPeak = hourBuckets.reduce(
    (peak, count) => (count > peak ? count : peak),
    0,
  )

  return {
    totalPages,
    typedCount,
    linkCount,
    searchCount,
    distinctDomains: domainCounts.size,
    sessionCount: day.sessions.length,
    topDomains,
    hourBuckets,
    hourPeak: Math.max(hourPeak, 1),
    firstVisitMs,
    lastVisitMs,
    peakHour,
    longestSessionMs,
    topUrls,
  }
}

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
  // visitTime is sometimes seconds since epoch (legacy Chrome-style) and
  // sometimes already ms (takeout / WebExtension API). Detect via the
  // 1e12 threshold the rest of the helper already uses for `localMsOf`;
  // otherwise we multiplied an ms value by 1,000 and got a fictional date
  // ~50,000 years from now, which `getHours()` collapsed to 0 → "12 AM"
  // showed up as a spurious peak hour even when no entry was at midnight.
  if (Number.isFinite(entry.visitTime) && entry.visitTime > 0) {
    const ms = entry.visitTime > 1e12 ? entry.visitTime : entry.visitTime * 1000
    return new Date(ms).getHours()
  }
  const parsed = Date.parse(entry.visitedAt)
  if (Number.isNaN(parsed)) return -1
  return new Date(parsed).getHours()
}
