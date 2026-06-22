/**
 * PaperSearchView — composed Search shell.
 *
 * Wraps the literary hero on top, then either the empty-state surface
 * (when query is empty) or the day-grouped results list (when query has
 * matches) or a "memory is patient" empty-results message (when query
 * has no matches).
 *
 * ## Responsibilities
 * - Stitch the hero + (empty | results | no-matches) branches together.
 * - Render results day-grouped per the design's `.sv-day-group` layout.
 *
 * ## Not responsible for
 * - Query execution or grouping logic — caller supplies pre-grouped
 *   results.
 */

import type { ReactNode, Ref } from 'react'
import { cn } from '@/lib/cn'
import {
  PaperSearchHero,
  type PaperSearchHeroCopy,
  type PaperSearchHeroFilter,
  type PaperSearchMode,
} from './paper-search-hero'
import {
  PaperSearchEmpty,
  type PaperSearchEmptyCopy,
  type PaperSearchRecent,
  type PaperSearchSuggestion,
} from './paper-search-empty'
import {
  PaperSearchResult,
  type PaperSearchResultEntry,
} from './paper-search-result'

export interface PaperSearchViewDayGroup {
  date: string
  /** Pretty label, e.g. "Yesterday" / "Saturday, May 17". */
  label: string
  entries: readonly PaperSearchResultEntry[]
}

export interface PaperSearchViewCopy {
  hero: PaperSearchHeroCopy
  empty: PaperSearchEmptyCopy
  /** Mono "N pages found" header above results, with {count} template. */
  resultsCount: string
  /** Mono range line, e.g. "{first} — {last} · {mode}". */
  resultsRange: string
  /** Pretty page suffix label, "page" / "pages". */
  pageSuffixSingular: string
  pageSuffixPlural: string
  /** Italic-serif message when query has no matches. */
  noMatchesTitle: string
  noMatchesBody: string
  seeInContextLabel: string
  /** Per-day right-aligned page count label. */
  dayCountTemplate: string
  /**
   * Label prefixing an enrichment excerpt on a result that matched fetched site
   * content (W-ENRICH-1). Optional — when omitted the excerpt renders without
   * the prefix.
   */
  enrichmentMatchLabel?: string
  /**
   * Smart (relevance) layout copy. Optional so the keyword-only callers + the
   * preview fixtures don't have to supply it. All keys are required once the
   * relevance layout is in use (the route always passes the full bundle).
   */
  relevance?: {
    /** Mono header above the ranked list, e.g. "{count} ranked by relevance". */
    rankedCount: string
    /** Per-row "Ask assistant" pill label. */
    askAssistantLabel: string
    /** Loading skeleton label while Smart results stream in. */
    loadingLabel: string
    /** Prev / next pagination button labels. */
    prevPageLabel: string
    nextPageLabel: string
    /** Mono pagination summary, e.g. "Page {page}". Bare-ordinal fallback. */
    pageSummary: string
    /**
     * I2: honest pagination summary that bounds the page within the result set,
     * e.g. "Page {page} · {total} ranked". Used when a real `total` is known.
     */
    pageSummaryRanked: string
    /** I2: position hint when more pages exist (a next cursor is available). */
    moreAvailable: string
    /** I2: position hint on the last page (no next cursor). */
    endOfResults: string
  }
}

export interface PaperSearchViewPagination {
  /** Disable the prev control (no earlier page). */
  prevDisabled: boolean
  /** Disable the next control (no later page / no cursor). */
  nextDisabled: boolean
  onPrev: () => void
  onNext: () => void
  /** 1-based page number for the summary line. */
  page: number
  /**
   * I2: total ranked results across the whole set (`AiSearchResponse.total`).
   * When present the summary bounds the page within the set ("Page N · M
   * ranked"); when absent it falls back to the bare ordinal. The next-cursor
   * state (`nextDisabled`) drives an honest "more available" / "end of results"
   * position hint either way.
   */
  total?: number | null
}

export interface PaperSearchViewProps {
  query: string
  mode: PaperSearchMode
  activeFilters: readonly PaperSearchHeroFilter[]
  /** Pre-grouped result days (already sorted newest → oldest). */
  groups: readonly PaperSearchViewDayGroup[]
  totalResults: number
  /**
   * Result layout. 'day-grouped' (default) keeps the byte-for-byte keyword/regex
   * UX. 'relevance' renders a flat, RRF-ranked list for Smart results and turns
   * on the AI states + pagination below. The route gates ALL new behavior behind
   * this flag so the keyword path is unchanged.
   */
  resultLayout?: 'day-grouped' | 'relevance'
  /** Pre-ranked relevance entries (only read in the 'relevance' layout). */
  rankedEntries?: readonly PaperSearchResultEntry[]
  /** Smart-search loading flag — shows the in-place skeleton in the results region. */
  aiLoading?: boolean
  /**
   * Smart-search error, surfaced via the `belowHeroSlot` callout by the route.
   * Passed here only so the view can suppress the empty/no-matches branches
   * while an error is showing (the composer must never unmount mid-error).
   */
  aiError?: string | null
  /** Backend notes (e.g. "lexical-only fallback") rendered above the ranked list. */
  aiNotes?: readonly string[]
  /** Prev/next cursor pagination (only read in the 'relevance' layout). */
  pagination?: PaperSearchViewPagination | null
  /**
   * I3: pre-formatted scope / freshness micro-line shown under the ranked-count
   * header (e.g. "1,240 pages indexed · updated May 17"). The route composes it
   * from real index data only — omitting any piece whose datum is unavailable —
   * and passes `null` when there is nothing honest to say.
   */
  relevanceScopeLine?: string | null
  /** Whether the Smart tab is selectable (forwarded to the hero). */
  smartAvailable?: boolean
  /** Per-row "Ask assistant" handler (relevance rows only). */
  onAskAssistant?: (entry: PaperSearchResultEntry) => void
  /** Suggestion cards for the empty state. */
  suggestions?: readonly PaperSearchSuggestion[]
  recent?: readonly PaperSearchRecent[]
  resolveDomainColor: (domain: string) => string
  resolveDomainAbbr: (domain: string) => string
  onQueryChange: (next: string) => void
  onModeChange: (next: PaperSearchMode) => void
  onRemoveFilter: (id: string) => void
  onSubmit?: (query: string) => void
  onSelectEntry?: (entry: PaperSearchResultEntry) => void
  onSeeInContext?: (entry: PaperSearchResultEntry, dayDate: string) => void
  /**
   * Optional star provider for each result row, keyed by the result's URL.
   * Omit to keep results star-free.
   */
  entryStar?: {
    isStarred: (url: string) => boolean
    onToggle: (url: string) => void
    starLabel: string
    unstarLabel: string
  }
  onPickSuggestion?: (suggestion: PaperSearchSuggestion) => void
  onRunRecent?: (recent: PaperSearchRecent) => void
  onAddDateFilter?: () => void
  onAddSourceFilter?: () => void
  onAddDomainFilter?: () => void
  onAddVisitCountFilter?: () => void
  onAddTagFilter?: () => void
  onAddNoteFilter?: () => void
  /**
   * Forwarded to the hero's input element so the panel can move focus
   * back to the input after appending an operator chip (e.g. clicking
   * `+ Tag` should leave the caret right after `tag:` ready to type).
   */
  inputRef?: Ref<HTMLInputElement>
  /** Optional slot rendered below the hero before the results (e.g. for
      provider gating callouts in semantic mode). */
  belowHeroSlot?: ReactNode
  /**
   * Optional slot rendered at the top of the relevance results region (the
   * compact index-status + "Build index" CTA the route mounts in Smart mode).
   * Only shown in the 'relevance' layout so the keyword surface stays bare.
   */
  relevanceHeaderSlot?: ReactNode
  copy: PaperSearchViewCopy
  className?: string
  testId?: string
}

export function PaperSearchView({
  query,
  mode,
  activeFilters,
  groups,
  totalResults,
  resultLayout = 'day-grouped',
  rankedEntries = [],
  aiLoading = false,
  aiError = null,
  aiNotes = [],
  pagination = null,
  relevanceScopeLine = null,
  smartAvailable = true,
  onAskAssistant,
  suggestions = [],
  recent = [],
  resolveDomainColor,
  resolveDomainAbbr,
  onQueryChange,
  onModeChange,
  onRemoveFilter,
  onSubmit,
  onSelectEntry,
  onSeeInContext,
  entryStar,
  onPickSuggestion,
  onRunRecent,
  onAddDateFilter,
  onAddSourceFilter,
  onAddDomainFilter,
  onAddVisitCountFilter,
  onAddTagFilter,
  onAddNoteFilter,
  inputRef,
  belowHeroSlot,
  relevanceHeaderSlot,
  copy,
  className,
  testId,
}: PaperSearchViewProps) {
  const hasQuery = query.trim().length > 0
  const isRelevance = resultLayout === 'relevance'
  const hasResults = isRelevance ? rankedEntries.length > 0 : totalResults > 0
  const oldestDate = groups[groups.length - 1]?.date ?? ''
  const newestDate = groups[0]?.date ?? ''

  return (
    <section
      data-testid={testId}
      className={cn('flex w-full flex-col', className)}
    >
      <PaperSearchHero
        ref={inputRef}
        query={query}
        mode={mode}
        activeFilters={activeFilters}
        onQueryChange={onQueryChange}
        onModeChange={onModeChange}
        onRemoveFilter={onRemoveFilter}
        onSubmit={onSubmit}
        onAddDateFilter={onAddDateFilter}
        onAddSourceFilter={onAddSourceFilter}
        onAddDomainFilter={onAddDomainFilter}
        onAddVisitCountFilter={onAddVisitCountFilter}
        onAddTagFilter={onAddTagFilter}
        onAddNoteFilter={onAddNoteFilter}
        smartAvailable={smartAvailable}
        copy={copy.hero}
      />

      {belowHeroSlot}

      {isRelevance ? (
        <RelevanceResults
          query={query}
          rankedEntries={rankedEntries}
          hasQuery={hasQuery}
          hasResults={hasResults}
          aiLoading={aiLoading}
          aiError={aiError}
          aiNotes={aiNotes}
          pagination={pagination}
          relevanceScopeLine={relevanceScopeLine}
          resolveDomainColor={resolveDomainColor}
          resolveDomainAbbr={resolveDomainAbbr}
          onSelectEntry={onSelectEntry}
          onSeeInContext={onSeeInContext}
          onAskAssistant={onAskAssistant}
          entryStar={entryStar}
          relevanceHeaderSlot={relevanceHeaderSlot}
          copy={copy}
        />
      ) : !hasQuery ? (
        <PaperSearchEmpty
          suggestions={suggestions}
          recent={recent}
          onPickSuggestion={onPickSuggestion}
          onRunRecent={onRunRecent}
          copy={copy.empty}
          testId="paper-search-empty"
        />
      ) : !hasResults ? (
        <div
          data-testid="paper-search-no-matches"
          className="mx-auto max-w-[720px] py-12 text-center"
        >
          <div className="text-ink-faint font-serif text-[16px] italic leading-[1.5]">
            {copy.noMatchesTitle}
          </div>
          <div className="text-ink-faint mt-2 font-mono text-[10px]">
            {copy.noMatchesBody}
          </div>
        </div>
      ) : (
        <div
          data-testid="paper-search-results"
          className="mx-auto max-w-[920px]"
        >
          <div className="border-border-light mb-[18px] flex items-baseline justify-between border-b pb-[10px]">
            <div className="text-ink font-serif text-[16px]">
              <strong className="text-accent font-medium">
                {totalResults.toLocaleString()}
              </strong>{' '}
              {copy.resultsCount
                .replace(
                  '{noun}',
                  totalResults === 1
                    ? copy.pageSuffixSingular
                    : copy.pageSuffixPlural,
                )
                .replace('{count}', '')}
            </div>
            <div className="text-ink-faint font-mono text-[11px]">
              {copy.resultsRange
                .replace('{first}', oldestDate)
                .replace('{last}', newestDate)
                .replace('{mode}', mode)}
            </div>
          </div>

          {groups.map((group) => (
            <div
              key={group.date}
              data-day={group.date}
              data-testid={`paper-search-day-${group.date}`}
              className="mb-6"
            >
              <div className="border-border-light mb-[10px] flex items-baseline gap-3 border-b pb-[6px]">
                <span className="text-ink font-serif text-[15px] font-medium tracking-[-0.005em]">
                  {group.label}
                </span>
                <span className="text-ink-faint font-mono text-[10.5px]">
                  {copy.dayCountTemplate
                    .replace('{count}', String(group.entries.length))
                    .replace(
                      '{noun}',
                      group.entries.length === 1
                        ? copy.pageSuffixSingular
                        : copy.pageSuffixPlural,
                    )}
                </span>
              </div>

              {group.entries.map((entry) => (
                <PaperSearchResult
                  key={entry.id}
                  entry={entry}
                  query={query}
                  domainColor={resolveDomainColor(entry.domain)}
                  domainAbbr={resolveDomainAbbr(entry.domain)}
                  onSelect={onSelectEntry}
                  onSeeInContext={
                    onSeeInContext
                      ? (chosen) => onSeeInContext(chosen, group.date)
                      : undefined
                  }
                  seeInContextLabel={copy.seeInContextLabel}
                  enrichmentMatchLabel={copy.enrichmentMatchLabel}
                  star={
                    entryStar && entry.url
                      ? {
                          starred: entryStar.isStarred(entry.url),
                          onToggle: () => entryStar.onToggle(entry.url),
                          starLabel: entryStar.starLabel,
                          unstarLabel: entryStar.unstarLabel,
                        }
                      : undefined
                  }
                />
              ))}
            </div>
          ))}
        </div>
      )}
    </section>
  )
}

interface RelevanceResultsProps {
  query: string
  rankedEntries: readonly PaperSearchResultEntry[]
  hasQuery: boolean
  hasResults: boolean
  aiLoading: boolean
  aiError: string | null
  aiNotes: readonly string[]
  pagination: PaperSearchViewPagination | null
  relevanceScopeLine: string | null
  resolveDomainColor: (domain: string) => string
  resolveDomainAbbr: (domain: string) => string
  onSelectEntry?: (entry: PaperSearchResultEntry) => void
  onSeeInContext?: (entry: PaperSearchResultEntry, dayDate: string) => void
  onAskAssistant?: (entry: PaperSearchResultEntry) => void
  entryStar?: PaperSearchViewProps['entryStar']
  relevanceHeaderSlot?: ReactNode
  copy: PaperSearchViewCopy
}

/**
 * Flat, RRF-ranked Smart-search result list. Deliberately NOT day-grouped:
 * hybrid relevance order is the whole point of Smart search, so we preserve the
 * backend ranking top-down. Routes the AI loading / empty / "no matches" states
 * through the same in-place result region the keyword path uses so the composer
 * above never unmounts mid-error.
 */
function RelevanceResults({
  query,
  rankedEntries,
  hasQuery,
  hasResults,
  aiLoading,
  aiError,
  aiNotes,
  pagination,
  relevanceScopeLine,
  resolveDomainColor,
  resolveDomainAbbr,
  onSelectEntry,
  onSeeInContext,
  onAskAssistant,
  entryStar,
  relevanceHeaderSlot,
  copy,
}: RelevanceResultsProps) {
  const relevanceCopy = copy.relevance
  // I2: bound the page within the ranked set. A real `total`
  // (`AiSearchResponse.total`) upgrades the bare "Page N" ordinal to
  // "Page N · M ranked"; the next-cursor state adds an honest
  // "more available" / "end of results" position hint so the user always knows
  // where they are. When `total` is genuinely unknown we keep the plain ordinal
  // rather than inventing a count.
  const paginationSummary = pagination
    ? buildRelevancePaginationSummary(pagination, relevanceCopy)
    : null
  return (
    <div
      data-testid="paper-search-relevance"
      className="mx-auto w-full max-w-[920px]"
    >
      {relevanceHeaderSlot ? (
        <div data-testid="paper-search-relevance-header" className="mb-4">
          {relevanceHeaderSlot}
        </div>
      ) : null}

      {aiLoading ? (
        <div
          data-testid="paper-search-relevance-loading"
          className="py-10 text-center"
        >
          <div className="text-ink-faint font-mono text-[11px] uppercase tracking-[0.08em]">
            {relevanceCopy?.loadingLabel}
          </div>
          <div className="mt-3 flex flex-col gap-2">
            {[0, 1, 2].map((row) => (
              <div
                key={row}
                aria-hidden="true"
                className="bg-hover rounded-paper h-[44px] w-full animate-pulse"
              />
            ))}
          </div>
        </div>
      ) : aiError ? (
        // The error itself renders in the route's belowHeroSlot callout; here we
        // just hold the region empty (the composer stays mounted above) so the
        // user can edit and retry without a layout jump.
        <div data-testid="paper-search-relevance-error-region" />
      ) : !hasQuery ? (
        <div
          data-testid="paper-search-relevance-prompt"
          className="mx-auto max-w-[720px] py-12 text-center"
        >
          <div className="text-ink-faint font-serif text-[16px] italic leading-[1.5]">
            {copy.empty.footer}
          </div>
        </div>
      ) : !hasResults ? (
        <div
          data-testid="paper-search-no-matches"
          className="mx-auto max-w-[720px] py-12 text-center"
        >
          <div className="text-ink-faint font-serif text-[16px] italic leading-[1.5]">
            {copy.noMatchesTitle}
          </div>
          <div className="text-ink-faint mt-2 font-mono text-[10px]">
            {copy.noMatchesBody}
          </div>
        </div>
      ) : (
        <div data-testid="paper-search-results">
          <div className="border-border-light mb-[18px] border-b pb-[10px]">
            <div className="flex items-baseline justify-between">
              <div className="text-ink font-serif text-[16px]">
                <strong className="text-accent font-medium">
                  {rankedEntries.length.toLocaleString()}
                </strong>{' '}
                {(relevanceCopy?.rankedCount ?? '').replace('{count}', '')}
              </div>
            </div>
            {/* I3: scope / freshness micro-line — index coverage + last-indexed
                so the user can tell a fully-indexed archive from a sparse one.
                Only rendered when the route has something honest to say. */}
            {relevanceScopeLine ? (
              <div
                data-testid="paper-search-relevance-scope"
                className="text-ink-faint mt-1 font-mono text-[10px] tracking-[0.02em]"
              >
                {relevanceScopeLine}
              </div>
            ) : null}
          </div>

          {aiNotes.length > 0 ? (
            <div
              data-testid="paper-search-relevance-notes"
              className="mb-4 flex flex-col gap-1"
            >
              {aiNotes.map((note) => (
                <p
                  key={note}
                  className="text-ink-muted font-mono text-[10px] tracking-[0.02em]"
                >
                  {note}
                </p>
              ))}
            </div>
          ) : null}

          {rankedEntries.map((entry) => (
            <PaperSearchResult
              key={entry.id}
              entry={entry}
              query={query}
              domainColor={resolveDomainColor(entry.domain)}
              domainAbbr={resolveDomainAbbr(entry.domain)}
              onSelect={onSelectEntry}
              onSeeInContext={
                // The button only renders when `entry.dayKey` is set, so the
                // captured `dayKey` is always a real Browse day here.
                onSeeInContext && entry.dayKey
                  ? (chosen) => onSeeInContext(chosen, entry.dayKey as string)
                  : undefined
              }
              seeInContextLabel={copy.seeInContextLabel}
              onAskAssistant={onAskAssistant}
              askAssistantLabel={relevanceCopy?.askAssistantLabel}
              star={
                entryStar && entry.url
                  ? {
                      starred: entryStar.isStarred(entry.url),
                      onToggle: () => entryStar.onToggle(entry.url),
                      starLabel: entryStar.starLabel,
                      unstarLabel: entryStar.unstarLabel,
                    }
                  : undefined
              }
            />
          ))}

          {pagination ? (
            <div
              data-testid="paper-search-relevance-pagination"
              className="border-border-light mt-6 flex items-center justify-between border-t pt-4"
            >
              <button
                type="button"
                onClick={pagination.onPrev}
                disabled={pagination.prevDisabled}
                data-testid="paper-search-relevance-prev"
                className={cn(
                  'border-border-default text-ink-secondary rounded-pill border px-[12px] py-[4px]',
                  'font-mono text-[10.5px] tracking-[0.02em]',
                  'enabled:hover:border-ink-muted enabled:cursor-pointer',
                  'disabled:cursor-not-allowed disabled:opacity-40',
                )}
              >
                ← {relevanceCopy?.prevPageLabel}
              </button>
              <span
                className="text-ink-faint flex flex-col items-center font-mono text-[10px]"
                data-testid="paper-search-relevance-page-summary"
              >
                <span>{paginationSummary?.summary}</span>
                <span
                  className="text-ink-faint/70"
                  data-testid="paper-search-relevance-position"
                >
                  {paginationSummary?.position}
                </span>
              </span>
              <button
                type="button"
                onClick={pagination.onNext}
                disabled={pagination.nextDisabled}
                data-testid="paper-search-relevance-next"
                className={cn(
                  'border-border-default text-ink-secondary rounded-pill border px-[12px] py-[4px]',
                  'font-mono text-[10.5px] tracking-[0.02em]',
                  'enabled:hover:border-ink-muted enabled:cursor-pointer',
                  'disabled:cursor-not-allowed disabled:opacity-40',
                )}
              >
                {relevanceCopy?.nextPageLabel} →
              </button>
            </div>
          ) : null}
        </div>
      )}
    </div>
  )
}

/**
 * I2: build the honest relevance pagination summary + position hint.
 *
 * - `summary` upgrades the bare "Page {page}" ordinal to "Page {page} ·
 *   {total} ranked" whenever a real total is known (`AiSearchResponse.total`),
 *   falling back to the plain ordinal when it is not — never a fabricated count.
 * - `position` reads the cursor state: a disabled Next means the user is on the
 *   last page ("end of results"); an enabled Next means there is "more
 *   available". This bounds the page even when the total is unknown.
 */
function buildRelevancePaginationSummary(
  pagination: PaperSearchViewPagination,
  relevanceCopy: PaperSearchViewCopy['relevance'],
): { summary: string; position: string } {
  const page = String(pagination.page)
  const hasTotal =
    pagination.total !== null &&
    pagination.total !== undefined &&
    Number.isFinite(pagination.total)
  const summary = hasTotal
    ? (relevanceCopy?.pageSummaryRanked ?? '')
        .replace('{page}', page)
        .replace('{total}', (pagination.total as number).toLocaleString())
    : (relevanceCopy?.pageSummary ?? '').replace('{page}', page)
  const position = pagination.nextDisabled
    ? (relevanceCopy?.endOfResults ?? '')
    : (relevanceCopy?.moreAvailable ?? '')
  return { summary, position }
}
