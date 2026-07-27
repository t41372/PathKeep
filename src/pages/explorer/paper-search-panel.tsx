/**
 * Paper-redesign Search surface mounted via `?layout=paper&surface=search`.
 * Wraps PaperSearchView and adapts the existing Explorer URL state
 * (`queryInput` / `mode` / `regexMode`) into the paper hero's three-mode
 * + day-grouped-result shape.
 *
 * ## Responsibilities
 * - Translate explorer mode + regex flag to the paper search-mode ternary.
 * - Build day groups from the visible history results.
 * - Forward query / mode / submit / select / see-in-context interactions
 *   back to the route via lightweight callbacks.
 *
 * ## Not responsible for
 * - Fetching results — the route still owns the queryState + history hooks.
 * - URL parsing — the route hands typed callbacks for each side effect.
 */

import { useCallback, useEffect, useMemo, useRef, type ReactNode } from 'react'
import type { HistoryEntry } from '@/lib/types/archive'
import {
  PaperSearchView,
  type PaperSearchHeroFilter,
  type PaperSearchMode,
  type PaperSearchRecent,
  type PaperSearchResultEntry,
  type PaperSearchViewOffsetPagination,
  type PaperSearchViewPagination,
} from '@/components/explorer-paper'
import { StatusCallout } from '@/components/primitives/status-callout'
import { buildPaperSearchViewCopy } from './paper-explorer-copy'
import {
  appendOperator,
  hasStarredFacet,
  parseActiveSearchFilters,
  removeFilterToken,
} from './paper-search-filters'
import {
  buildPaperSearchDayGroups,
  explorerStateFromPaperSearchMode,
  paperSearchModeFromExplorerState,
} from './paper-search-helpers'
import { getDomainAbbr, getDomainColor } from './paper/domain-color'
import type { ExplorerMode, RecentSearchEntry } from './types'

export interface PaperSearchPanelAboveResultsCallout {
  tone: 'info' | 'blocked' | 'warning'
  eyebrow: string
  title: string
  body: string
}

export interface PaperSearchPanelProps {
  /** Active query text. */
  query: string
  /** Current explorer-side mode + regex flag. */
  mode: ExplorerMode
  regexMode: boolean
  /** History entries currently visible to the route. */
  entries: readonly HistoryEntry[]
  totalResults: number
  language: string
  explorerT: (key: string, vars?: Record<string, string | number>) => string
  /**
   * Optional StatusCallout rendered below the hero composer, above the
   * results list. Lets the route surface query-blocking errors (invalid
   * regex, backend failure) without unmounting the composer — see
   * feedback-2026-05-25 §3.2 B for why this is mandatory on the search
   * surface.
   */
  aboveResultsCallout?: PaperSearchPanelAboveResultsCallout | null
  /**
   * Smart-search relevance entries — pre-ranked by the route (REACH-B). When
   * the active mode is Smart the panel renders these via the relevance layout
   * instead of day-grouping `entries`. Day-grouped keyword behavior is byte-for
   * -byte unchanged because the relevance path is gated behind the Smart mode.
   */
  rankedEntries?: readonly PaperSearchResultEntry[]
  /** Smart-search loading flag (in-place skeleton). */
  aiLoading?: boolean
  /** Smart-search error (suppresses empty/no-match branches while showing). */
  aiError?: string | null
  /** Backend notes for the ranked list (e.g. lexical-only fallback). */
  aiNotes?: readonly string[]
  /** Prev/next cursor pagination for the relevance list. */
  pagination?: PaperSearchViewPagination | null
  /**
   * Offset pagination for the day-grouped keyword / regex list. The two
   * pagination models stay separate on purpose (cursor vs. page number) —
   * see `PaperSearchViewOffsetPagination`. Whichever layout is inactive gets
   * `null` so a stale descriptor can never leak into the other list.
   */
  offsetPagination?: PaperSearchViewOffsetPagination | null
  /**
   * I3: pre-formatted scope / freshness micro-line for the ranked header
   * (index coverage + last-indexed). `null` when there is nothing honest to say.
   */
  relevanceScopeLine?: string | null
  /** Whether the Smart tab is selectable (REACH-A gating). */
  smartAvailable?: boolean
  /** Per-row "Ask assistant" handler (relevance rows only). */
  onAskAssistant?: (entry: PaperSearchResultEntry) => void
  /** Compact index-status + Build-CTA slot mounted atop the relevance results. */
  relevanceHeaderSlot?: ReactNode
  /** Apply text input changes to URL + local state. */
  onQueryChange: (next: string) => void
  /** Apply explorer mode + regex flag from the paper hero. */
  onModeChange: (next: { mode: ExplorerMode; regexMode: boolean }) => void
  /** Submit handler (Enter in the hero, or the Search button). */
  onSubmit: (query: string) => void
  /** True while the last-submitted query is in flight (button spinner). */
  isSearching?: boolean
  /**
   * Disable the Search button — empty draft, or the draft + mode match the last
   * submission (no redundant identical query). The route owns this gate.
   */
  searchSubmitDisabled?: boolean
  /**
   * The last-submitted paper mode when it differs from the live mode and there
   * are visible results — drives the hero's stale-results banner. `null` hides
   * it.
   */
  staleResultsMode?: PaperSearchMode | null
  /** Selected entry id update — number coerced from the paper id. */
  onSelectEntry: (id: number) => void
  /** Jump back to PaperExplorerView centred on the result's day. */
  onSeeInContext: (entry: PaperSearchResultEntry, dayDate: string) => void
  /**
   * Recent searches persisted by `useExplorerUrlState`. Rendered in the
   * empty state so the writes that hook has always made are finally read
   * back; clicking one re-runs it through the normal submit path.
   */
  recentSearches?: readonly RecentSearchEntry[]
  /** Optional star provider forwarded to each search result row. */
  entryStar?: {
    isStarred: (url: string) => boolean
    onToggle: (url: string) => void
    starLabel: string
    unstarLabel: string
  }
}

export function PaperSearchPanel({
  query,
  mode,
  regexMode,
  entries,
  totalResults,
  language,
  explorerT,
  aboveResultsCallout,
  rankedEntries,
  aiLoading = false,
  aiError = null,
  aiNotes,
  pagination = null,
  offsetPagination = null,
  relevanceScopeLine = null,
  smartAvailable = true,
  onAskAssistant,
  relevanceHeaderSlot,
  onQueryChange,
  onModeChange,
  onSubmit,
  isSearching = false,
  searchSubmitDisabled = false,
  staleResultsMode = null,
  onSelectEntry,
  onSeeInContext,
  recentSearches,
  entryStar,
}: PaperSearchPanelProps) {
  const inputRef = useRef<HTMLInputElement | null>(null)
  // The Smart (relevance) layout is gated behind the active mode so keyword /
  // regex search renders the exact day-grouped UX it always has. Both AI URL
  // modes — the real `hybrid` and the legacy `semantic` alias — light up Smart.
  // An `is:starred` query is the one exception: it always renders the TRUE
  // starred set through the keyword (day-grouped) layout, never the ranked view,
  // matching the route's `smartSearchActive` gate.
  const paperMode = paperSearchModeFromExplorerState(mode, regexMode)
  const isRelevance = paperMode === 'smart' && !hasStarredFacet(query)

  const activeFilters = useMemo<PaperSearchHeroFilter[]>(
    () =>
      parseActiveSearchFilters(query).map((filter) => ({
        id: filter.id,
        label: filter.label,
      })),
    [query],
  )

  // Pending rAF handle for the input-focus-and-place-caret side effect.
  // Tracking it in a ref lets rapid successive chip clicks cancel the
  // earlier frame before scheduling the new one — otherwise rAF1
  // would write a stale caret index against rAF2's already-committed
  // query (review §6 race).
  const focusFrameRef = useRef<number | null>(null)
  useEffect(() => {
    // Cancel any pending focus frame on unmount so the rAF callback
    // never fires against a stale input ref.
    return () => {
      if (focusFrameRef.current !== null) {
        window.cancelAnimationFrame(focusFrameRef.current)
        focusFrameRef.current = null
      }
    }
  }, [])

  const focusInputAtEnd = useCallback((nextQuery: string) => {
    if (focusFrameRef.current !== null) {
      window.cancelAnimationFrame(focusFrameRef.current)
    }
    focusFrameRef.current = window.requestAnimationFrame(() => {
      focusFrameRef.current = null
      const input = inputRef.current
      if (!input) return
      input.focus()
      const cursor = nextQuery.length
      try {
        input.setSelectionRange(cursor, cursor)
      } catch {
        /* Some input types reject setSelectionRange; focus is enough. */
      }
    })
  }, [])

  const appendFilterOperator = useCallback(
    (operator: string) => {
      const next = appendOperator(query, operator)
      // `appendOperator` returns the query verbatim when the operator name
      // fails its `^[a-z]+$` check. Suppress the spurious state setter +
      // caret refocus so an invalid chip click doesn't disrupt an open
      // dropdown or in-progress IME composition. The only production
      // callers pass literal 'tag' / 'note', so this branch is currently
      // unreachable from real chip clicks — keep it for future i18n /
      // plugin-registered operators.
      // Stryker disable next-line ConditionalExpression: defensive guard.
      /* v8 ignore next -- defensive: unreachable, see comment above. */
      if (next === query) return
      onQueryChange(next)
      focusInputAtEnd(next)
    },
    [query, onQueryChange, focusInputAtEnd],
  )

  const handleRemoveFilter = useCallback(
    (id: string) => {
      const filter = parseActiveSearchFilters(query).find(
        (candidate) => candidate.id === id,
      )
      if (!filter) return
      onQueryChange(removeFilterToken(query, filter.tokenIndex))
    },
    [query, onQueryChange],
  )

  const handleAddTagFilter = useCallback(
    () => appendFilterOperator('tag'),
    [appendFilterOperator],
  )

  const handleAddNoteFilter = useCallback(
    () => appendFilterOperator('note'),
    [appendFilterOperator],
  )

  const recent = useMemo(
    () => toPaperSearchRecents(recentSearches, language),
    [recentSearches, language],
  )

  // Re-running a recent search goes through the SAME submit path a typed
  // query takes (`onQueryChange` then `onSubmit`) rather than a second
  // bespoke route — so URL state, the stale-results banner, and the
  // recent-search write-back all stay consistent.
  const handleRunRecent = useCallback(
    (entry: PaperSearchRecent) => {
      // `entry.q` is the adapted row's own (trimmed, non-empty) query — see
      // `toPaperSearchRecents`. Reading it straight off the row instead of
      // looking the source entry back up keeps this total: there is no
      // "recent row with no matching entry" state to guard against.
      const nextMode = explorerStateFromPaperSearchMode(entry.mode)
      if (nextMode.mode !== mode || nextMode.regexMode !== regexMode) {
        onModeChange(nextMode)
      }
      onQueryChange(entry.q)
      onSubmit(entry.q)
    },
    [mode, regexMode, onModeChange, onQueryChange, onSubmit],
  )

  return (
    <PaperSearchView
      query={query}
      mode={paperMode}
      activeFilters={activeFilters}
      groups={
        isRelevance
          ? []
          : buildPaperSearchDayGroups(entries, {
              language,
              enrichmentSourceLabel: explorerT(
                'paperSearchView.enrichmentSourceGeneric',
              ),
            })
      }
      totalResults={totalResults}
      resultLayout={isRelevance ? 'relevance' : 'day-grouped'}
      rankedEntries={rankedEntries}
      aiLoading={aiLoading}
      aiError={aiError}
      aiNotes={aiNotes}
      pagination={isRelevance ? pagination : null}
      offsetPagination={isRelevance ? null : offsetPagination}
      relevanceScopeLine={isRelevance ? relevanceScopeLine : null}
      smartAvailable={smartAvailable}
      onAskAssistant={onAskAssistant}
      relevanceHeaderSlot={relevanceHeaderSlot}
      resolveDomainColor={getDomainColor}
      resolveDomainAbbr={getDomainAbbr}
      inputRef={inputRef}
      onQueryChange={onQueryChange}
      onModeChange={(nextMode) =>
        onModeChange(explorerStateFromPaperSearchMode(nextMode))
      }
      onRemoveFilter={handleRemoveFilter}
      onAddTagFilter={handleAddTagFilter}
      onAddNoteFilter={handleAddNoteFilter}
      onSubmit={onSubmit}
      recent={recent}
      onRunRecent={handleRunRecent}
      isSearching={isSearching}
      submitDisabled={searchSubmitDisabled}
      staleMode={staleResultsMode}
      onSelectEntry={(entry) => onSelectEntry(Number(entry.id))}
      onSeeInContext={onSeeInContext}
      entryStar={entryStar}
      belowHeroSlot={
        aboveResultsCallout ? (
          <div
            className="mx-auto w-full max-w-[920px] py-3"
            data-testid="paper-search-above-results-callout"
          >
            <StatusCallout
              tone={aboveResultsCallout.tone}
              eyebrow={aboveResultsCallout.eyebrow}
              title={aboveResultsCallout.title}
              body={aboveResultsCallout.body}
            />
          </div>
        ) : null
      }
      copy={buildPaperSearchViewCopy(explorerT)}
      testId="explorer-paper-search-view"
    />
  )
}

/**
 * Stable identity for one persisted recent search. The stored entries carry no
 * id of their own, and two entries can share a query text with different
 * filters, so the id has to be derived from the whole params bag.
 */
function recentSearchId(entry: RecentSearchEntry): string {
  return JSON.stringify(entry.params)
}

/**
 * Adapts persisted recent searches into the empty state's row shape.
 *
 * Entries with no query text are dropped: the empty state's row renders the
 * query as its primary label, and a filters-only recall has nothing to show
 * there. `count` / `when` are omitted (not faked) for entries persisted before
 * those were recorded — the row then uses the brief caption.
 */
function toPaperSearchRecents(
  entries: readonly RecentSearchEntry[] | undefined,
  language: string,
): PaperSearchRecent[] {
  if (!entries?.length) return []
  return entries
    .filter((entry) => Boolean(entry.params.q?.trim()))
    .map((entry) => ({
      id: recentSearchId(entry),
      q: entry.params.q!.trim(),
      mode: paperSearchModeFromExplorerState(
        entry.params.mode ?? 'keyword',
        entry.params.regex === '1',
      ),
      count: entry.total,
      when:
        entry.at === undefined
          ? undefined
          : formatRecentWhen(entry.at, language),
    }))
}

/**
 * Formats a recent-search timestamp for the row caption. Uses the shared
 * locale so the caption matches the rest of the surface rather than the
 * host's default.
 */
function formatRecentWhen(epochMs: number, language: string): string {
  return new Date(epochMs).toLocaleDateString(language, {
    month: 'short',
    day: 'numeric',
  })
}
