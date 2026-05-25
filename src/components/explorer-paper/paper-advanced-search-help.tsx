/**
 * @file paper-advanced-search-help.tsx
 * @description Hover/focus popover that documents the advanced keyword
 * syntax (site:, intitle:, OR, "exact phrase", filetype:, after:/before:)
 * the local search query parser already accepts.
 * @module components/explorer-paper
 *
 * ## Responsibilities
 * - Render the `?` trigger button next to the paper search hero's mode
 *   toggle, and the hover/focus popover that lists supported operators
 *   with localised one-line explanations.
 * - Mirror the v0.2 `AdvancedSearchHelp` content so users who relied on
 *   the hover discoverability don't lose it under the paper redesign.
 *
 * ## Not responsible for
 * - Parsing or validating advanced search syntax.
 * - Owning regex-mode behavior or backend search execution.
 *
 * ## Why this helper exists
 * Without the hover hint, users have no way to discover `site:`, `-foo`,
 * `intitle:`, or `filetype:` operators — the parser supports them today,
 * but the v0.3 paper redesign dropped the explanatory affordance. See
 * feedback-2026-05-25 §3.3 B.
 */

import { useId, useState } from 'react'
import { cn } from '@/lib/cn'

const ADVANCED_SEARCH_EXAMPLES = [
  { code: 'site:github.com -pathkeep', labelKey: 'siteExclude' },
  { code: '"release notes"', labelKey: 'exactPhrase' },
  { code: 'manual OR youtube', labelKey: 'or' },
  { code: 'intitle:manual inurl:docs', labelKey: 'field' },
  {
    code: 'filetype:pdf after:2026-05-01 before:2026-05-07',
    labelKey: 'fileDate',
  },
] as const

export interface PaperAdvancedSearchHelpCopy {
  ariaLabel: string
  title: string
  intro: string
  siteExclude: string
  exactPhrase: string
  or: string
  field: string
  fileDate: string
  regexNote: string
}

export interface PaperAdvancedSearchHelpProps {
  copy: PaperAdvancedSearchHelpCopy
  className?: string
  testId?: string
}

/**
 * Renders a `?` trigger next to the search mode toggle. The popover
 * is visible on hover (mouse) and on focus (keyboard) — the open state
 * is mirrored to `data-open` so the styling stays declarative.
 */
export function PaperAdvancedSearchHelp({
  copy,
  className,
  testId,
}: PaperAdvancedSearchHelpProps) {
  const panelId = useId()
  const [open, setOpen] = useState(false)

  return (
    <span
      className={cn('relative inline-block', className)}
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      data-testid={testId ?? 'paper-advanced-search-help'}
    >
      <button
        type="button"
        aria-describedby={panelId}
        aria-label={copy.ariaLabel}
        aria-expanded={open}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        className={cn(
          'border-border-default text-ink-muted bg-card-paper hover:border-ink-muted',
          'inline-flex h-[18px] w-[18px] items-center justify-center rounded-full',
          'border font-mono text-[10px] leading-none transition-colors',
          'focus:outline-none focus-visible:ring-2 focus-visible:ring-accent',
        )}
      >
        ?
      </button>
      {open ? (
        <span
          id={panelId}
          role="tooltip"
          data-testid="paper-advanced-search-help-panel"
          className={cn(
            'absolute right-0 top-[26px] z-50',
            'border-border-default bg-paper text-ink rounded-paper',
            'w-[320px] border p-3 text-left shadow-paper',
          )}
        >
          <span className="block font-serif text-[13px] font-medium tracking-[-0.01em]">
            {copy.title}
          </span>
          <span className="text-ink-faint mt-1 block font-serif text-[11.5px] italic leading-[1.4]">
            {copy.intro}
          </span>
          <span className="mt-2 block">
            {ADVANCED_SEARCH_EXAMPLES.map((example) => (
              <span
                key={example.code}
                className="border-border-light mt-[6px] flex flex-col gap-[1px] border-t pt-[5px] first:border-t-0 first:pt-0"
              >
                <code className="text-ink font-mono text-[10.5px] leading-[1.4]">
                  {example.code}
                </code>
                <span className="text-ink-secondary font-serif text-[11px] leading-[1.35]">
                  {copy[example.labelKey]}
                </span>
              </span>
            ))}
          </span>
          <span className="text-ink-faint mt-3 block font-mono text-[9.5px] uppercase tracking-[0.06em]">
            {copy.regexNote}
          </span>
        </span>
      ) : null}
    </span>
  )
}
