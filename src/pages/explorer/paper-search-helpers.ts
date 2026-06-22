/**
 * Adapters that bridge the existing Explorer search data layer to the
 * paper-redesign PaperSearchView.
 *
 * The Explorer route already owns `queryInput`, `mode`, `regexMode`,
 * `visibleTimeResults`, and the cleared/grouped URL machinery. This module
 * keeps the mapping logic out of `index.tsx` so the route file stays
 * focused on layout decisions rather than data reshaping.
 *
 * ## Responsibilities
 * - Convert the Explorer keyword/semantic/regex mode trio into the paper
 *   `PaperSearchMode` ternary and back.
 * - Group a flat HistoryEntry stream into PaperSearchView day groups,
 *   preserving newest-first order.
 * - Format a HistoryEntry as a PaperSearchResultEntry (title fallback to
 *   url, local-time clock, transition kind label).
 *
 * ## Not responsible for
 * - Fetching results or owning URL state — the route hands them in.
 * - Locale-aware day labelling — Browse handles that via `prettyDay`; the
 *   Search day header uses the same helper so callers should pre-format
 *   when needed.
 */

import type { HistoryEntry } from '@/lib/types/archive'
import type {
  PaperSearchMode,
  PaperSearchResultEntry,
  PaperSearchViewDayGroup,
} from '@/components/explorer-paper'
import { scoreBand } from '@/lib/intelligence-ai-presentation'
import type {
  AiIndexStatus,
  AiQueueStatus,
  AiSearchResultItem,
} from '@/lib/types/intelligence'
import { localDayKey } from './paper/group-entries'
import { prettyDay } from './paper/date-helpers'
import type { ExplorerMode } from './types'

/**
 * Project the Explorer URL mode + regex flag onto the paper Search mode.
 *
 * The paper hero exposes three mutually-exclusive modes (Keyword | Regex |
 * Smart); the legacy state tracks regex as a sibling boolean which means we
 * have to flatten on read. REACH-B collapses both AI URL modes — the real
 * `hybrid` and the legacy `semantic` alias — onto the single 'smart' tab,
 * because `backend.searchAiHistory` runs the identical hybrid call for both
 * (`AiSearchRequest` carries no mode field), so exposing two AI tabs would be
 * dishonest.
 */
export function paperSearchModeFromExplorerState(
  mode: ExplorerMode,
  regexMode: boolean,
): PaperSearchMode {
  if (regexMode) return 'regex'
  if (mode === 'hybrid' || mode === 'semantic') return 'smart'
  return 'keyword'
}

/**
 * Inverse of `paperSearchModeFromExplorerState` returning the params that
 * should be persisted to the URL. Routes can spread the result onto their
 * existing `updateParam` calls.
 *
 * Smart maps to `?mode=hybrid` — the true backend behavior (RRF over lexical +
 * semantic recall). The legacy `?mode=semantic` URL is read back as the same
 * Smart tab (see `paperSearchModeFromExplorerState`) so old deep links keep
 * working, but we always WRITE `hybrid` so the surfaced mode is honest.
 */
export function explorerStateFromPaperSearchMode(next: PaperSearchMode): {
  mode: ExplorerMode
  regexMode: boolean
} {
  if (next === 'regex') return { mode: 'keyword', regexMode: true }
  if (next === 'smart') return { mode: 'hybrid', regexMode: false }
  return { mode: 'keyword', regexMode: false }
}

function formatLocalTime(visitedAt: string): string {
  const parsed = new Date(visitedAt)
  if (Number.isNaN(parsed.getTime())) return ''
  const hh = String(parsed.getHours()).padStart(2, '0')
  const mm = String(parsed.getMinutes()).padStart(2, '0')
  return `${hh}:${mm}`
}

function transitionTypeLabel(transition: number | null | undefined): string {
  if (transition === null || transition === undefined) return ''
  switch (transition) {
    case 0:
      return 'link'
    case 1:
      return 'typed'
    case 2:
      return 'auto-bookmark'
    case 3:
      return 'auto-subframe'
    case 4:
      return 'manual-subframe'
    case 5:
      return 'generated'
    case 6:
      return 'start-page'
    case 7:
      return 'form-submit'
    case 8:
      return 'reload'
    case 9:
      return 'keyword'
    case 10:
      return 'keyword-generated'
    default:
      return ''
  }
}

export function paperSearchEntryFromHistoryEntry(
  entry: HistoryEntry,
): PaperSearchResultEntry {
  return {
    id: entry.id,
    title: entry.title?.trim() ? entry.title : entry.url,
    url: entry.url,
    domain: entry.domain,
    time: formatLocalTime(entry.visitedAt),
    transitionType: transitionTypeLabel(entry.transition ?? null) || undefined,
    // Surface the enrichment excerpt when the lexical-search backend attached one
    // (W-ENRICH-1, 06 §6). Browse/regex/fuzzy/preview rows leave it null/undefined,
    // which the result row treats as "no excerpt" and suppresses the affordance.
    enrichmentExcerpt: entry.enrichmentExcerpt?.trim()
      ? entry.enrichmentExcerpt
      : undefined,
  }
}

export interface BuildPaperSearchDayGroupsOptions {
  /** BCP-47 language code for the day header label, e.g. "en" / "zh-CN". */
  language: string
}

/**
 * Bucket a flat HistoryEntry stream into PaperSearchView day groups,
 * newest-day first.
 *
 * Within a day, entries are sorted by visit time descending so the most
 * recent hit appears at the top. The day label uses `prettyDay` (long
 * weekday + month + day + year) — Search results don't get a "Today /
 * Yesterday" pill the way Browse does, because Search routinely returns
 * matches from years ago.
 */
export function buildPaperSearchDayGroups(
  entries: readonly HistoryEntry[],
  options: BuildPaperSearchDayGroupsOptions,
): PaperSearchViewDayGroup[] {
  if (entries.length === 0) return []
  const byDay = new Map<string, HistoryEntry[]>()
  for (const entry of entries) {
    const key = localDayKey(entry.visitedAt)
    const bucket = byDay.get(key)
    if (bucket) {
      bucket.push(entry)
    } else {
      byDay.set(key, [entry])
    }
  }
  const days = Array.from(byDay.entries())
    .map(([date, dayEntries]) => {
      const sorted = [...dayEntries].sort((a, b) => {
        const aMs = Date.parse(a.visitedAt)
        const bMs = Date.parse(b.visitedAt)
        if (Number.isNaN(aMs) || Number.isNaN(bMs)) return 0
        return bMs - aMs
      })
      return {
        date,
        label: prettyDay(date, { language: options.language }),
        entries: sorted.map(paperSearchEntryFromHistoryEntry),
      }
    })
    .sort((a, b) => b.date.localeCompare(a.date))
  return days
}

/**
 * Translate accessor shared by the AI adapters below — pinned to the
 * `intelligence` namespace because `scoreBand`'s band labels live there.
 */
type IntelligenceTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Map one Smart-search result (`AiSearchResultItem`) onto the existing paper
 * `PaperSearchResultEntry` so relevance rows reuse the exact same row component
 * keyword rows render through.
 *
 * The honest part of REACH-B lives here: `AiSearchResultItem` has NO snippet
 * field, so we surface the backend `matchReason` as a mono caption (never a
 * faked snippet) and derive a `relevanceBand` pill from `score` via the shared
 * `scoreBand(...)`. `id` keeps the real `historyId`, so selecting a Smart row
 * opens the detail panel exactly like a keyword row.
 */
export function paperSearchEntryFromAiSearchItem(
  item: AiSearchResultItem,
  intelligenceT: IntelligenceTranslator,
): PaperSearchResultEntry {
  const band = scoreBand(item.score, intelligenceT)
  // `localDayKey` always returns a non-empty key (a real YYYY-MM-DD, the raw
  // 10-char prefix for an unparseable timestamp, or the literal 'unknown' for a
  // blank one), so the stamped `dayKey` is always present. "See in context"
  // lands the user on whatever Browse day matches it; we never fabricate a date.
  return {
    id: item.historyId,
    title: item.title?.trim() ? item.title : item.url,
    url: item.url,
    domain: item.domain,
    time: formatLocalTime(item.visitedAt),
    matchReason: item.matchReason,
    relevanceBand: { label: band.label, tone: band.tone },
    dayKey: localDayKey(item.visitedAt),
  }
}

/**
 * Build the flat, relevance-ranked entry list for the Smart layout. Unlike the
 * keyword day-grouping path, Smart results are RRF-ranked by relevance, so we
 * deliberately preserve the backend order (do NOT re-sort or bucket by day —
 * day-grouping would destroy the ranking the user needs to read top-down).
 */
export function buildPaperSearchRelevanceList(
  items: readonly AiSearchResultItem[],
  intelligenceT: IntelligenceTranslator,
): PaperSearchResultEntry[] {
  return items.map((item) =>
    paperSearchEntryFromAiSearchItem(item, intelligenceT),
  )
}

/**
 * The queue-job phase a semantic-index backfill is currently in, from the
 * caller's point of view.
 *
 * - `idle` — no backfill is queued or running.
 * - `queued` — a backfill is enqueued but the worker hasn't started it (or has
 *   started a different job). The CTA must NOT imply the index is built.
 * - `running` — a backfill is actively embedding rows.
 * - `paused` — a backfill is enqueued but the queue is paused, so it will not
 *   make progress until the user resumes it. The CTA says so honestly.
 */
export type SmartIndexPhase = 'idle' | 'queued' | 'running' | 'paused'

/**
 * Live, queue-derived view of the semantic-index backfill the in-surface Build
 * CTA renders (REACH-B B1).
 *
 * The honesty contract: every field here is read straight from the queue read
 * model (`AiQueueStatus`, refreshed by the route's bounded poll) or the shell
 * snapshot — never fabricated. The backend exposes NO total-candidate count to
 * the UI, so this deliberately carries no `percent`/`total`: the only honest
 * progress numbers are the queue's own queued/running job counts. A bare
 * `buildAiIndex` enqueue therefore surfaces as `queued`/`running` (or `paused`),
 * never as "built".
 */
export interface SmartIndexProgress {
  phase: SmartIndexPhase
  /** A backfill is queued or running (drives the CTA's busy/disabled state). */
  active: boolean
  /** Count of index jobs waiting in the queue (queued + paused + stale). */
  queuedJobs: number
  /** Count of index jobs the worker is currently running. */
  runningJobs: number
  /** Pages already embedded into the vector plane (grows as the build lands). */
  indexedItems: number
}

/**
 * Derive the live semantic-index backfill phase the in-surface Build CTA shows.
 *
 * Prefers the freshest signal: the live `AiQueueStatus` (re-fetched by the
 * route's bounded poll while a build is active) when present, falling back to
 * the shell snapshot's last-known queue counts so the CTA is honest even before
 * the first poll lands. `pendingAction` is the route's local "we just clicked
 * Build" flag — it keeps the CTA in the active state across the gap between the
 * click and the first queue read that observes the new job, so the CTA never
 * flickers back to "nothing to rank yet" on a bare enqueue.
 */
export function deriveSmartIndexProgress(args: {
  queueStatus: AiQueueStatus | null
  /**
   * Last-known queue counts from the shell snapshot. `null` only during the
   * shell-loading window (the search surface never renders then), so the
   * helper falls back to zeros rather than forcing callers to synthesize a
   * fake `AiIndexStatus`.
   */
  snapshotAiStatus: AiIndexStatus | null
  pendingAction: boolean
}): SmartIndexProgress {
  const { queueStatus, snapshotAiStatus, pendingAction } = args
  const queuedJobs = queueStatus?.queued ?? snapshotAiStatus?.queuedJobs ?? 0
  const runningJobs = queueStatus?.running ?? snapshotAiStatus?.runningJobs ?? 0
  const paused = queueStatus?.paused ?? snapshotAiStatus?.queuePaused ?? false
  const indexedItems = snapshotAiStatus?.indexedItems ?? 0
  const hasPendingJobs = queuedJobs > 0 || runningJobs > 0
  // The local click flag counts as "active" so the CTA reflects intent before
  // the first queue read observes the enqueued job.
  const active = pendingAction || hasPendingJobs

  let phase: SmartIndexPhase
  if (!active) {
    phase = 'idle'
  } else if (runningJobs > 0) {
    phase = 'running'
  } else if (paused && hasPendingJobs) {
    // Enqueued but the queue is paused: it will not progress until resumed. Be
    // honest rather than implying the build is underway.
    phase = 'paused'
  } else {
    phase = 'queued'
  }

  return { phase, active, queuedJobs, runningJobs, indexedItems }
}

/**
 * Compose the honest scope / freshness micro-line shown under the ranked-count
 * header (REACH-B I3).
 *
 * Built from REAL index data only: the indexed-page coverage count and a
 * last-indexed freshness timestamp. Each piece is omitted when its datum is
 * unavailable (no provider has indexed anything yet → no count; the status never
 * recorded a `lastIndexedAt` → no freshness), so the line never fakes coverage
 * it cannot prove. Returns `null` when there is nothing honest to say, which the
 * view treats as "render no micro-line".
 *
 * Pure + cheap (a couple of string formats), so it is safe to call in render.
 */
export function buildSmartScopeLine(args: {
  indexedItems: number
  lastIndexedAt: string | null | undefined
  language: string
  explorerT: (key: string, vars?: Record<string, string | number>) => string
}): string | null {
  const { indexedItems, lastIndexedAt, language, explorerT } = args
  const pieces: string[] = []
  if (indexedItems > 0) {
    pieces.push(
      explorerT('paperSearchView.relevanceScopeIndexed', {
        count: indexedItems.toLocaleString(language),
      }),
    )
  }
  const freshness = formatScopeFreshness(lastIndexedAt, language)
  if (freshness) {
    pieces.push(
      explorerT('paperSearchView.relevanceScopeUpdated', { date: freshness }),
    )
  }
  if (pieces.length === 0) return null
  return pieces.join(' · ')
}

/**
 * Format a last-indexed RFC3339 timestamp into a compact, locale-aware day
 * label (e.g. "May 17, 2026"). Returns `null` for a missing or unparseable
 * value so the caller omits the freshness piece rather than printing "Invalid
 * Date".
 */
function formatScopeFreshness(
  lastIndexedAt: string | null | undefined,
  language: string,
): string | null {
  if (!lastIndexedAt) return null
  const parsed = new Date(lastIndexedAt)
  if (Number.isNaN(parsed.getTime())) return null
  return new Intl.DateTimeFormat(language, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  }).format(parsed)
}
