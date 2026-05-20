/**
 * Folded "album" of consecutive same-domain visits inside a session.
 *
 * ## Responsibilities
 * - Render a single header row with a stacked-favicon block, the domain name,
 *   the page count, and the time range. Click toggles expanded/collapsed.
 * - Collapsed: show first N (default 4) titles with their times, plus a
 *   "+ N more" footer that expands the stack inline.
 * - Expanded: render the full list of entries as compact rows with title +
 *   URL + time, each clickable to surface the detail panel.
 * - Force-expand whenever any entry in the stack is the current target (e.g.
 *   arrived from Search "See in context"), so the user lands on a visible row.
 *
 * ## Not responsible for
 * - Determining which entries belong to the stack — caller does the grouping
 *   via `src/pages/explorer/paper/group-entries.ts`.
 * - The pulse highlight on a target entry; the Browse view applies it after
 *   the row mounts.
 *
 * ## Dependencies
 * - Paper tokens via `src/styles/tokens.css`.
 */

import { useState } from 'react'
import { cn } from '@/lib/cn'
import { sanitizeExplorerDisplayText } from '@/pages/explorer/helpers'

export interface PaperDomainStackEntry {
  id: number | string
  title?: string | null
  domain: string
  url?: string | null
  time: string
}

export interface PaperDomainStackProps {
  domain: string
  domainColor: string
  domainAbbr: string
  entries: PaperDomainStackEntry[]
  targetEntryId?: number | string | null
  collapsedPreviewCount?: number
  onSelectEntry?: (entry: PaperDomainStackEntry) => void
  expandLabel: string
  morePrefix: string
  pagesLabel: string
  className?: string
  testId?: string
}

export function PaperDomainStack({
  domain,
  domainColor,
  domainAbbr,
  entries,
  targetEntryId = null,
  collapsedPreviewCount = 4,
  onSelectEntry,
  expandLabel,
  morePrefix,
  pagesLabel,
  className,
  testId,
}: PaperDomainStackProps) {
  const containsTarget =
    targetEntryId !== null &&
    targetEntryId !== undefined &&
    entries.some((entry) => entry.id === targetEntryId)
  const [userExpanded, setUserExpanded] = useState(false)
  // Force-expanded whenever the active target sits inside this stack. Otherwise
  // honour the user's manual toggle. Deriving during render keeps us out of the
  // setState-in-effect cascade the react-hooks linter flags.
  const expanded = userExpanded || containsTarget

  const count = entries.length
  const first = entries[0]
  const last = entries[entries.length - 1]
  const previewEntries = entries.slice(0, collapsedPreviewCount)
  const overflowCount = Math.max(0, count - collapsedPreviewCount)

  return (
    <div
      data-testid={testId}
      data-expanded={expanded ? 'true' : undefined}
      className={cn(
        'border-border-light bg-card-paper rounded-paper overflow-hidden border',
        'transition-shadow duration-150',
        className,
      )}
    >
      <button
        type="button"
        aria-expanded={expanded}
        aria-label={expandLabel}
        onClick={() => setUserExpanded((value) => !value)}
        className="grid w-full grid-cols-[44px_1fr_auto] items-center gap-3 px-[14px] py-[10px] text-left"
      >
        <div className="relative h-9 w-11">
          <span
            className="absolute left-0 top-0 h-7 w-9 rounded-[3px]"
            style={{ background: domainColor, opacity: 0.3 }}
          />
          <span
            className="absolute left-1 top-1 h-7 w-9 rounded-[3px]"
            style={{ background: domainColor, opacity: 0.6 }}
          />
          <span
            className="absolute left-2 top-2 flex h-7 w-9 items-center justify-center rounded-[3px] font-mono text-[10px] font-semibold tracking-[0.06em]"
            style={{
              background: domainColor,
              color: 'rgba(255,255,255,0.8)',
            }}
          >
            {domainAbbr}
          </span>
        </div>
        <div className="min-w-0">
          <div className="text-ink font-mono text-[12px] font-medium truncate">
            {domain}
          </div>
          <div className="text-ink-faint mt-px font-sans text-[11.5px]">
            {count} {pagesLabel} · {last.time} — {first.time}
          </div>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-ink-muted font-mono text-[16px]">{count}</span>
          <span
            aria-hidden="true"
            className={cn(
              'text-ink-faint inline-block text-[12px] transition-transform duration-150',
              expanded ? 'rotate-90' : 'rotate-0',
            )}
          >
            ▸
          </span>
        </div>
      </button>

      {!expanded ? (
        <div className="flex flex-col gap-[2px] pb-[10px] pl-[70px] pr-[14px]">
          {previewEntries.map((entry) => (
            <button
              type="button"
              key={entry.id}
              data-entry-id={entry.id}
              onClick={(event) => {
                event.stopPropagation()
                if (onSelectEntry) onSelectEntry(entry)
              }}
              className="text-ink-muted truncate text-left font-serif text-[12px] leading-[1.3] hover:text-ink"
            >
              <span className="text-ink-faint mr-[6px] font-mono text-[10px]">
                {entry.time}
              </span>
              {sanitizeExplorerDisplayText(
                entry.title || entry.url || entry.domain,
              )}
            </button>
          ))}
          {overflowCount > 0 ? (
            <button
              type="button"
              onClick={(event) => {
                event.stopPropagation()
                setUserExpanded(true)
              }}
              className="text-ink-faint mt-px text-left font-mono text-[10.5px] hover:text-ink"
            >
              {morePrefix} {overflowCount}
            </button>
          ) : null}
        </div>
      ) : (
        <div className="border-border-light border-t">
          {entries.map((entry) => (
            <button
              type="button"
              key={entry.id}
              data-entry-id={entry.id}
              onClick={(event) => {
                event.stopPropagation()
                if (onSelectEntry) onSelectEntry(entry)
              }}
              className={cn(
                'border-border-light grid w-full grid-cols-[28px_1fr_auto] items-center gap-[10px]',
                'px-[14px] py-2 text-left transition-colors duration-100',
                'hover:bg-hover border-b last:border-b-0',
              )}
            >
              <span
                aria-hidden="true"
                className="flex h-7 w-7 items-center justify-center rounded-[3px] font-mono text-[9px] font-semibold"
                style={{
                  background: domainColor,
                  color: 'rgba(255,255,255,0.7)',
                }}
              >
                {domainAbbr}
              </span>
              <div className="min-w-0">
                <div className="text-ink truncate font-serif text-[13px]">
                  {sanitizeExplorerDisplayText(
                    entry.title || entry.url || entry.domain,
                  )}
                </div>
                <div className="text-ink-faint mt-px truncate font-mono text-[10px]">
                  {sanitizeExplorerDisplayText(entry.url || entry.domain)}
                </div>
              </div>
              <span className="text-ink-faint font-mono text-[10.5px]">
                {entry.time}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
