import { Link } from 'react-router-dom'
import type { AppScreen } from '../../app/router'

interface TopbarProps {
  screen: AppScreen
}

export function Topbar({ screen }: TopbarProps) {
  return (
    <header className="topbar">
      <div className="topbar-left">
        <span aria-hidden className="crosshair-mark">
          +
        </span>
        <div>
          <h1 className="page-title">{screen.title}</h1>
          <p className="page-subtitle">{screen.subtitle}</p>
        </div>
      </div>

      <div className="topbar-right">
        <label className="global-search">
          <span aria-hidden className="search-icon">
            ⌕
          </span>
          <input
            aria-label="Search history"
            className="search-input"
            placeholder="Search history...  ⌘K"
            type="search"
          />
        </label>
        <Link className="ghost-button" to="/onboarding">
          Review onboarding
        </Link>
        <button className="primary-button" type="button">
          Backup Now
        </button>
      </div>
    </header>
  )
}
