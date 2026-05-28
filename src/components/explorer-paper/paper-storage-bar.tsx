/**
 * Single-row storage breakdown bar used in the Audit ledger.
 *
 * Layout: sans label + mono size on top, accent-tinted bar beneath.
 * Tone variants pick a different accent strength so a stacked set of bars
 * reads as a hierarchy ("Core archive" > "FTS index" > "Embeddings" >
 * "Snapshots") at a glance.
 */

import { cn } from '@/lib/cn'

export type PaperStorageBarTone = 'primary' | 'secondary' | 'tertiary' | 'muted'

export interface PaperStorageBarProps {
  label: string
  /** Pre-formatted size string, e.g. "8.2 GB". */
  size: string
  /** Width percentage 0..100. */
  pct: number
  tone?: PaperStorageBarTone
  className?: string
  testId?: string
}

function toneFill(tone: PaperStorageBarTone): string {
  switch (tone) {
    case 'primary':
      return 'var(--accent)'
    case 'secondary':
      return 'color-mix(in srgb, var(--accent) 70%, var(--ink-faint))'
    case 'tertiary':
      return 'color-mix(in srgb, var(--accent) 45%, var(--ink-faint))'
    case 'muted':
      return 'var(--ink-faint)'
  }
}

export function PaperStorageBar({
  label,
  size,
  pct,
  tone = 'primary',
  className,
  testId,
}: PaperStorageBarProps) {
  const clampedPct = Math.max(0, Math.min(100, pct))
  return (
    <div
      data-testid={testId}
      data-tone={tone}
      className={cn('mb-3', className)}
    >
      <div className="text-ink-secondary mb-1 flex justify-between font-sans text-[12.5px]">
        <span>{label}</span>
        <span className="text-ink-muted font-mono text-[11px]">{size}</span>
      </div>
      <div
        aria-hidden="true"
        className="bg-page h-[6px] overflow-hidden rounded-[2px]"
      >
        <span
          className="block h-full"
          style={{
            width: `${clampedPct}%`,
            background: toneFill(tone),
          }}
        />
      </div>
    </div>
  )
}
