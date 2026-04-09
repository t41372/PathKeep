import clsx from 'clsx'
import { Link, useLocation } from 'react-router-dom'
import type { AppScreen } from '../../app/router'
import { useI18n } from '../../lib/i18n'

interface SidebarNavItemProps {
  screen: AppScreen
  collapsed: boolean
}

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
        {screen.icon}
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
