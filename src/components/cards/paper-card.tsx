/**
 * Paper card primitives shared across every redesigned route.
 *
 * Why this file exists:
 * - The design grammar uses `pk-card` everywhere: bordered surface, optional
 *   accent left border, header with serif title + mono badge / "→" link, body
 *   with padding control. Centralizing this primitive prevents every route
 *   from reinventing the same shell.
 *
 * Responsibilities:
 * - Render a Card surface with consistent border / radius / shadow.
 * - Render a CardHeader with serif title on the left and a slot on the right.
 * - Render a CardBadge / CardLink for the right-slot conventions.
 *
 * Not responsible for:
 * - Specific content layouts (each route composes children itself).
 */

import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

export interface PaperCardProps {
  children: ReactNode
  className?: string
  accent?: boolean
  testId?: string
  /**
   * Optional DOM id forwarded to the root element. Settings sections + similar
   * surfaces use it for hash-link scrolling (`#settings-derived` etc.) and
   * for test queries that look up the panel via `document.getElementById`.
   * Defaults to `testId` when only one identifier is needed for both.
   */
  id?: string
}

export function PaperCard({
  children,
  className,
  accent = false,
  testId,
  id,
}: PaperCardProps) {
  return (
    <section
      className={cn(
        'bg-card-paper border border-border-light shadow-frame overflow-hidden',
        'rounded-paper',
        accent && 'border-accent border-l-[3px]',
        className,
      )}
      data-testid={testId}
      id={id ?? testId}
    >
      {children}
    </section>
  )
}

export interface PaperCardHeaderProps {
  title: ReactNode
  right?: ReactNode
  compact?: boolean
  className?: string
}

export function PaperCardHeader({
  title,
  right,
  compact = false,
  className,
}: PaperCardHeaderProps) {
  return (
    <header
      className={cn(
        'border-border-light flex items-center justify-between gap-2 border-b',
        compact ? 'px-[18px] py-[10px]' : 'px-[18px] py-[14px]',
        className,
      )}
    >
      <span className="font-serif text-[14px] font-medium tracking-[-0.005em] text-ink">
        {title}
      </span>
      {right ? (
        <div className="flex items-center gap-3 font-mono text-[10px] text-ink-faint">
          {right}
        </div>
      ) : null}
    </header>
  )
}

export interface PaperCardBodyProps {
  children: ReactNode
  className?: string
  padded?: boolean
}

export function PaperCardBody({
  children,
  className,
  padded = true,
}: PaperCardBodyProps) {
  return (
    <div className={cn(padded ? 'p-[18px]' : '', className)}>{children}</div>
  )
}

export interface PaperCardBadgeProps {
  children: ReactNode
  onClick?: () => void
  className?: string
}

export function PaperCardBadge({
  children,
  onClick,
  className,
}: PaperCardBadgeProps) {
  if (onClick) {
    return (
      <button
        type="button"
        onClick={onClick}
        className={cn(
          'hover:text-accent text-ink-faint cursor-pointer font-mono text-[10px] transition-colors',
          className,
        )}
      >
        {children}
      </button>
    )
  }
  return (
    <span className={cn('text-ink-faint font-mono text-[10px]', className)}>
      {children}
    </span>
  )
}
