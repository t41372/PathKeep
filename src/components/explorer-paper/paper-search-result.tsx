/**
 * Single search-result row used in the paper Search list.
 *
 * Visually distinct from PaperContactFrame: results have a domain swatch on
 * the left, a Newsreader title with `<mark>` highlights of the query, the
 * mono URL beneath, an optional italic-serif snippet (for semantic mode), an
 * optional enrichment excerpt tagged with a "Page summary" source pill, and a
 * meta column on the right with time + transition type. A small
 * "See in context →" pill appears on hover that routes back into the
 * Browse contact sheet centred on the entry's day.
 *
 * ## Responsibilities
 * - Render the icon / title / url / snippet / enrichment / meta blocks per the
 *   design.
 * - Highlight matching tokens of `query` inside the title, snippet, and
 *   enrichment excerpt (only real overlaps highlight — honest on every path).
 * - Label the enrichment excerpt by its SOURCE (the page's summary), never with
 *   a match-claim caption, so the framing stays honest on a pure-semantic hit
 *   whose summary contains none of the query words.
 * - Surface the "see in context" jump as a separate handler.
 *
 * ## Not responsible for
 * - Resolving the snippet / enrichment excerpt (caller passes them ready).
 * - Deciding the enrichment source label (the adapters stamp it).
 * - Domain colour or abbreviation (caller passes them — same as
 *   PaperContactFrame).
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'
import { StarToggle } from '@/components/shell/star-toggle'
import { sanitizeExplorerDisplayText } from '@/pages/explorer/helpers'

export interface PaperSearchResultEntry {
  id: number | string
  title: string
  url: string
  domain: string
  time: string
  /** Free-form transition kind label ("link", "typed", …). */
  transitionType?: string
  /** Optional snippet to render below the URL (semantic mode). */
  snippet?: string
  /**
   * Smart-search match reason — a short backend-supplied caption explaining why
   * a row matched ("Semantic match", "Lexical + semantic match", "…(Starred)").
   * Rendered as a mono caption on relevance-ranked rows. There is NO snippet on
   * Smart results (`AiSearchResultItem` has no snippet field); this caption is
   * the honest stand-in. Absent on keyword/regex rows.
   */
  matchReason?: string
  /**
   * Relevance band derived from the Smart result's `score` via `scoreBand(...)`.
   * Rendered as a small status pill in the meta column on relevance-ranked rows
   * so the user can read confidence at a glance. Absent on keyword/regex rows.
   */
  relevanceBand?: {
    label: string
    tone: 'success' | 'warning' | 'blocked' | 'info'
  }
  /**
   * YYYY-MM-DD local day key for the row's visit. Keyword rows get their day
   * from the enclosing day group; relevance (Smart) rows are ungrouped, so the
   * adapter stamps this so "see in context" can still jump to the right Browse
   * day. Optional — absent on rows without a parseable visit time.
   */
  dayKey?: string
  /**
   * Optional enrichment excerpt: a capped piece of the page's stored enrichment
   * summary (W-ENRICH-1, 06 §6). Surfaced on BOTH lexical and semantic/hybrid
   * rows when the page has enrichment text. It is the page's summary — NOT the
   * matched span — so it is labelled by SOURCE (`enrichmentSourceLabel`), never
   * with a match-claim caption: on a pure-semantic hit the summary may contain
   * none of the query words, so "matched in …" would be dishonest. Query tokens
   * that do appear get `<mark>` highlights (honest on both paths).
   */
  enrichmentExcerpt?: string
  /**
   * Source label for the enrichment excerpt ("Page summary"). Rendered as a pill
   * above the excerpt so the user knows the snippet is the page's enrichment
   * summary rather than the title/URL or a matched span. Set by the adapters for
   * every enriched row; when absent the pill is suppressed.
   */
  enrichmentSourceLabel?: string
}

export interface PaperSearchResultProps {
  entry: PaperSearchResultEntry
  domainColor: string
  domainAbbr: string
  /** Active query — substrings are wrapped in <mark> inside the title. */
  query?: string
  onSelect?: (entry: PaperSearchResultEntry) => void
  onSeeInContext?: (entry: PaperSearchResultEntry) => void
  seeInContextLabel?: string
  /**
   * Per-row "Ask assistant" affordance for Smart (relevance-ranked) rows. When
   * supplied AND the entry carries a `matchReason` (i.e. it is a Smart result),
   * a small pill renders below the row that hands the entry to the assistant.
   * Keyword/regex rows never pass this, so the affordance stays Smart-only.
   */
  onAskAssistant?: (entry: PaperSearchResultEntry) => void
  askAssistantLabel?: string
  /** Star affordance for this result. Omit to hide it. */
  star?: {
    starred: boolean
    onToggle: () => void
    starLabel: string
    unstarLabel: string
  }
  className?: string
  testId?: string
}

export function PaperSearchResult({
  entry,
  domainColor,
  domainAbbr,
  query,
  onSelect,
  onSeeInContext,
  seeInContextLabel,
  onAskAssistant,
  askAssistantLabel,
  star,
  className,
  testId,
}: PaperSearchResultProps) {
  return (
    <div
      data-entry-id={entry.id}
      data-testid={testId}
      onClick={() => onSelect?.(entry)}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault()
          onSelect?.(entry)
        }
      }}
      role={onSelect ? 'button' : undefined}
      tabIndex={onSelect ? 0 : undefined}
      className={cn(
        'group grid grid-cols-[32px_1fr_auto] items-start gap-3',
        'rounded-paper -mx-3 cursor-pointer px-3 py-[10px]',
        'hover:bg-hover transition-colors duration-150',
        className,
      )}
    >
      <span
        aria-hidden="true"
        className="mt-[2px] flex h-[30px] w-[30px] shrink-0 items-center justify-center rounded-[3px] font-mono text-[10px] font-semibold"
        style={{
          background: domainColor,
          color: 'rgba(255,255,255,0.7)',
        }}
      >
        {domainAbbr}
      </span>

      <div className="min-w-0">
        <div className="text-ink m-0 font-serif text-[14.5px] leading-[1.3] tracking-[-0.005em]">
          {highlightQuery(
            sanitizeExplorerDisplayText(
              entry.title || entry.url || entry.domain,
            ),
            query,
          )}
        </div>
        <div className="text-ink-faint truncate font-mono text-[10.5px]">
          {entry.domain} · {sanitizeExplorerDisplayText(entry.url, 96)}
        </div>
        {entry.matchReason ? (
          <div
            data-testid={
              testId
                ? `${testId}-match-reason`
                : 'paper-search-result-match-reason'
            }
            className="text-ink-muted mt-1 font-mono text-[10px] tracking-[0.02em]"
          >
            {entry.matchReason}
          </div>
        ) : null}
        {entry.snippet ? (
          <div className="text-ink-muted mt-1 line-clamp-2 font-serif text-[12.5px] italic leading-[1.4]">
            “…{entry.snippet}…”
          </div>
        ) : null}
        {entry.enrichmentExcerpt ? (
          <div
            data-testid={
              testId ? `${testId}-enrichment` : 'paper-search-result-enrichment'
            }
            className="mt-1"
          >
            {/* Label the excerpt by its SOURCE (the page's enrichment summary),
                never with a match-claim — the summary is the same page text on
                lexical AND semantic rows, so a "matched in …" caption would lie
                on a pure-semantic hit. The pill carries the honest framing; the
                row's `matchReason` separately explains WHY it matched. */}
            {entry.enrichmentSourceLabel ? (
              <div className="text-ink-faint flex items-center gap-1.5 font-mono text-[9.5px] uppercase tracking-[0.06em]">
                <span className="border-border-light text-ink-secondary rounded-pill inline-flex items-center border px-[6px] py-[1px] tracking-[0.04em]">
                  {entry.enrichmentSourceLabel}
                </span>
              </div>
            ) : null}
            <div className="text-ink-muted mt-1 line-clamp-2 font-serif text-[12.5px] italic leading-[1.4]">
              “…{highlightQuery(entry.enrichmentExcerpt, query)}…”
            </div>
          </div>
        ) : null}
        {(onSeeInContext && seeInContextLabel) ||
        (onAskAssistant && askAssistantLabel && entry.matchReason) ? (
          <div className="mt-2 flex flex-wrap items-center gap-[6px]">
            {onSeeInContext && seeInContextLabel ? (
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation()
                  onSeeInContext(entry)
                }}
                data-testid="paper-search-result-see-in-context"
                className={cn(
                  'inline-flex items-center gap-[5px] rounded-pill',
                  'border-border-light text-ink-muted border bg-transparent px-[8px] py-[3px]',
                  'font-mono text-[10px] tracking-[0.04em] opacity-50',
                  'group-hover:opacity-100 transition-opacity duration-150',
                  'hover:border-accent hover:text-accent-text hover:bg-accent-soft',
                )}
              >
                {seeInContextLabel} →
              </button>
            ) : null}
            {onAskAssistant && askAssistantLabel && entry.matchReason ? (
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation()
                  onAskAssistant(entry)
                }}
                data-testid="paper-search-result-ask-assistant"
                className={cn(
                  'inline-flex items-center gap-[5px] rounded-pill',
                  'border-border-light text-ink-muted border bg-transparent px-[8px] py-[3px]',
                  'font-mono text-[10px] tracking-[0.04em] opacity-50',
                  'group-hover:opacity-100 transition-opacity duration-150',
                  'hover:border-accent hover:text-accent-text hover:bg-accent-soft',
                )}
              >
                {askAssistantLabel} →
              </button>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className="flex shrink-0 items-start gap-2">
        <div className="text-ink-faint flex flex-col items-end gap-[2px] font-mono text-[10px]">
          {entry.relevanceBand ? (
            <span
              data-testid={
                testId ? `${testId}-band` : 'paper-search-result-band'
              }
              data-tone={entry.relevanceBand.tone}
              className={cn(
                'rounded-pill border px-[6px] py-[1px] text-[9px] uppercase tracking-[0.06em]',
                bandToneClass(entry.relevanceBand.tone),
              )}
            >
              {entry.relevanceBand.label}
            </span>
          ) : null}
          <span>{entry.time}</span>
          {entry.transitionType ? (
            <span className="text-ink-faint uppercase tracking-[0.06em] text-[9px]">
              {entry.transitionType}
            </span>
          ) : null}
        </div>
        {star ? (
          <StarToggle
            starred={star.starred}
            onToggle={star.onToggle}
            starLabel={star.starLabel}
            unstarLabel={star.unstarLabel}
            testId={testId ? `${testId}-star` : undefined}
          />
        ) : null}
      </div>
    </div>
  )
}

/**
 * Map a relevance-band tone onto the paper status-pill palette as an honest
 * relevance LADDER: accent for the strongest match (`success`), a neutral ink
 * box for a mid "Relevant" match (`info`), and the faintest treatment for a weak
 * or unscored match (`blocked`). Kept as a tiny pure lookup so the band pill
 * stays consistent with the paper surface without pulling in a heavier status
 * component. `warning` is intentionally NOT produced by `scoreBand` (a caution
 * tone would misread on a positive match) but is mapped to the neutral mid step
 * so the pill never renders unstyled if a caller passes it.
 */
function bandToneClass(
  tone: 'success' | 'warning' | 'blocked' | 'info',
): string {
  switch (tone) {
    case 'success':
      return 'border-accent text-accent bg-accent-soft'
    case 'info':
    case 'warning':
      return 'border-border-default text-ink-secondary bg-card-paper'
    default:
      return 'border-border-light text-ink-faint bg-transparent'
  }
}

/**
 * Wrap any whitespace-separated tokens from `query` inside the title with
 * `<mark>` so the design's accent-tinted highlight kicks in. Returns the
 * input verbatim when the query is empty, every token escapes to nothing,
 * or the regex compile fails — render must never throw on user input.
 */
function highlightQuery(text: string, query: string | undefined): ReactNode {
  const trimmed = query?.trim()
  if (!trimmed) return text
  const tokens = trimmed
    .split(/\s+/)
    .map((piece) => piece.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
    .filter(Boolean)
  // `trim()` strips outer whitespace and `split(/\s+/)` never yields empty
  // strings for non-empty input, so this branch is unreachable from real
  // input — keep it as a safety net if the escape pipeline ever changes.
  // Stryker disable next-line ConditionalExpression: defensive guard.
  /* v8 ignore next -- defensive: unreachable, see comment above. */
  if (tokens.length === 0) return text
  let pattern: RegExp
  try {
    pattern = new RegExp(`(${tokens.join('|')})`, 'gi')
  } catch {
    return text
  }
  const parts = text.split(pattern)
  return parts.map((part, index) =>
    pattern.test(part) ? (
      <mark
        key={index}
        className="bg-accent-soft text-accent rounded-[1px] px-[1px]"
      >
        {part}
      </mark>
    ) : (
      <span key={index}>{part}</span>
    ),
  )
}
