/**
 * Dense list row — the alternate "list" rendering of a history entry inside
 * the Browse contact sheet.
 *
 * ## Responsibilities
 * - Render a 3-column row: domain swatch (24 px) · title + domain inline · time.
 * - Provide hover affordance and keyboard activation.
 *
 * ## Not responsible for
 * - Domain colour / abbreviation resolution (caller supplies them).
 *
 * ## Dependencies
 * - Paper tokens via `src/styles/tokens.css`.
 */

import { cn } from '@/lib/cn'

export interface PaperListRowEntry {
  id: number | string
  title?: string | null
  domain: string
  url?: string | null
  time: string
  /**
   * Optional cached favicon, already hydrated to a data URL. When present
   * the row replaces the domain swatch with a real icon; otherwise the
   * coloured abbreviation block remains the fallback.
   */
  faviconDataUrl?: string | null
}

export interface PaperListRowProps {
  entry: PaperListRowEntry
  domainColor: string
  domainAbbr: string
  selected?: boolean
  onClick?: (entry: PaperListRowEntry) => void
  className?: string
  testId?: string
}

export function PaperListRow({
  entry,
  domainColor,
  domainAbbr,
  selected = false,
  onClick,
  className,
  testId,
}: PaperListRowProps) {
  const handleClick = () => {
    if (onClick) onClick(entry)
  }

  return (
    <button
      type="button"
      onClick={handleClick}
      data-entry-id={entry.id}
      data-selected={selected ? 'true' : undefined}
      data-testid={testId}
      className={cn(
        'border-border-light grid w-full grid-cols-[26px_1fr_auto] items-center gap-[10px] border-b px-1 py-[7px] text-left',
        'hover:bg-hover transition-colors duration-100',
        selected && 'bg-accent-soft',
        className,
      )}
    >
      {entry.faviconDataUrl ? (
        <img
          src={entry.faviconDataUrl}
          alt=""
          aria-hidden="true"
          data-testid={testId ? `${testId}-favicon` : undefined}
          className="border-border-light h-4 w-4 self-center justify-self-center rounded-[3px] border bg-page object-contain"
        />
      ) : (
        <span
          aria-hidden="true"
          data-testid={testId ? `${testId}-swatch` : undefined}
          className="flex h-6 w-6 items-center justify-center rounded-[6px] font-mono text-[8px] font-semibold"
          style={{
            background: domainColor,
            color: 'rgba(255,255,255,0.7)',
          }}
        >
          {domainAbbr}
        </span>
      )}
      <span className="flex min-w-0 items-baseline gap-2">
        <span className="text-ink flex-1 truncate font-sans text-[12.5px]">
          {entry.title || entry.url || entry.domain}
        </span>
        <span className="text-ink-faint shrink-0 font-mono text-[10px]">
          {entry.domain}
        </span>
      </span>
      <span className="text-ink-faint shrink-0 font-mono text-[10px]">
        {entry.time}
      </span>
    </button>
  )
}
