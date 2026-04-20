/**
 * Shared selectable row primitive for intelligence workbench member lists.
 *
 * Why this file exists:
 * - Explorer grouped views and promoted route member lists should share the
 *   same keyboard/click handling instead of duplicating accessibility glue.
 */

import type { KeyboardEvent, ReactNode } from 'react'

function handleKeyDown(
  event: KeyboardEvent<HTMLDivElement>,
  onSelect?: () => void,
) {
  if (!onSelect) {
    return
  }

  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    onSelect()
  }
}

export function WorkbenchEntityRow({
  actions,
  className,
  content,
  contentClassName,
  icon,
  iconClassName,
  meta,
  metaClassName,
  onSelect,
}: {
  actions?: ReactNode
  className: string
  content: ReactNode
  contentClassName: string
  icon?: ReactNode
  iconClassName?: string
  meta?: ReactNode
  metaClassName?: string
  onSelect?: () => void
}) {
  return (
    <div
      className={className}
      onClick={onSelect}
      onKeyDown={(event) => handleKeyDown(event, onSelect)}
      role={onSelect ? 'button' : undefined}
      tabIndex={onSelect ? 0 : undefined}
    >
      {icon ? <span className={iconClassName}>{icon}</span> : null}
      <span className={contentClassName}>{content}</span>
      {meta ? <span className={metaClassName}>{meta}</span> : null}
      {actions}
    </div>
  )
}
