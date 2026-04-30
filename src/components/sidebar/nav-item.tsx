/**
 * This module renders part of the sidebar navigation surface for the desktop shell.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `SidebarNavItem`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

import clsx from 'clsx'
import { Link, useLocation } from 'react-router-dom'
import type { AppScreen } from '../../app/router'
import { useI18n } from '../../lib/i18n'
import { Glyph } from '../ui'

/**
 * Describes the props accepted by `SidebarNavItem`.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
interface SidebarNavItemProps {
  screen: AppScreen
  collapsed: boolean
}

/**
 * Explains how sidebar nav item works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function SidebarNavItem({ screen, collapsed }: SidebarNavItemProps) {
  const location = useLocation()
  const { t } = useI18n()
  const isActive = location.pathname === screen.href
  const label = t(screen.labelKey)
  const badge = screen.badgeKey ? t(screen.badgeKey) : null

  return (
    <Link
      aria-label={label}
      aria-current={isActive ? 'page' : undefined}
      className={clsx('nav-item', {
        'nav-item--active': isActive,
        'nav-item--collapsed': collapsed,
      })}
      title={collapsed ? label : undefined}
      to={screen.href}
    >
      <span aria-hidden className="nav-icon">
        <Glyph icon={screen.icon} />
      </span>
      <span
        aria-hidden={collapsed}
        className={clsx('nav-label', {
          'nav-label--hidden': collapsed,
        })}
      >
        {label}
      </span>
      {badge && !collapsed ? <span className="nav-badge">{badge}</span> : null}
    </Link>
  )
}
