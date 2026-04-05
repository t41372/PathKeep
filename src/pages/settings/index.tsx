import { useApp, type SettingsTab } from '../../lib/app-context'
import { Glyph } from '../../components/ui'
import { GeneralSettings } from './general'
import { SourcesSettings } from './sources'
import { ScheduleSettings } from './schedule'
import { SecuritySettings } from './security'
import { RemoteSettings } from './remote'
import { AiProvidersSettings } from './ai-providers'

interface SubNavItem {
  id: SettingsTab
  label: string
  icon: string
}

export function SettingsPage() {
  const { t, activeSettingsTab, setActiveSettingsTab } = useApp()

  const subNavItems: SubNavItem[] = [
    { id: 'general', label: t('settingsGeneral'), icon: 'tune' },
    { id: 'sources', label: t('settingsSources'), icon: 'source' },
    { id: 'schedule', label: t('settingsSchedule'), icon: 'schedule' },
    { id: 'security', label: t('settingsSecurity'), icon: 'shield' },
    { id: 'remote', label: t('settingsRemote'), icon: 'cloud_upload' },
    {
      id: 'ai-providers',
      label: t('settingsAiProviders'),
      icon: 'smart_toy',
    },
  ]

  const renderTab = () => {
    switch (activeSettingsTab) {
      case 'general':
        return <GeneralSettings />
      case 'sources':
        return <SourcesSettings />
      case 'schedule':
        return <ScheduleSettings />
      case 'security':
        return <SecuritySettings />
      case 'remote':
        return <RemoteSettings />
      case 'ai-providers':
        return <AiProvidersSettings />
    }
  }

  return (
    <div className="settingsLayout">
      <nav className="settingsSubNav">
        <h3 className="settingsSubNavTitle">{t('settingsNav')}</h3>
        {subNavItems.map((item) => (
          <button
            key={item.id}
            className={`settingsSubNavItem ${activeSettingsTab === item.id ? 'active' : ''}`}
            type="button"
            onClick={() => setActiveSettingsTab(item.id)}
          >
            <Glyph icon={item.icon} />
            <span>{item.label}</span>
          </button>
        ))}
      </nav>
      <div className="settingsContent">{renderTab()}</div>
    </div>
  )
}
