/**
 * @file index.tsx
 * @description Renders the Settings route shell and composes the extracted Settings sections around one canonical route-state hook.
 * @module pages/settings
 *
 * ## 職責
 * - 處理 Settings route 的 loading / unavailable / locked gating。
 * - 組合 sticky section nav 與各個 extracted Settings sections。
 * - 把 shell snapshot、shared support state 與 route hook 的 section state 接起來。
 *
 * ## 不負責
 * - 不直接持有 updater、AI、remote backup、derived-state 等 workflow 的本地 draft state。
 * - 不在 route shell 內重複定義 section-specific JSX 或 side effects。
 * - 不建立第二套 Settings section anchor / icon / label contract。
 *
 * ## 依賴關係
 * - 依賴 `useShellData()` 提供 shell snapshot 與 app actions。
 * - 依賴 `use-settings-route-state.ts` 作為 Settings workflow state 的唯一 owner。
 * - 依賴各 extracted section modules 渲染具體 panel UI。
 *
 * ## 性能備注
 * - route shell 只做 gating 和 composition，不再承擔重型 section-local JSX 或 duplicated background loads。
 */

import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { useI18n } from '../../lib/i18n'
import { AiProvidersSection } from './ai-providers-section'
import { AppearanceSection } from './appearance-section'
import { DataMigrationSection } from './data-migration-section'
import { LinkPreviewsSection } from './link-previews-section'
import { AppLockSection } from './app-lock-section'
import { GeneralSection } from './general-section'
import { PaperSettingsHeader } from './paper-settings-header'
import { ProfileSelectionSection } from './profile-selection-section'
import {
  createSettingsSectionNavItems,
  getSettingsSectionNavItem,
  type SettingsSectionKey,
} from './section-nav-items'
import { SettingsSectionNav } from './section-nav'
import { useSettingsRouteState } from './use-settings-route-state'

/**
 * Renders the Settings route shell around the extracted Settings sections.
 *
 * The page keeps only route-level gating and section composition so Settings
 * can keep growing without sliding back into a single mega-component.
 */
export function SettingsPage() {
  const {
    appLockStatus,
    buildInfo,
    clearAppLockPasscode,
    dashboard,
    loading,
    lockAppSession,
    refreshAppData,
    refreshKey,
    saveConfig,
    setAppLockPasscode,
    snapshot,
  } = useShellData()
  const { setLanguagePreference, t } = useI18n()
  const [searchParams] = useSearchParams()
  const paperLayout = searchParams.get('layout') === 'paper'
  const routeState = useSettingsRouteState({
    appLockStatus,
    buildInfo,
    clearAppLockPasscode,
    dashboard,
    enableAiIntegrationPreview: false,
    enableDerivedRuntime: false,
    enableRetentionPreview: false,
    lockAppSession,
    refreshAppData,
    refreshKey,
    saveConfig,
    setAppLockPasscode,
    setLanguagePreference,
    snapshot,
  })
  const settingsSectionNavItems = createSettingsSectionNavItems(t, [
    'general',
    'profiles',
    'applock',
    'ai',
    'migration',
    'linkPreviews',
  ])
  const settingsSection = (key: SettingsSectionKey) =>
    getSettingsSectionNavItem(settingsSectionNavItems, key)

  if (!snapshot) {
    if (loading || !routeState.supportStateLoaded) {
      return (
        <section className="page-shell">
          <LoadingState label={t('settings.loadingModules')} />
        </section>
      )
    }

    if (
      routeState.supportState.securityStatus?.encrypted &&
      !routeState.supportState.securityStatus.unlocked
    ) {
      return (
        <section className="page-shell">
          <EmptyState
            action={
              <Link className="btn-primary" to="/security">
                {t('dashboard.reviewSecurity')}
              </Link>
            }
            description={t('settings.archiveUnlockBody')}
            eyebrow={t('navigation.settingsLabel')}
            title={t('settings.archiveUnlockTitle')}
          />
        </section>
      )
    }

    return (
      <section className="page-shell">
        <EmptyState
          description={t('settings.unavailableBody')}
          eyebrow={t('navigation.settingsLabel')}
          title={t('settings.unavailableTitle')}
        />
      </section>
    )
  }

  return (
    <section className="page-shell settings-page" data-testid="settings-page">
      {paperLayout ? (
        <PaperSettingsHeader
          eyebrow={t('settings.paperHeaderEyebrow')}
          title={t('settings.paperHeaderTitle')}
          subtitle={t('settings.paperHeaderSubtitle')}
          jumpLabel={t('settings.paperJumpLabel')}
          items={settingsSectionNavItems}
          testId="settings-paper-header"
        />
      ) : (
        <SettingsSectionNav
          items={settingsSectionNavItems}
          label={t('navigation.settingsLabel')}
        />
      )}

      {paperLayout ? null : (
        <div className="settings-overview" aria-labelledby="settings-overview">
          <div className="settings-overview__intro">
            <h2 id="settings-overview">{t('settings.preferencesOverview')}</h2>
            <p>{t('settings.preferencesOverviewBody')}</p>
          </div>
          <div className="settings-advanced-grid">
            <Link className="settings-workflow-link-card" to="/maintenance">
              <span className="settings-workflow-link-card__title">
                {t('settings.openMaintenance')}
              </span>
              <span>{t('settings.openMaintenanceBody')}</span>
            </Link>
            <Link className="settings-workflow-link-card" to="/integrations">
              <span className="settings-workflow-link-card__title">
                {t('settings.openIntegrations')}
              </span>
              <span>{t('settings.openIntegrationsBody')}</span>
            </Link>
          </div>
        </div>
      )}

      <div className="settings-group">
        <div className="settings-group__label">{t('settings.groupCore')}</div>
        <AppearanceSection />
        <GeneralSection
          explorerBackgroundPrefetchPages={
            routeState.general.explorerBackgroundPrefetchPages
          }
          navItem={settingsSection('general')}
          onExplorerBackgroundPrefetchPagesChange={
            routeState.general.onExplorerBackgroundPrefetchPagesChange
          }
          onLanguageChange={routeState.general.onLanguageChange}
          saving={routeState.general.saving}
          snapshot={snapshot}
        />
        <ProfileSelectionSection
          navItem={settingsSection('profiles')}
          state={routeState.profiles}
        />
      </div>

      <div className="settings-group">
        <div className="settings-group__label">
          {t('settings.groupPrivacyAccess')}
        </div>
        <AppLockSection
          navItem={settingsSection('applock')}
          state={routeState.appLock}
        />
      </div>

      <div className="settings-group">
        <div className="settings-group__label">
          {t('settings.groupIntelligence')}
        </div>
        <AiProvidersSection
          navItem={settingsSection('ai')}
          state={routeState.ai}
        />
      </div>

      <div className="settings-group">
        <div className="settings-group__label">
          {t('settings.groupBackupSync')}
        </div>
        <DataMigrationSection navItem={settingsSection('migration')} />
        <LinkPreviewsSection anchorId={settingsSection('linkPreviews').id} />
      </div>
    </section>
  )
}
