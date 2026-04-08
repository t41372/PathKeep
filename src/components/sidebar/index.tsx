import { sidebarSections } from '../../app/router'
import { useShellData } from '../../app/shell-data-context'
import { formatBytes } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import { BrandMark } from '../brand-mark'
import { SidebarNavItem } from './nav-item'

interface SidebarProps {
  collapsed: boolean
  onToggle: () => void
}

export function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const { language, t } = useI18n()
  const { buildInfo, dashboard, snapshot } = useShellData()
  const { activeProfileId } = useProfileScope()

  const archiveLabel = snapshot?.archiveStatus.warning
    ? t('navigation.archiveAttentionNeeded')
    : snapshot?.config.initialized
      ? t('navigation.archiveHealthy')
      : t('navigation.archiveNotInitialized')
  const runtimeLabel = snapshot?.archiveStatus.encrypted
    ? t('navigation.encryptedArchive')
    : t('navigation.plaintextArchive')
  const archiveSize = formatBytes(
    dashboard?.storage.archiveDatabaseBytes,
    language,
  )
  const activeProfile = snapshot?.browserProfiles.find(
    (profile) => profile.profileId === activeProfileId,
  )
  const profileScopeLabel = activeProfileId
    ? (activeProfile?.profileName ?? profileIdLabel(activeProfileId))
    : t('common.profileAllProfiles')

  function toggleTheme() {
    const html = document.documentElement
    const current = html.getAttribute('data-theme')
    html.setAttribute('data-theme', current === 'light' ? 'dark' : 'light')
  }

  return (
    <aside
      className="sidebar"
      aria-label={t('navigation.coreSection')}
      data-collapsed={collapsed ? 'true' : 'false'}
    >
      <div className="sidebar-header">
        <div className="logo-lockup">
          <div aria-hidden className="logo-mark">
            <BrandMark alt="" />
          </div>
          <div className="logo-text">
            <span className="logo-name">PATHKEEP</span>
            <span className="logo-version">
              {buildInfo
                ? `v${buildInfo.version}`
                : t('navigation.loadingBuild')}
            </span>
          </div>
        </div>
        <button
          aria-label={
            collapsed
              ? t('navigation.expandNavigation')
              : t('navigation.collapseNavigation')
          }
          className="sidebar-toggle"
          type="button"
          onClick={onToggle}
        >
          <span aria-hidden>{collapsed ? '→' : '←'}</span>
        </button>
      </div>

      <nav className="nav-main">
        {sidebarSections.map((section) => (
          <div key={section.label} className="nav-section">
            <div className="nav-section-label">
              {section.labelKey ? t(section.labelKey) : section.label}
            </div>
            {section.items.map((item) => (
              <SidebarNavItem
                key={item.id}
                collapsed={collapsed}
                screen={item}
              />
            ))}
          </div>
        ))}
      </nav>

      <footer className="sidebar-footer">
        <div className="sidebar-status">
          <span
            aria-hidden
            className={`status-dot ${
              snapshot?.archiveStatus.warning
                ? 'status-dot--accent'
                : 'status-dot--ok'
            }`}
          />
          <span>{archiveLabel}</span>
        </div>
        <div className="sidebar-meta">
          <span>{runtimeLabel}</span>
          <span className="sep">·</span>
          <span>{archiveSize}</span>
        </div>
        <div className="sidebar-profile-scope">
          <span className="mono-support">
            {t('common.profileScope')}: {profileScopeLabel}
          </span>
        </div>
        <button
          className="theme-toggle"
          type="button"
          aria-label={t('navigation.toggleTheme')}
          onClick={toggleTheme}
        >
          <span>◐</span>
        </button>
      </footer>
    </aside>
  )
}
