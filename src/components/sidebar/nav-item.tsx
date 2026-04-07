import clsx from 'clsx'
import { Link, useLocation } from 'react-router-dom'
import type { AppScreen } from '../../app/router'

interface SidebarNavItemProps {
  screen: AppScreen
}

export function SidebarNavItem({ screen }: SidebarNavItemProps) {
  const location = useLocation()
  const isActive = location.pathname === screen.href

  return (
    <Link
      aria-current={isActive ? 'page' : undefined}
      className={clsx('nav-item', {
        'nav-item--active': isActive,
      })}
      to={screen.href}
    >
      <span aria-hidden className="nav-icon">
        {screen.icon}
      </span>
      <span className="nav-label">{screen.label}</span>
      {screen.badge ? <span className="nav-badge">{screen.badge}</span> : null}
    </Link>
  )
}
