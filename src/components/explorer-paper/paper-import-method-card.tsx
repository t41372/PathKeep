/**
 * Method picker card used at the top of the paper Import wizard.
 *
 * Three cards side-by-side in the design (`pk-import.jsx` `.import-methods`)
 * representing the available import paths: Google Takeout / Browser direct
 * / CSV-JSON. Each card has an icon slot, a serif title, a sans desc, and
 * a mono hint at the bottom. Active state flips border + background to
 * accent.
 *
 * ## Responsibilities
 * - Render the card content; surface click via onSelect.
 * - Reflect active state with accent border + soft fill.
 *
 * ## Not responsible for
 * - The icon — caller renders the SVG or text glyph as `children`.
 */

import { type ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface PaperImportMethodCardProps {
  id: string
  title: string
  description: string
  /** Short tail line, mono, e.g. "Recommended · ZIP or unpacked". */
  hint?: string
  /** Optional icon node rendered above the title. */
  icon?: ReactNode
  active?: boolean
  onSelect?: (id: string) => void
  className?: string
  testId?: string
}

export function PaperImportMethodCard({
  id,
  title,
  description,
  hint,
  icon,
  active = false,
  onSelect,
  className,
  testId,
}: PaperImportMethodCardProps) {
  return (
    <button
      type="button"
      data-testid={testId ?? `paper-import-method-${id}`}
      data-active={active ? 'true' : undefined}
      onClick={() => onSelect?.(id)}
      disabled={!onSelect}
      className={cn(
        'rounded-paper border bg-card-paper p-4 text-left transition-all duration-150',
        active
          ? 'border-accent bg-accent-soft'
          : 'border-border-default text-ink-secondary',
        'enabled:hover:border-ink-muted enabled:hover:-translate-y-[1px] enabled:hover:shadow-frame',
        'disabled:cursor-not-allowed disabled:opacity-60',
        className,
      )}
    >
      {icon ? (
        <div
          className={cn(
            'mb-[10px] grid h-8 w-8 place-items-center',
            active ? 'text-accent' : 'text-ink-muted',
          )}
        >
          {icon}
        </div>
      ) : null}
      <div className="text-ink mb-[2px] font-serif text-[15px] tracking-[-0.005em]">
        {title}
      </div>
      <div className="text-ink-muted font-sans text-[12px] leading-[1.4]">
        {description}
      </div>
      {hint ? (
        <div className="text-ink-faint mt-2 font-mono text-[10px]">{hint}</div>
      ) : null}
    </button>
  )
}
