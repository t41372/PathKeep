/**
 * Shared capped-scroll body wrapper for Intelligence cards.
 */

import type { ReactNode } from 'react'

export function IntelligenceSectionBody({
  children,
  className,
  variant = 'default',
}: {
  children: ReactNode
  className?: string
  variant?: 'default' | 'workbench'
}) {
  const classes = [
    'intelligence-section__body',
    'intelligence-section__scroll-region',
    variant === 'workbench' ? 'intelligence-section__body--workbench' : '',
    className ?? '',
  ]
    .filter(Boolean)
    .join(' ')

  return <div className={classes}>{children}</div>
}
