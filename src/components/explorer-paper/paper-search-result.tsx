/**
 * Single search-result row used in the paper Search list.
 *
 * Visually distinct from PaperContactFrame: results have a domain swatch on
 * the left, a Newsreader title with `<mark>` highlights of the query, the
 * mono URL beneath, an optional italic-serif snippet (for semantic mode),
 * and a meta column on the right with time + transition type. A small
 * "See in context →" pill appears on hover that routes back into the
 * Browse contact sheet centred on the entry's day.
 *
 * ## Responsibilities
 * - Render the icon / title / url / snippet / meta blocks per the design.
 * - Highlight matching tokens of `query` inside the title and snippet.
 * - Surface the "see in context" jump as a separate handler.
 *
 * ## Not responsible for
 * - Resolving the snippet (semantic mode only; caller passes it ready).
 * - Domain colour or abbreviation (caller passes them — same as
 *   PaperContactFrame).
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'
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
        {entry.snippet ? (
          <div className="text-ink-muted mt-1 line-clamp-2 font-serif text-[12.5px] italic leading-[1.4]">
            “…{entry.snippet}…”
          </div>
        ) : null}
        {onSeeInContext && seeInContextLabel ? (
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation()
              onSeeInContext(entry)
            }}
            data-testid="paper-search-result-see-in-context"
            className={cn(
              'mt-2 inline-flex items-center gap-[5px] rounded-pill',
              'border-border-light text-ink-muted border bg-transparent px-[8px] py-[3px]',
              'font-mono text-[10px] tracking-[0.04em] opacity-50',
              'group-hover:opacity-100 transition-opacity duration-150',
              'hover:border-accent hover:text-accent-text hover:bg-accent-soft',
            )}
          >
            {seeInContextLabel} →
          </button>
        ) : null}
      </div>

      <div className="text-ink-faint flex shrink-0 flex-col items-end gap-[2px] font-mono text-[10px]">
        <span>{entry.time}</span>
        {entry.transitionType ? (
          <span className="text-ink-ghost uppercase tracking-[0.06em] text-[9px]">
            {entry.transitionType}
          </span>
        ) : null}
      </div>
    </div>
  )
}

/**
 * Wrap any whitespace-separated tokens from `query` inside the title with
 * `<mark>` so the design's accent-tinted highlight kicks in. Returns the
 * input verbatim when the query is empty or the regex compile fails.
 */
function highlightQuery(text: string, query: string | undefined): ReactNode {
  const trimmed = query?.trim()
  if (!trimmed) return text
  const tokens = trimmed
    .split(/\s+/)
    .map((piece) => piece.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'))
    .filter(Boolean)
  const pattern = new RegExp(`(${tokens.join('|')})`, 'gi')
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
