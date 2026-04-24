/**
 * @file index.tsx
 * @description Renders the Maintenance route for updates, cleanup, diagnostics, and data repair workflows.
 * @module pages/maintenance
 *
 * ## Responsibilities
 * - Own workflow-heavy surfaces that should not live on the preference-only Settings route.
 * - Compose updater, retention cleanup, derived-state rebuild/clear, remote backup PME, diagnostics, and platform guidance.
 * - Keep destructive or long-running actions behind their existing preview/result state.
 *
 * ## Not responsible for
 * - Editing everyday user preferences that belong on `/settings`.
 * - Rendering external output payloads or generated integration artifacts owned by `/integrations`.
 * - Duplicating the canonical runtime queue, which remains `/jobs`.
 *
 * ## Dependencies
 * - Uses the shared Settings route-state hooks while enabling only Maintenance-owned workflow slices.
 * - Reuses existing extracted sections so behavior stays single-sourced during the IA cutover.
 *
 * ## Performance notes
 * - Expensive runtime/retention loads happen on this advanced route, not on the main Settings page.
 */

import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { useI18n } from '../../lib/i18n'
import { DerivedStateSection } from '../settings/derived-state-section'
import { PlatformSection } from '../settings/platform-section'
import { RemoteBackupSection } from '../settings/remote-backup-section'
import { RetentionSection } from '../settings/retention-section'
import {
  createSettingsSectionNavItems,
  getSettingsSectionNavItem,
  type SettingsSectionKey,
} from '../settings/section-nav-items'
import { SettingsSectionNav } from '../settings/section-nav'
import { UpdaterSection } from '../settings/updater-section'
import { useSettingsRouteState } from '../settings/use-settings-route-state'
import { DiagnosticsSection } from './diagnostics-section'

/**
 * Renders advanced Maintenance workflows after the Settings hard cutover.
 */
export function MaintenancePage() {
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
  const routeState = useSettingsRouteState({
    appLockStatus,
    buildInfo,
    clearAppLockPasscode,
    dashboard,
    enableAiIntegrationPreview: false,
    enableDerivedRuntime: true,
    enableRetentionPreview: true,
    lockAppSession,
    refreshAppData,
    refreshKey,
    saveConfig,
    setAppLockPasscode,
    setLanguagePreference,
    snapshot,
  })
  const maintenanceNavItems = createSettingsSectionNavItems(t, [
    'updater',
    'retention',
    'derived',
    'remote',
    'platform',
  ])
  const maintenanceSection = (key: SettingsSectionKey) =>
    getSettingsSectionNavItem(maintenanceNavItems, key)

  if (!snapshot) {
    if (loading || !routeState.supportStateLoaded) {
      return (
        <section className="page-shell">
          <LoadingState label={t('settings.loadingModules')} />
        </section>
      )
    }

    return (
      <section className="page-shell">
        <EmptyState
          action={
            <Link className="btn-primary" to="/security">
              {t('dashboard.reviewSecurity')}
            </Link>
          }
          description={t('settings.unavailableBody')}
          eyebrow={t('navigation.maintenanceLabel')}
          title={t('settings.maintenanceUnavailableTitle')}
        />
      </section>
    )
  }

  return (
    <section
      className="page-shell settings-page maintenance-page"
      data-testid="maintenance-page"
    >
      <SettingsSectionNav
        items={maintenanceNavItems}
        label={t('navigation.maintenanceLabel')}
      />

      <div className="settings-overview" aria-labelledby="maintenance-overview">
        <div className="settings-overview__intro">
          <h2 id="maintenance-overview">{t('settings.maintenanceTitle')}</h2>
          <p>{t('settings.maintenanceBody')}</p>
        </div>
        <div className="settings-advanced-grid">
          <Link className="settings-workflow-link-card" to="/jobs">
            <span className="settings-workflow-link-card__title">
              {t('navigation.jobsLabel')}
            </span>
            <span>{t('settings.openJobsBody')}</span>
          </Link>
          <Link className="settings-workflow-link-card" to="/settings">
            <span className="settings-workflow-link-card__title">
              {t('navigation.settingsLabel')}
            </span>
            <span>{t('settings.backToSettingsBody')}</span>
          </Link>
        </div>
      </div>

      <div className="settings-group">
        <div className="settings-group__label">
          {t('settings.groupMaintenance')}
        </div>
        <UpdaterSection
          navItem={maintenanceSection('updater')}
          state={routeState.updater}
        />
        <RetentionSection
          navItem={maintenanceSection('retention')}
          state={routeState.retention}
        />
      </div>

      <div className="settings-group">
        <div className="settings-group__label">
          {t('settings.groupDerivedData')}
        </div>
        <DerivedStateSection
          navItem={maintenanceSection('derived')}
          snapshot={snapshot}
          state={routeState.derived}
        />
      </div>

      <div className="settings-group">
        <div className="settings-group__label">
          {t('settings.groupBackupSync')}
        </div>
        <RemoteBackupSection
          credentialsSaved={snapshot.config.remoteBackup.credentialsSaved}
          lastError={snapshot.config.remoteBackup.lastError ?? null}
          lastUploadedAt={snapshot.config.remoteBackup.lastUploadedAt ?? null}
          lastUploadedObjectKey={
            snapshot.config.remoteBackup.lastUploadedObjectKey ?? null
          }
          navItem={maintenanceSection('remote')}
          state={routeState.remote}
        />
      </div>

      <div className="settings-group">
        <div className="settings-group__label">
          {t('settings.groupDiagnostics')}
        </div>
        <DiagnosticsSection
          buildInfo={buildInfo}
          copyFeedback={routeState.general.supportCopyFeedback}
          onCopyPath={routeState.general.onCopyPath}
          onOpenPath={routeState.general.onOpenPath}
          snapshot={snapshot}
        />
        <PlatformSection
          navItem={maintenanceSection('platform')}
          snapshot={snapshot}
          supportState={routeState.supportState}
        />
      </div>
    </section>
  )
}
