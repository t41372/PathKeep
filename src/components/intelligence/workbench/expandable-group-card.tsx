/**
 * Shared expandable card shell for grouped Explorer workbench surfaces.
 *
 * Why this file exists:
 * - Session and trail grouped views need the same expand/collapse frame while
 *   keeping entity-specific header/body content outside the primitive.
 */

import type { ReactNode } from 'react'

export function WorkbenchExpandableGroupCard({
  bodyClassName,
  children,
  expanded,
  headerClassName,
  headerContent,
  onToggle,
  rootClassName,
}: {
  bodyClassName: string
  children?: ReactNode
  expanded: boolean
  headerClassName: string
  headerContent: ReactNode
  onToggle: () => void
  rootClassName: string
}) {
  return (
    <div className={`${rootClassName}${expanded ? ` ${rootClassName}--expanded` : ''}`}>
      <button
        aria-expanded={expanded}
        className={headerClassName}
        type="button"
        onClick={onToggle}
      >
        {headerContent}
      </button>
      {expanded ? <div className={bodyClassName}>{children}</div> : null}
    </div>
  )
}
