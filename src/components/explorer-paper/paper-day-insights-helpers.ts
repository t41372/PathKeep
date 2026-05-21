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
}

/**
 * Aggregates one day's visits into the shape the day-insights strip needs.
 *
 * Returns zeros / empty arrays for days with no visits; callers should
 * guard at the render boundary rather than expecting non-empty.
 */
export function aggregateDayInsights(day: PaperDay): DayInsights {
  const domainCounts = new Map<string, number>()
  const hourBuckets = new Array<number>(24).fill(0)
  let totalPages = 0
  let typedCount = 0
  let linkCount = 0
  let searchCount = 0

  for (const session of day.sessions) {
    for (const block of session.blocks) {
      if (block.type === 'single') {
        accumulate(block.entry, domainCounts, hourBuckets, (deltas) => {
          totalPages += deltas.page
          typedCount += deltas.typed
          linkCount += deltas.link
          searchCount += deltas.search
        })
      } else {
        for (const entry of block.entries) {
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
  }
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
  // Prefer the second-precision visitTime (Chrome epoch already converted),
  // fall back to parsing visitedAt — both should land in the user's local
  // timezone since we want to bucket by what the wall clock read.
  if (Number.isFinite(entry.visitTime) && entry.visitTime > 0) {
    return new Date(entry.visitTime * 1000).getHours()
  }
  const parsed = Date.parse(entry.visitedAt)
  if (Number.isNaN(parsed)) return -1
  return new Date(parsed).getHours()
}
