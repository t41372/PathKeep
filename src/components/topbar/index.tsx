import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { AppScreen } from '../../app/router'
import { useShellData } from '../../app/shell-data-context'

interface TopbarProps {
  screen: AppScreen
}

export function Topbar({ screen }: TopbarProps) {
  const navigate = useNavigate()
  const { busyAction, notice, runBackup, snapshot } = useShellData()
  const [query, setQuery] = useState('')

  const backupDisabled = !snapshot || busyAction !== null
  const backupLabel = snapshot?.config.initialized
    ? 'Backup Now'
    : 'Initialize first'

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
        <form
          className="global-search"
          onSubmit={(event) => {
            event.preventDefault()
            const nextQuery = query.trim()
            void navigate(
              nextQuery
                ? `/explorer?q=${encodeURIComponent(nextQuery)}`
                : '/explorer',
            )
          }}
        >
          <span aria-hidden className="search-icon">
            ⌕
          </span>
          <input
            aria-label="Search history"
            className="search-input"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search history...  ⌘K"
            type="search"
          />
        </form>
        <button
          className="btn-backup"
          type="button"
          disabled={backupDisabled}
          onClick={() => {
            if (!snapshot?.config.initialized) {
              void navigate('/onboarding')
              return
            }
            void runBackup()
          }}
        >
          <span aria-hidden className="backup-icon">
            ▶
          </span>
          {busyAction ?? backupLabel}
        </button>
      </div>
      {notice ? <p className="topbar-notice">{notice}</p> : null}
    </header>
  )
}
