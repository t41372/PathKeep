import { shellStatus } from '../../app/preview-data'
import { sidebarSections } from '../../app/router'
import { SidebarNavItem } from './nav-item'

export function Sidebar() {
  return (
    <aside className="sidebar" aria-label="Primary">
      <div className="sidebar-header">
        <div aria-hidden className="logo-mark">
          <span>PK</span>
        </div>
        <div className="logo-text">
          <span className="logo-name">PATHKEEP</span>
          <span className="logo-version">{shellStatus.version}</span>
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
          <span aria-hidden className="status-dot status-dot--ok" />
          <span>{shellStatus.archiveHealth}</span>
        </div>
        <div className="sidebar-meta">
          <span>{shellStatus.runtime}</span>
          <span className="sep">·</span>
          <span>{shellStatus.archiveSize}</span>
        </div>
      </footer>
    </aside>
  )
}
