import { Glyph } from './ui'
import { useApp, type PageId } from '../lib/app-context'

interface NavItem {
  id: PageId
  label: string
  icon: string
  disabled: boolean
}

export function Sidebar() {
  const { t, initialized, activePage, setActivePage } = useApp()

  const navItems: NavItem[] = [
    {
      id: 'dashboard',
      label: t('dashboardNav'),
      icon: 'dashboard',
      disabled: false,
    },
    {
      id: 'explorer',
      label: t('explorerNav'),
      icon: 'search',
      disabled: !initialized,
    },
    {
      id: 'insights',
      label: t('insightsNav'),
      icon: 'neurology',
      disabled: !initialized,
    },
    {
      id: 'activity',
      label: t('activityNav'),
      icon: 'fact_check',
      disabled: !initialized,
    },
    {
      id: 'import',
      label: t('importNav'),
      icon: 'upload_file',
      disabled: !initialized,
    },
    {
      id: 'settings',
      label: t('settingsNav'),
      icon: 'settings',
      disabled: false,
    },
  ]

  return (
    <aside className="sideRail">
      <div className="brandMark" aria-hidden="true">
        <Glyph filled icon="history" />
      </div>

      <nav className="navStack">
        {navItems.map((item) => (
          <button
            key={item.id}
            className={`navItem ${activePage === item.id ? 'active' : ''}`}
            disabled={item.disabled}
            type="button"
            title={item.label}
            aria-label={item.label}
            onClick={() => setActivePage(item.id)}
          >
            <Glyph icon={item.icon} />
            <span className="navLabel">{item.label}</span>
          </button>
        ))}
      </nav>

      <div className="sideRailFooter" aria-hidden="true">
        <Glyph icon="shield" />
      </div>
    </aside>
  )
}
