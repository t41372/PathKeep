import clsx from 'clsx'
import { Link, useLocation } from 'react-router-dom'
import type { AppScreen } from '../../app/router'

interface SidebarNavItemProps {
  screen: AppScreen
  collapsed: boolean
}

export function SidebarNavItem({ screen, collapsed }: SidebarNavItemProps) {
  const location = useLocation()
  const isActive = location.pathname === screen.href

  return (
    <Link
      aria-label={screen.label}
      aria-current={isActive ? 'page' : undefined}
      className={clsx('nav-item', {
        'nav-item--active': isActive,
        'nav-item--collapsed': collapsed,
      })}
      title={collapsed ? screen.label : undefined}
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
        {screen.label}
      </span>
      {screen.badge && !collapsed ? (
        <span className="nav-badge">{screen.badge}</span>
      ) : null}
    </Link>
  )
}
