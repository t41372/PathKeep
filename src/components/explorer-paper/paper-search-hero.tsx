/**
 * Literary search hero — the centerpiece of the paper Search experience.
 *
 * Layout mirrors `pk-search.jsx` → SearchView's `.sv-hero`:
 *
 *   ┌───────────────────────────────────────────────┐
 *   │      What would you like to find again?       │  ← sv-prompt
 *   │                                               │
 *   │  ┌──────────────────────────────────────────┐ │  ← sv-input (28 px Newsreader)
 *   │  │  rust async runtime                      │ │
 *   │  └──────────────────────────────────────────┘ │
 *   │  MODE  [ keyword | regex | semantic ]  hint   │  ← sv-modes
 *   │  FILTERS  [Last 30 days×] [+ Date] [+ Source] │  ← sv-filters
 *   └───────────────────────────────────────────────┘
 *
 * ## Responsibilities
 * - Render the italic-serif prompt above the input, the 28 px Newsreader
 *   input with underline border, the keyword/regex/semantic mode toggle,
 *   the active mode's hint, and the filter chip row (active chips with ×
 *   handlers, "+ Date / Source / Domain" add chips).
 * - Stay controlled — caller owns query, mode, active filters; the hero
 *   surfaces change callbacks for each.
 *
 * ## Not responsible for
 * - Resolving filter values to URL state or backend queries.
 * - Rendering results (PaperSearchResults / PaperContactSheet do that).
 * - Auto-focus on first mount — caller controls via `autoFocus` prop.
 */

import { forwardRef, useCallback, type KeyboardEvent, type Ref } from 'react'
import { cn } from '@/lib/cn'
import {
  PaperAdvancedSearchHelp,
  type PaperAdvancedSearchHelpCopy,
} from './paper-advanced-search-help'

export type PaperSearchMode = 'keyword' | 'regex' | 'semantic'

export interface PaperSearchHeroCopy {
  prompt: string
  inputPlaceholder: string
  modesLabel: string
  filtersLabel: string
  modeKeyword: string
  modeRegex: string
  modeSemantic: string
  /** Mode-specific tail hints, e.g. "Match the exact words…" */
  modeHintKeyword: string
  modeHintRegex: string
  modeHintSemantic: string
  /**
   * Add-chip prefixes: "+ Date", "+ Source", "+ Domain", "+ Visit
   * count" plus the §3.3 A annotations chips "+ Tag" / "+ Note".
   * The annotations chips are wired end-to-end (panel parses active
   * `tag:` / `note:` operators back into removable chips); the
   * remaining chips stay inert until the broader filter-chip wiring
   * pass lands.
   */
  addFilterDate: string
  addFilterSource: string
  addFilterDomain: string
  addFilterVisitCount: string
  addFilterTag: string
  addFilterNote: string
  /** Aria label template for the chip remove button, e.g. "Remove {label}". */
  removeChipLabel: string
  /**
   * Copy bag for the advanced-syntax popover (`?` chip next to the mode
   * toggle). Documents the supported `site:` / `intitle:` / `OR` /
   * `filetype:` / date operators that the local keyword parser accepts —
   * the popover existed in v0.2 and was inadvertently dropped during the
   * paper redesign, see feedback-2026-05-25 §3.3 B.
   */
  advancedSyntaxHelp: PaperAdvancedSearchHelpCopy
}

export interface PaperSearchHeroFilter {
  id: string
  label: string
}

export interface PaperSearchHeroProps {
  query: string
  mode: PaperSearchMode
  /** Currently active filter chips. */
  activeFilters: readonly PaperSearchHeroFilter[]
  onQueryChange: (next: string) => void
  onModeChange: (next: PaperSearchMode) => void
  onRemoveFilter: (id: string) => void
  /** Add-chip click handlers. Omit any to render that chip inert. */
  onAddDateFilter?: () => void
  onAddSourceFilter?: () => void
  onAddDomainFilter?: () => void
  onAddVisitCountFilter?: () => void
  onAddTagFilter?: () => void
  onAddNoteFilter?: () => void
  /** Optional Enter handler; defaults to swallowing the event so the form doesn't submit. */
  onSubmit?: (query: string) => void
  /** True when this hero owns first-paint focus (Search route default). */
  autoFocus?: boolean
  copy: PaperSearchHeroCopy
  className?: string
  testId?: string
}

export const PaperSearchHero = forwardRef(function PaperSearchHero(
  {
    query,
    mode,
    activeFilters,
    onQueryChange,
    onModeChange,
    onRemoveFilter,
    onAddDateFilter,
    onAddSourceFilter,
    onAddDomainFilter,
    onAddVisitCountFilter,
    onAddTagFilter,
    onAddNoteFilter,
    onSubmit,
    autoFocus = false,
    copy,
    className,
    testId,
  }: PaperSearchHeroProps,
  ref: Ref<HTMLInputElement>,
) {
  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === 'Enter') {
        event.preventDefault()
        onSubmit?.(query)
      } else if (event.key === 'Escape' && query.length > 0) {
        event.preventDefault()
        onQueryChange('')
      }
    },
    [onSubmit, onQueryChange, query],
  )

  const modeHint =
    mode === 'keyword'
      ? copy.modeHintKeyword
      : mode === 'regex'
        ? copy.modeHintRegex
        : copy.modeHintSemantic

  return (
    <section
      data-testid={testId}
      className={cn('mx-auto mt-3 mb-8 max-w-[720px] pt-3', className)}
    >
      <div className="text-ink-faint mb-4 text-center font-serif text-[14px] italic">
        {copy.prompt}
      </div>

      <div className="border-ink-muted border-b px-0 pb-3 pt-1">
        <input
          ref={ref}
          data-testid="paper-search-input"
          type="text"
          value={query}
          autoFocus={autoFocus}
          placeholder={copy.inputPlaceholder}
          onChange={(event) => onQueryChange(event.target.value)}
          onKeyDown={handleKeyDown}
          className={cn(
            'text-ink w-full border-0 bg-transparent px-0 py-[6px]',
            'font-serif text-[28px] font-normal leading-[1.2] tracking-[-0.01em]',
            'placeholder:text-ink-faint placeholder:italic',
            'outline-none',
          )}
        />
      </div>

      <div className="mt-[14px] flex items-center gap-4">
        <span className="text-ink-faint font-mono text-[9.5px] uppercase tracking-[0.08em]">
          {copy.modesLabel}
        </span>
        <div
          role="tablist"
          className="border-border-default rounded-paper inline-flex overflow-hidden border"
        >
          {(
            [
              { value: 'keyword', label: copy.modeKeyword },
              { value: 'regex', label: copy.modeRegex },
              { value: 'semantic', label: copy.modeSemantic },
            ] as const
          ).map((item, index) => (
            <button
              key={item.value}
              type="button"
              role="tab"
              aria-selected={mode === item.value}
              onClick={() => onModeChange(item.value)}
              className={cn(
                'border-border-default font-mono text-[11px] tracking-[0.02em] px-[11px] py-[4px]',
                'transition-colors duration-150',
                index < 2 && 'border-r',
                mode === item.value
                  ? 'bg-accent text-paper'
                  : 'text-ink-muted bg-transparent hover:bg-hover hover:text-ink',
              )}
            >
              {item.label}
            </button>
          ))}
        </div>
        <span
          className="text-ink-faint ml-auto font-serif text-[12.5px] italic"
          data-testid="paper-search-mode-hint"
        >
          {modeHint}
        </span>
        <PaperAdvancedSearchHelp
          copy={copy.advancedSyntaxHelp}
          testId="paper-search-advanced-help"
        />
      </div>

      <div className="mt-[14px] flex flex-wrap items-center gap-[6px]">
        <span className="text-ink-faint mr-1 font-mono text-[9.5px] uppercase tracking-[0.08em]">
          {copy.filtersLabel}
        </span>
        {activeFilters.map((filter) => (
          <span
            key={filter.id}
            data-testid={`paper-search-active-filter-${filter.id}`}
            className={cn(
              'border-accent text-accent bg-accent-soft',
              'rounded-pill inline-flex items-center gap-[6px]',
              'border px-[9px] py-[3px] font-mono text-[10.5px]',
            )}
          >
            {filter.label}
            <button
              type="button"
              aria-label={copy.removeChipLabel.replace('{label}', filter.label)}
              onClick={() => onRemoveFilter(filter.id)}
              className="text-current opacity-60 hover:opacity-100"
            >
              ✕
            </button>
          </span>
        ))}
        <AddFilterChip
          label={copy.addFilterDate}
          onClick={onAddDateFilter}
          testId="paper-search-add-date"
        />
        <AddFilterChip
          label={copy.addFilterSource}
          onClick={onAddSourceFilter}
          testId="paper-search-add-source"
        />
        <AddFilterChip
          label={copy.addFilterDomain}
          onClick={onAddDomainFilter}
          testId="paper-search-add-domain"
        />
        <AddFilterChip
          label={copy.addFilterVisitCount}
          onClick={onAddVisitCountFilter}
          testId="paper-search-add-visit-count"
        />
        <AddFilterChip
          label={copy.addFilterTag}
          onClick={onAddTagFilter}
          testId="paper-search-add-tag"
        />
        <AddFilterChip
          label={copy.addFilterNote}
          onClick={onAddNoteFilter}
          testId="paper-search-add-note"
        />
      </div>
    </section>
  )
})

function AddFilterChip({
  label,
  onClick,
  testId,
}: {
  label: string
  onClick?: () => void
  testId?: string
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={!onClick}
      data-testid={testId}
      className={cn(
        'border-border-default text-ink-secondary bg-card-paper',
        'rounded-pill inline-flex items-center gap-[6px]',
        'border px-[9px] py-[3px] font-mono text-[10.5px]',
        'transition-colors duration-150',
        'hover:border-ink-muted enabled:cursor-pointer',
        'disabled:cursor-not-allowed disabled:opacity-40',
      )}
    >
      {label}
    </button>
  )
}
