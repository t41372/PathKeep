/**
 * Active-threads / sessions list used across Dashboard + Intelligence.
 *
 * Each row is a 3-column grid: a thin coloured pulse (hot / warm / cool),
 * the serif title + mono meta, and a right-aligned count + "pages" label.
 * Clicking a row routes the consumer to whatever drill-in is appropriate
 * (thread page, session day jump, etc.).
 *
 * ## Responsibilities
 * - Render the thread/session list.
 * - Map the tone tag to one of three pulse colours per the design.
 *
 * ## Not responsible for
 * - Computing tone — caller sets it based on recency / volume.
 */

import { cn } from '@/lib/cn'

export type PaperThreadTone = 'hot' | 'warm' | 'cool'

export interface PaperThreadRow {
  id: string
  title: string
  /** Mono meta line beneath the title, e.g. "12d · today". */
  meta: string
  count: number
  /** Tone drives the pulse colour; defaults to "hot". */
  tone?: PaperThreadTone
}

export interface PaperThreadListProps {
  rows: readonly PaperThreadRow[]
  onSelect?: (row: PaperThreadRow) => void
  /** Label suffix below the count, e.g. "pages" / "items". */
  countLabel?: string
  className?: string
  testId?: string
}

export function PaperThreadList({
  rows,
  onSelect,
  countLabel = 'pages',
  className,
  testId,
}: PaperThreadListProps) {
  return (
    <div data-testid={testId} className={cn('flex flex-col', className)}>
      {rows.map((row) => (
        <button
          type="button"
          key={row.id}
          onClick={() => onSelect?.(row)}
          disabled={!onSelect}
          data-testid={`paper-thread-${row.id}`}
          className={cn(
            'border-border-light grid grid-cols-[4px_1fr_auto] items-center gap-[10px]',
            'border-b py-3 last:border-b-0',
            'text-left',
            'enabled:cursor-pointer enabled:hover:bg-hover transition-colors duration-150',
            'disabled:cursor-default',
          )}
        >
          <span
            aria-hidden="true"
            className={cn(
              'self-stretch rounded-[2px]',
              row.tone === 'warm'
                ? 'bg-[color-mix(in_srgb,var(--accent)_50%,var(--ink-faint))]'
                : row.tone === 'cool'
                  ? 'bg-ink-faint'
                  : 'bg-accent',
            )}
          />
          <div className="min-w-0">
            <div className="text-ink truncate font-serif text-[14px] tracking-[-0.005em]">
              {row.title}
            </div>
            <div className="text-ink-faint mt-[3px] truncate font-mono text-[10.5px]">
              {row.meta}
            </div>
          </div>
          <div className="text-ink self-center text-right font-mono text-[18px] tracking-[-0.01em]">
            {row.count}
            <span className="text-ink-faint -mt-[2px] block font-mono text-[9px] uppercase tracking-[0.06em]">
              {countLabel}
            </span>
          </div>
        </button>
      ))}
    </div>
  )
}
