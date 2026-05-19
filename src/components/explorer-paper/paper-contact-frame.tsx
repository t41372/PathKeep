/**
 * Single contact-sheet "frame" — one card representing one history entry in
 * card mode (the default Browse layout).
 *
 * ## Responsibilities
 * - Render the 16:10 image area filled with the domain colour, the favicon (if
 *   provided), and the domain abbreviation as a fallback overlay.
 * - Render the caption block: serif title (2-line clamp), mono domain, mono time.
 * - Render the optional filmstrip frame number (top-right) and transition-type
 *   token (bottom-left).
 * - Apply a selected/hover treatment that matches `.cs-frame--selected` and
 *   `.cs-frame:hover` — accent border + soft shadow, not a fill change.
 *
 * ## Not responsible for
 * - Domain colour resolution — caller passes `domainColor` (use
 *   `src/pages/explorer/paper/domain-color.ts` to derive it deterministically).
 * - Click handling logic; consumes the standard `onClick(entry)` contract.
 *
 * ## Dependencies
 * - Paper tokens via `src/styles/tokens.css`.
 */

import { type ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface PaperContactFrameEntry {
  id: number | string
  title?: string | null
  domain: string
  url?: string | null
  time: string
  transitionType?: string | null
  faviconDataUrl?: string | null
}

export interface PaperContactFrameProps {
  entry: PaperContactFrameEntry
  domainColor: string
  domainAbbr: string
  index?: number
  selected?: boolean
  onClick?: (entry: PaperContactFrameEntry) => void
  className?: string
  testId?: string
  children?: ReactNode
}

export function PaperContactFrame({
  entry,
  domainColor,
  domainAbbr,
  index,
  selected = false,
  onClick,
  className,
  testId,
  children,
}: PaperContactFrameProps) {
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
        'group relative w-full overflow-hidden text-left',
        'rounded-paper border bg-card-paper',
        'shadow-frame hover:shadow-frame-hover transition-shadow duration-150',
        selected
          ? 'border-accent shadow-[0_0_0_1px_var(--accent)]'
          : 'border-border-light hover:border-border-default',
        className,
      )}
    >
      <div
        className="relative flex aspect-[16/10] items-center justify-center overflow-hidden"
        style={{ background: domainColor }}
      >
        {entry.faviconDataUrl ? (
          <img
            src={entry.faviconDataUrl}
            alt=""
            aria-hidden="true"
            className="h-[40%] w-auto opacity-90"
          />
        ) : (
          <span
            className="font-mono text-[15px] font-medium uppercase tracking-[0.08em]"
            style={{ color: 'rgba(255,255,255,0.65)' }}
          >
            {domainAbbr}
          </span>
        )}
        <span
          aria-hidden="true"
          className="pointer-events-none absolute inset-[3px] rounded-[1px] border"
          style={{ borderColor: 'rgba(255,255,255,0.12)' }}
        />
        {typeof index === 'number' ? (
          <span
            className="absolute right-[7px] top-[5px] z-[1] font-mono text-[9px] tracking-[0.06em]"
            style={{ color: 'rgba(255,255,255,0.4)' }}
          >
            {String(index + 1).padStart(2, '0')}
          </span>
        ) : null}
        {entry.transitionType ? (
          <span
            className="absolute bottom-[5px] left-[7px] z-[1] font-mono text-[8px] uppercase tracking-[0.08em]"
            style={{ color: 'rgba(255,255,255,0.35)' }}
          >
            {entry.transitionType}
          </span>
        ) : null}
      </div>
      <div className="px-[10px] pb-[10px] pt-[9px]">
        <div className="text-ink line-clamp-2 font-serif text-[12.5px] leading-[1.35]">
          {entry.title || entry.url || entry.domain}
        </div>
        <div className="mt-[5px] flex items-center justify-between">
          <span className="text-ink-faint max-w-[65%] truncate font-mono text-[10px]">
            {entry.domain}
          </span>
          <span className="text-ink-faint font-mono text-[10px]">
            {entry.time}
          </span>
        </div>
      </div>
      {children}
    </button>
  )
}
