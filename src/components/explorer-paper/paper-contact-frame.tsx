/**
 * Single contact-sheet "frame" — one card representing one history entry in
 * card mode (the default Browse layout).
 *
 * ## Responsibilities
 * - Render the 1.91:1 image area (Open Graph standard so social cards
 *   are not cropped) filled with the domain colour, the favicon (if
 *   provided), and the domain abbreviation as a fallback overlay.
 * - Render the caption block: serif title (2-line clamp), mono domain, mono time.
 * - Render the optional filmstrip frame number (top-right) and transition-type
 *   token (bottom-left).
 * - Apply a selected/hover treatment that matches `.cs-frame--selected` and
 *   `.cs-frame:hover` — accent border + soft shadow, not a fill change.
 * - When neither favicon nor og:image is available, render a first-class
 *   fallback panel (domain word-mark, time, title) instead of a bare
 *   abbreviation glyph — see `FallbackPanel` below.
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
import { StarToggle } from '@/components/shell/star-toggle'
import { sanitizeExplorerDisplayText } from '@/pages/explorer/helpers'

export interface PaperContactFrameEntry {
  id: number | string
  title?: string | null
  domain: string
  url?: string | null
  time: string
  transitionType?: string | null
  faviconDataUrl?: string | null
  /**
   * Optional og:image preview, hydrated lazily by the card-mode hook.
   * When present it replaces the domain-colour background with a
   * full-bleed social card; the index + transition tokens stay legible
   * via a top/bottom scrim so the card still scans as a paper frame.
   */
  ogImageDataUrl?: string | null
}

/** Star affordance for a contact card. Overlaid on the card's top-left. */
export interface PaperContactFrameStar {
  starred: boolean
  onToggle: () => void
  starLabel: string
  unstarLabel: string
  /** State words for the star's polite live region ("Starred"/"Unstarred"). */
  statusLabel?: { starred: string; unstarred: string }
}

export interface PaperContactFrameProps {
  entry: PaperContactFrameEntry
  domainColor: string
  domainAbbr: string
  index?: number
  selected?: boolean
  onClick?: (entry: PaperContactFrameEntry) => void
  star?: PaperContactFrameStar
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
  star,
  className,
  testId,
  children,
}: PaperContactFrameProps) {
  const handleClick = () => {
    if (onClick) onClick(entry)
  }

  return (
    <div className={cn('group relative', className)}>
      <button
        type="button"
        onClick={handleClick}
        data-entry-id={entry.id}
        data-selected={selected ? 'true' : undefined}
        data-testid={testId}
        className={cn(
          'relative w-full overflow-hidden text-left',
          'rounded-paper border bg-card-paper',
          'shadow-frame hover:shadow-frame-hover transition-shadow duration-150',
          selected
            ? 'border-accent shadow-[0_0_0_1px_var(--accent)]'
            : 'border-border-light hover:border-border-default',
        )}
      >
        <div
          className="relative flex aspect-[1.91/1] items-center justify-center overflow-hidden"
          style={{
            background: entry.ogImageDataUrl ? '#000' : domainColor,
          }}
        >
          {entry.ogImageDataUrl ? (
            <>
              <img
                src={entry.ogImageDataUrl}
                alt=""
                aria-hidden="true"
                data-testid={testId ? `${testId}-og-image` : undefined}
                className="absolute inset-0 h-full w-full object-cover"
              />
              {/* Scrim so the index/transition tokens stay readable above
                the og:image bytes. Top + bottom only — center stays clean. */}
              <span
                aria-hidden="true"
                className="pointer-events-none absolute inset-0"
                style={{
                  background:
                    'linear-gradient(to bottom, rgba(0,0,0,0.22) 0%, rgba(0,0,0,0) 18%, rgba(0,0,0,0) 82%, rgba(0,0,0,0.28) 100%)',
                }}
              />
            </>
          ) : entry.faviconDataUrl ? (
            <img
              src={entry.faviconDataUrl}
              alt=""
              aria-hidden="true"
              data-testid={testId ? `${testId}-favicon` : undefined}
              className="h-[44%] w-auto opacity-95"
            />
          ) : (
            <FallbackPanel
              entry={entry}
              domainAbbr={domainAbbr}
              testId={testId}
            />
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
            {sanitizeExplorerDisplayText(
              entry.title || entry.url || entry.domain,
            )}
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
      {star ? (
        <StarToggle
          starred={star.starred}
          onToggle={star.onToggle}
          starLabel={star.starLabel}
          unstarLabel={star.unstarLabel}
          statusLabel={star.statusLabel}
          alwaysVisible={selected}
          testId={testId ? `${testId}-star` : undefined}
          className={cn(
            'absolute left-[6px] top-[6px] z-[2]',
            // Tint the icon for contrast over the (possibly dark) image area.
            'bg-[color-mix(in_srgb,var(--bg-paper)_72%,transparent)] backdrop-blur-[2px]',
          )}
        />
      ) : null}
    </div>
  )
}

/**
 * First-class fallback panel for cards with neither favicon nor og:image.
 *
 * Why: the old fallback rendered just the domain abbreviation glyph
 * centred in the hero area — the card scanned as empty even though the
 * caption row below carried title / domain / time. With ~half the
 * archive falling into this state on Chrome takeout imports (no favicons
 * captured) and on long-tail / personal sites (no og:image authored),
 * the image area has to earn its space without a hero image.
 *
 * We deliberately do not duplicate the title here — the caption row
 * already does that. Instead the image area becomes a serif "title
 * page": full-width italic domain word-mark (book-spine style) plus a
 * mono ledger row with the abbreviation token and visit time. A glance
 * answers "which site, which moment"; the caption answers "which page".
 */
function FallbackPanel({
  entry,
  domainAbbr,
  testId,
}: {
  entry: PaperContactFrameEntry
  domainAbbr: string
  testId?: string
}) {
  const cleanDomain = entry.domain.replace(/^www\./i, '')
  return (
    <div
      data-testid={testId ? `${testId}-fallback` : 'paper-frame-fallback'}
      className="absolute inset-0 flex flex-col justify-between px-[12px] pt-[9px] pb-[10px]"
    >
      <div className="flex items-baseline justify-between gap-2">
        <span
          aria-hidden="true"
          className="font-mono text-[9px] uppercase leading-none tracking-[0.1em]"
          style={{ color: 'rgba(255,255,255,0.45)' }}
        >
          {domainAbbr}
        </span>
        <span
          className="font-mono text-[9.5px] leading-none tabular-nums"
          style={{ color: 'rgba(255,255,255,0.7)' }}
        >
          {entry.time}
        </span>
      </div>
      <div
        className="truncate font-serif text-[19px] italic leading-[1.1]"
        style={{ color: 'rgba(255,255,255,0.92)' }}
        title={cleanDomain}
      >
        {cleanDomain}
      </div>
    </div>
  )
}
