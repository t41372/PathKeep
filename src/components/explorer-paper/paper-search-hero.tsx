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

import {
  forwardRef,
  useCallback,
  useState,
  type KeyboardEvent,
  type Ref,
} from 'react'
import { cn } from '@/lib/cn'
import {
  PaperAdvancedSearchHelp,
  type PaperAdvancedSearchHelpCopy,
} from './paper-advanced-search-help'

export type PaperSearchMode = 'keyword' | 'regex' | 'smart'

export interface PaperSearchHeroCopy {
  prompt: string
  inputPlaceholder: string
  modesLabel: string
  filtersLabel: string
  modeKeyword: string
  modeRegex: string
  /** "Smart search" — the single AI mode (REACH-B; maps to ?mode=hybrid). */
  modeSmart: string
  /** Mode-specific tail hints, e.g. "Match the exact words…" */
  modeHintKeyword: string
  modeHintRegex: string
  modeHintSmart: string
  /**
   * Hint shown on the Smart tab when AI is off / unavailable, replacing
   * `modeHintSmart`. REACH-A's honest "available to turn on" vocabulary — the
   * tab stays visible but disabled so Smart search is discoverable, not hidden.
   */
  modeHintSmartUnavailable: string
  /** Aria suffix appended to the disabled Smart tab, e.g. "(unavailable)". */
  modeSmartUnavailableAria: string
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
  /** Primary submit-gate button label (idle). */
  searchButton: string
  /** Submit-gate button label while the submitted query is in flight. */
  searchingButton: string
  /** aria-label on the idle submit button. */
  searchButtonAria: string
  /** aria-label on the in-flight submit button. */
  searchingButtonAria: string
  /** Subtle hint shown below the input while it has focus. */
  submitHint: string
  /**
   * Stale-results banner template, e.g. "Showing {mode} results — press
   * Search to update". `{mode}` is filled from `staleModeNames`.
   */
  staleBanner: string
  /** Display names for the stale-banner `{mode}` slot (NOT the tab labels). */
  staleModeNames: {
    keyword: string
    regex: string
    smart: string
  }
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
  /**
   * True while the last-submitted query is running. Swaps the button label to
   * "Searching…" + a spinner, but never locks it — the user may re-submit.
   */
  isSearching?: boolean
  /**
   * Disables the Search button. The route computes this: empty draft, or the
   * draft + mode are identical to the last submission (no redundant query).
   */
  submitDisabled?: boolean
  /**
   * When set, the on-screen results reflect this (last-submitted) mode while the
   * live mode differs — renders the subtle stale-results banner under the input.
   * `null`/omitted hides it.
   */
  staleMode?: PaperSearchMode | null
  /** True when this hero owns first-paint focus (Search route default). */
  autoFocus?: boolean
  /**
   * Whether the Smart tab is selectable. When false (AI off / no embedding
   * provider / index empty per REACH-A's `optionalAiAvailability`) the tab still
   * renders — discoverable, REACH-A pattern — but is disabled and shows the
   * `modeHintSmartUnavailable` hint. Defaults to true so non-AI callers (and the
   * preview fixtures) keep the tab live. The route never lets Smart be the
   * *active* mode while unavailable, so the disabled tab is only ever reached
   * from another mode.
   */
  smartAvailable?: boolean
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
    isSearching = false,
    submitDisabled = false,
    staleMode = null,
    autoFocus = false,
    smartAvailable = true,
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
      } else if (event.key === 'Escape') {
        // Esc clears a non-empty draft (→ idle). When the draft is already
        // empty, Esc blurs the input so a second press releases focus rather
        // than being a dead no-op.
        event.preventDefault()
        if (query.length > 0) {
          onQueryChange('')
        } else {
          event.currentTarget.blur()
        }
      }
    },
    [onSubmit, onQueryChange, query],
  )

  // The submit hint is only meaningful while the input has focus. Local state
  // keeps the toggle inside the hero — it never reaches the route or touches the
  // result region, so there is no cross-tree re-render.
  const [inputFocused, setInputFocused] = useState(false)

  const staleBannerText =
    staleMode != null
      ? copy.staleBanner.replace('{mode}', copy.staleModeNames[staleMode])
      : null

  const modeHint =
    mode === 'keyword'
      ? copy.modeHintKeyword
      : mode === 'regex'
        ? copy.modeHintRegex
        : smartAvailable
          ? copy.modeHintSmart
          : copy.modeHintSmartUnavailable

  return (
    <section
      data-testid={testId}
      className={cn('mx-auto mt-3 mb-8 max-w-[720px] pt-3', className)}
    >
      <div className="text-ink-faint mb-4 text-center font-serif text-[14px] italic">
        {copy.prompt}
      </div>

      <div className="border-ink-muted flex items-end gap-3 border-b px-0 pb-3 pt-1">
        <input
          ref={ref}
          data-testid="paper-search-input"
          type="text"
          value={query}
          autoFocus={autoFocus}
          placeholder={copy.inputPlaceholder}
          onChange={(event) => onQueryChange(event.target.value)}
          onKeyDown={handleKeyDown}
          onFocus={() => setInputFocused(true)}
          onBlur={() => setInputFocused(false)}
          className={cn(
            'text-ink min-w-0 flex-1 border-0 bg-transparent px-0 py-[6px]',
            'font-serif text-[28px] font-normal leading-[1.2] tracking-[-0.01em]',
            'placeholder:text-ink-faint placeholder:italic',
            'outline-none',
          )}
        />
        <button
          type="button"
          data-testid="paper-search-submit"
          // Don't lock the button while searching — the user may refine and
          // re-submit. Only the route's redundant-query / empty-draft gate
          // disables it.
          disabled={submitDisabled}
          aria-label={
            isSearching ? copy.searchingButtonAria : copy.searchButtonAria
          }
          aria-busy={isSearching}
          onClick={() => onSubmit?.(query)}
          className={cn(
            'bg-accent text-paper rounded-paper shrink-0 self-center',
            // Fixed width so the label never reflows between
            // "Search" / "Searching…".
            'inline-flex w-[116px] items-center justify-center gap-[6px]',
            'px-[14px] py-[7px] font-mono text-[11px] tracking-[0.02em]',
            'transition-opacity duration-150',
            'enabled:hover:opacity-90 enabled:cursor-pointer',
            'disabled:cursor-not-allowed disabled:opacity-40',
          )}
        >
          {isSearching ? (
            <span
              data-testid="paper-search-submit-spinner"
              aria-hidden="true"
              className={cn(
                'border-paper/40 border-t-paper h-[11px] w-[11px]',
                'rounded-full border-[1.5px] motion-safe:animate-spin',
              )}
            />
          ) : null}
          <span>{isSearching ? copy.searchingButton : copy.searchButton}</span>
        </button>
      </div>

      {staleBannerText ? (
        <div
          data-testid="paper-search-stale-banner"
          role="status"
          className="text-ink-faint mt-2 font-serif text-[12.5px] italic"
        >
          {staleBannerText}
        </div>
      ) : (
        <div
          data-testid="paper-search-submit-hint"
          aria-hidden={!inputFocused}
          className={cn(
            'text-ink-faint mt-2 font-mono text-[9.5px] tracking-[0.04em]',
            // Only meaningful while the input is focused — fade without
            // stealing layout from the result region.
            'transition-opacity duration-150',
            inputFocused ? 'opacity-100' : 'opacity-0',
          )}
        >
          {copy.submitHint}
        </div>
      )}

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
              { value: 'keyword', label: copy.modeKeyword, disabled: false },
              { value: 'regex', label: copy.modeRegex, disabled: false },
              {
                value: 'smart',
                label: copy.modeSmart,
                disabled: !smartAvailable,
              },
            ] as const
          ).map((item, index) => (
            <button
              key={item.value}
              type="button"
              role="tab"
              aria-selected={mode === item.value}
              aria-label={
                item.disabled
                  ? `${item.label} ${copy.modeSmartUnavailableAria}`
                  : undefined
              }
              disabled={item.disabled}
              data-testid={`paper-search-mode-${item.value}`}
              onClick={() => onModeChange(item.value)}
              className={cn(
                'border-border-default font-mono text-[11px] tracking-[0.02em] px-[11px] py-[4px]',
                'transition-colors duration-150',
                index < 2 && 'border-r',
                item.disabled && 'cursor-not-allowed opacity-40',
                mode === item.value
                  ? 'bg-accent text-paper'
                  : 'text-ink-muted bg-transparent enabled:hover:bg-hover enabled:hover:text-ink',
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
