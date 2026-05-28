/**
 * Refind shelf — list of pages the user keeps revisiting, surfaced in
 * Intelligence and the Dashboard.
 *
 * Each entry follows the design's `.otd-entry` pattern: domain swatch on
 * the left, serif title + mono meta on the right. Row click routes to the
 * detail surface.
 *
 * ## Responsibilities
 * - Render the shelf with deterministic domain swatches.
 * - Surface click + keyboard activation.
 *
 * ## Not responsible for
 * - Ranking / scoring — caller pre-sorts entries.
 * - Domain colour / abbreviation — caller supplies them so the shelf
 *   matches whatever palette the Browse view uses.
 */

import { cn } from '@/lib/cn'

export interface PaperRefindItem {
  id: string
  title: string
  domain: string
  /** Mono meta line, e.g. "47 visits · over 11 months". */
  meta: string
}

export interface PaperRefindShelfProps {
  items: readonly PaperRefindItem[]
  resolveDomainColor: (domain: string) => string
  resolveDomainAbbr: (domain: string) => string
  onSelect?: (item: PaperRefindItem) => void
  className?: string
  testId?: string
}

export function PaperRefindShelf({
  items,
  resolveDomainColor,
  resolveDomainAbbr,
  onSelect,
  className,
  testId,
}: PaperRefindShelfProps) {
  return (
    <div data-testid={testId} className={cn('flex flex-col', className)}>
      {items.map((item) => (
        <button
          type="button"
          key={item.id}
          onClick={() => onSelect?.(item)}
          disabled={!onSelect}
          data-testid={`paper-refind-${item.id}`}
          className={cn(
            'border-border-light flex items-start gap-3 border-b py-[10px] last:border-b-0',
            'text-left transition-colors duration-150',
            'enabled:cursor-pointer enabled:hover:bg-hover',
            'disabled:cursor-default',
          )}
        >
          <span
            aria-hidden="true"
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[3px] font-mono text-[10px] font-semibold"
            style={{
              background: resolveDomainColor(item.domain),
              color: 'rgba(255,255,255,0.7)',
            }}
          >
            {resolveDomainAbbr(item.domain)}
          </span>
          <div className="min-w-0">
            <div className="text-ink line-clamp-2 font-serif text-[13px] leading-[1.35]">
              {item.title}
            </div>
            <div className="text-ink-faint mt-[2px] font-mono text-[10.5px]">
              {item.meta}
            </div>
          </div>
        </button>
      ))}
    </div>
  )
}
