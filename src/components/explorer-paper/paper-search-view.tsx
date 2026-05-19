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

import type { ReactNode } from 'react'
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
}

export interface PaperSearchViewProps {
  query: string
  mode: PaperSearchMode
  activeFilters: readonly PaperSearchHeroFilter[]
  /** Pre-grouped result days (already sorted newest → oldest). */
  groups: readonly PaperSearchViewDayGroup[]
  totalResults: number
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
  onPickSuggestion?: (suggestion: PaperSearchSuggestion) => void
  onRunRecent?: (recent: PaperSearchRecent) => void
  onAddDateFilter?: () => void
  onAddSourceFilter?: () => void
  onAddDomainFilter?: () => void
  onAddVisitCountFilter?: () => void
  /** Optional slot rendered below the hero before the results (e.g. for
      provider gating callouts in semantic mode). */
  belowHeroSlot?: ReactNode
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
  onPickSuggestion,
  onRunRecent,
  onAddDateFilter,
  onAddSourceFilter,
  onAddDomainFilter,
  onAddVisitCountFilter,
  belowHeroSlot,
  copy,
  className,
  testId,
}: PaperSearchViewProps) {
  const hasQuery = query.trim().length > 0
  const hasResults = totalResults > 0
  const oldestDate = groups[groups.length - 1]?.date ?? ''
  const newestDate = groups[0]?.date ?? ''

  return (
    <section
      data-testid={testId}
      className={cn('flex w-full flex-col', className)}
    >
      <PaperSearchHero
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
        copy={copy.hero}
      />

      {belowHeroSlot}

      {!hasQuery ? (
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
          <div className="text-ink-ghost mt-2 font-mono text-[10px]">
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
                />
              ))}
            </div>
          ))}
        </div>
      )}
    </section>
  )
}
