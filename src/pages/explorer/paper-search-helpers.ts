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
import { localDayKey } from './paper/group-entries'
import { prettyDay } from './paper/date-helpers'
import type { ExplorerMode } from './types'

/**
 * Project the Explorer URL mode + regex flag onto the paper Search mode.
 *
 * The paper hero exposes three mutually-exclusive modes; the legacy state
 * tracks regex as a sibling boolean which means we have to flatten on read.
 */
export function paperSearchModeFromExplorerState(
  mode: ExplorerMode,
  regexMode: boolean,
): PaperSearchMode {
  if (regexMode) return 'regex'
  if (mode === 'semantic') return 'semantic'
  return 'keyword'
}

/**
 * Inverse of `paperSearchModeFromExplorerState` returning the params that
 * should be persisted to the URL. Routes can spread the result onto their
 * existing `updateParam` calls.
 */
export function explorerStateFromPaperSearchMode(next: PaperSearchMode): {
  mode: ExplorerMode
  regexMode: boolean
} {
  if (next === 'regex') return { mode: 'keyword', regexMode: true }
  if (next === 'semantic') return { mode: 'semantic', regexMode: false }
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
