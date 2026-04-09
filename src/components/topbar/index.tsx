import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { AppScreen } from '../../app/router'
import { useI18n } from '../../lib/i18n'
import { ProfileSwitcher } from '../profile-switcher'
import { useShellData } from '../../app/shell-data-context'

interface TopbarProps {
  screen: AppScreen
}

export function Topbar({ screen }: TopbarProps) {
  const navigate = useNavigate()
  const { t } = useI18n()
  const {
    appLockStatus,
    busyAction,
    lockAppSession,
    notice,
    runBackup,
    snapshot,
  } = useShellData()
  const [query, setQuery] = useState('')

  const backupDisabled = !snapshot || busyAction !== null
  const backupLabel = snapshot?.config.initialized
    ? t('navigation.backupNow')
    : t('navigation.initializeFirst')
  const title = t(screen.titleKey)
  const subtitle = t(screen.subtitleKey)

  return (
    <header className="topbar">
      <div className="topbar-left">
        <span aria-hidden className="crosshair-mark">
          +
        </span>
        <div>
          <h1 className="page-title">{title}</h1>
          <p className="page-subtitle">{subtitle}</p>
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
            aria-label={t('navigation.searchHistory')}
            className="search-input"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('navigation.searchHistoryPlaceholder')}
            type="search"
          />
        </form>
        <ProfileSwitcher />
        {appLockStatus?.enabled ? (
          <button
            className="btn-secondary"
            type="button"
            disabled={busyAction !== null}
            onClick={() => {
              void lockAppSession('manual')
            }}
          >
            {t('navigation.lockNow')}
          </button>
        ) : null}
        <button
          className="btn-backup"
          type="button"
          disabled={backupDisabled}
          aria-busy={busyAction !== null}
          onClick={() => {
            if (!snapshot?.config.initialized) {
              void navigate('/onboarding')
              return
            }
            void runBackup()
          }}
        >
          <span aria-hidden className="backup-icon">
            ⟳
          </span>
          {busyAction ?? backupLabel}
        </button>
      </div>
      {notice ? <p className="topbar-notice">{notice}</p> : null}
    </header>
  )
}
