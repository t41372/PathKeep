import { sidebarSections } from '../../app/router'
import { useShellData } from '../../app/shell-data-context'
import { formatBytes } from '../../lib/format'
import { SidebarNavItem } from './nav-item'

export function Sidebar() {
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

  return (
    <aside className="sidebar" aria-label="Primary">
      <div className="sidebar-header">
        <div aria-hidden className="logo-mark">
          <span>PK</span>
        </div>
        <div className="logo-text">
          <span className="logo-name">PATHKEEP</span>
          <span className="logo-version">
            {buildInfo ? `v${buildInfo.version}` : 'Loading build'}
          </span>
        </div>
      </div>

      <nav className="nav-main">
        {sidebarSections.map((section) => (
          <div key={section.label}>
            <div className="nav-section-label">{section.label}</div>
            {section.items.map((item) => (
              <SidebarNavItem key={item.id} screen={item} />
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
      </footer>
    </aside>
  )
}
