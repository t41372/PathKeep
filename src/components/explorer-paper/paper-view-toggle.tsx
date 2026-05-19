/**
 * Two-button pill toggle for the Browse view mode (Cards / List).
 *
 * Matches `.cs-view-toggle` from the handoff: 28 px tall, mono labels in caps
 * with `⊞ Cards` / `☰ List` symbols, accent-soft fill on the active side.
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface PaperViewToggleOption<TValue extends string> {
  value: TValue
  label: ReactNode
}

export interface PaperViewToggleProps<TValue extends string> {
  value: TValue
  options: PaperViewToggleOption<TValue>[]
  onChange: (next: TValue) => void
  ariaLabel?: string
  className?: string
  testId?: string
}

export function PaperViewToggle<TValue extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  className,
  testId,
}: PaperViewToggleProps<TValue>) {
  return (
    <div
      role="tablist"
      aria-label={ariaLabel}
      data-testid={testId}
      className={cn(
        'border-border-default bg-card-paper rounded-paper inline-flex h-7 items-center overflow-hidden border',
        className,
      )}
    >
      {options.map((option, index) => {
        const active = option.value === value
        return (
          <button
            key={option.value}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onChange(option.value)}
            className={cn(
              'h-full whitespace-nowrap font-mono text-[11px] tracking-[0.02em] px-3',
              'transition-colors duration-150',
              index < options.length - 1 && 'border-border-light border-r',
              active
                ? 'bg-accent-soft text-accent-text'
                : 'text-ink-muted hover:text-ink bg-transparent',
            )}
          >
            {option.label}
          </button>
        )
      })}
    </div>
  )
}
