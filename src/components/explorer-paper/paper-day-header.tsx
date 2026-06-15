/**
 * Sticky day separator used inside the paper Browse contact sheet.
 *
 * ## Responsibilities
 * - Render the date label (Newsreader serif, 22 px), the secondary meta line
 *   ("N pages · M sessions"), and the right-aligned "Day N" index pill.
 * - Provide the sticky positioning so it pins below the toolbar as the user
 *   scrolls into the day's entries.
 * - Highlight the bottom border in accent when this day is the active target
 *   (e.g. arrived from On This Day or Search "See in context").
 *
 * ## Not responsible for
 * - Computing day labels or counts (caller supplies them ready-formatted).
 * - Driving the calendar / day-nav controls (those live in PaperDayNavControl).
 *
 * ## Dependencies
 * - Paper tokens (`text-ink`, `bg-paper`, `border-border*`) via `src/styles/tokens.css`.
 * - The sticky `top` reads the `--pk-toolbar-h` custom property that the
 *   contact-sheet root keeps in sync with the live (chip-wrapping) toolbar
 *   height. Reading a CSS variable — instead of a measured value threaded
 *   through React state — means the offset tracks the toolbar within the same
 *   layout/paint frame, with no render-cycle lag. Falls back to 44 px when no
 *   ancestor sets the property (e.g. standalone use).
 *
 * ## Performance notes
 * - Pure presentation, no effects. Safe to mount per-day in long lists.
 */

import { forwardRef, type ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface PaperDayHeaderProps {
  label: ReactNode
  meta?: ReactNode
  rightIndex?: ReactNode
  active?: boolean
  className?: string
  testId?: string
}

export const PaperDayHeader = forwardRef<HTMLDivElement, PaperDayHeaderProps>(
  function PaperDayHeader(
    { label, meta, rightIndex, active = false, className, testId },
    ref,
  ) {
    return (
      <div
        ref={ref}
        className={cn('bg-paper sticky z-[9] -mx-7 px-7', className)}
        style={{ top: 'var(--pk-toolbar-h, 44px)' }}
        data-testid={testId}
        data-active={active ? 'true' : undefined}
      >
        <div
          className={cn(
            'flex items-baseline justify-between gap-4 py-[14px] pb-[10px]',
            'border-b-[2px]',
            active ? 'border-accent' : 'border-border-default',
          )}
        >
          <div className="flex min-w-0 flex-1 flex-wrap items-baseline gap-3">
            <span className="text-ink whitespace-nowrap font-serif text-[22px] font-normal tracking-[-0.02em]">
              {label}
            </span>
            {meta ? (
              <span className="text-ink-faint whitespace-nowrap font-sans text-[12.5px]">
                {meta}
              </span>
            ) : null}
          </div>
          {rightIndex ? (
            <span className="text-ink-faint shrink-0 whitespace-nowrap font-mono text-[10px]">
              {rightIndex}
            </span>
          ) : null}
        </div>
      </div>
    )
  },
)
