/**
 * Empty state for the paper Search route — the "no query yet" surface.
 *
 * Surfaces two affordances from the design:
 * 1. SAVED_PROMPTS — a 2-column grid of suggestion cards with a mono cue,
 *    a serif example sentence, and a mono hint (results-count or
 *    "Date filter").
 * 2. RECENT_SEARCHES — a thin list of past queries with their mode + count
 *    + when, clickable to re-run.
 *
 * Both lists are optional; either can render alone.
 */

import { cn } from '@/lib/cn'

export interface PaperSearchSuggestion {
  id: string
  /** Mono uppercase cue, e.g. "Just ask" / "By time" / "By domain". */
  cue: string
  /** Serif example sentence the user can click to seed the query. */
  text: string
  /** Mono hint shown beneath the example. */
  hint?: string
}

export interface PaperSearchRecent {
  id: string
  q: string
  mode: 'keyword' | 'regex' | 'semantic'
  /** Number of results last time the query ran. */
  count: number
  /** Pretty-formatted "when" string. */
  when: string
}

export interface PaperSearchEmptyCopy {
  tryAskingHeading: string
  recentHeading: string
  /** Template, e.g. "{mode} · {count} results · {when}". */
  recentMeta: string
  /** Quiet footer line. */
  footer: string
}

export interface PaperSearchEmptyProps {
  suggestions?: readonly PaperSearchSuggestion[]
  recent?: readonly PaperSearchRecent[]
  onPickSuggestion?: (suggestion: PaperSearchSuggestion) => void
  onRunRecent?: (recent: PaperSearchRecent) => void
  copy: PaperSearchEmptyCopy
  className?: string
  testId?: string
}

export function PaperSearchEmpty({
  suggestions = [],
  recent = [],
  onPickSuggestion,
  onRunRecent,
  copy,
  className,
  testId,
}: PaperSearchEmptyProps) {
  return (
    <section
      data-testid={testId}
      className={cn('mx-auto max-w-[720px]', className)}
    >
      {suggestions.length > 0 ? (
        <>
          <SectionHeading>{copy.tryAskingHeading}</SectionHeading>
          <div className="grid grid-cols-1 gap-[10px] sm:grid-cols-2">
            {suggestions.map((suggestion) => (
              <button
                type="button"
                key={suggestion.id}
                onClick={() => onPickSuggestion?.(suggestion)}
                disabled={!onPickSuggestion}
                data-testid={`paper-search-suggestion-${suggestion.id}`}
                className={cn(
                  'rounded-paper border-border-light bg-card-paper border px-[14px] py-[12px]',
                  'text-left transition-all duration-150',
                  'enabled:hover:border-ink-muted enabled:hover:-translate-y-[1px] enabled:hover:shadow-frame',
                  'disabled:cursor-not-allowed disabled:opacity-60',
                )}
              >
                <div className="text-ink-faint font-mono text-[10px] uppercase tracking-[0.06em]">
                  {suggestion.cue}
                </div>
                <div className="text-ink mt-1 font-serif text-[14.5px] leading-[1.3] tracking-[-0.005em]">
                  {suggestion.text}
                </div>
                {suggestion.hint ? (
                  <div className="text-ink-faint mt-[6px] font-mono text-[10.5px]">
                    {suggestion.hint}
                  </div>
                ) : null}
              </button>
            ))}
          </div>
        </>
      ) : null}

      {recent.length > 0 ? (
        <>
          <SectionHeading className="mt-6">{copy.recentHeading}</SectionHeading>
          <div className="grid gap-1">
            {recent.map((entry) => (
              <button
                type="button"
                key={entry.id}
                onClick={() => onRunRecent?.(entry)}
                disabled={!onRunRecent}
                data-testid={`paper-search-recent-${entry.id}`}
                className={cn(
                  'border-border-light flex items-center justify-between border-b py-2 last:border-b-0',
                  'text-left text-ink-secondary font-serif text-[14px]',
                  'enabled:hover:text-ink transition-colors duration-150',
                  'disabled:cursor-not-allowed disabled:opacity-60',
                )}
              >
                <span>{entry.q}</span>
                <span className="text-ink-faint font-mono text-[10.5px]">
                  {copy.recentMeta
                    .replace('{mode}', entry.mode)
                    .replace('{count}', String(entry.count))
                    .replace('{when}', entry.when)}
                </span>
              </button>
            ))}
          </div>
        </>
      ) : null}

      <div className="border-border-light mt-10 border-t py-5 text-center font-serif text-[13px] italic text-ink-faint">
        {copy.footer}
      </div>
    </section>
  )
}

function SectionHeading({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <h3
      className={cn(
        'text-ink-faint mb-[10px] mt-6 font-mono text-[10px] uppercase tracking-[0.08em]',
        className,
      )}
    >
      {children}
    </h3>
  )
}
