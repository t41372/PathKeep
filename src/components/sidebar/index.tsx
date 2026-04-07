import { sidebarSections } from '../../app/router'
import { useShellData } from '../../app/shell-data-context'
import { formatBytes } from '../../lib/format'
import { BrandMark } from '../brand-mark'
import { SidebarNavItem } from './nav-item'

interface SidebarProps {
  collapsed: boolean
  onToggle: () => void
}

export function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const { buildInfo, dashboard, snapshot } = useShellData()

  const archiveLabel = snapshot?.archiveStatus.warning
    ? 'Archive attention needed'
    : snapshot?.config.initialized
      ? 'Archive healthy'
      : 'Archive not initialized'
  const runtimeLabel = snapshot?.archiveStatus.encrypted
    ? 'Encrypted archive'
    : 'Plaintext archive'
  const archiveSize = formatBytes(dashboard?.storage.archiveDatabaseBytes)

  function toggleTheme() {
    const html = document.documentElement
    const current = html.getAttribute('data-theme')
    html.setAttribute('data-theme', current === 'light' ? 'dark' : 'light')
  }

  return (
    <aside
      className="sidebar"
      aria-label="Primary"
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
              {buildInfo ? `v${buildInfo.version}` : 'Loading build'}
            </span>
          </div>
        </div>
        <button
          aria-label={collapsed ? 'Expand navigation' : 'Collapse navigation'}
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
            <div className="nav-section-label">{section.label}</div>
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
        <button
          className="theme-toggle"
          type="button"
          aria-label="Toggle theme"
          onClick={toggleTheme}
        >
          <span>◐</span>
        </button>
      </footer>
    </aside>
  )
}
