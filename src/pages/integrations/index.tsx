/**
 * @file index.tsx
 * @description Renders the Integrations route for external outputs and generated local artifacts.
 * @module pages/integrations
 *
 * ## Responsibilities
 * - Own manual external-output review, trusted local-host snippet generation, and AI/MCP generated artifacts.
 * - Keep raw payloads and generated code behind bounded review panels outside preference Settings.
 * - Preserve local-first, preview-first honesty for every external integration surface.
 *
 * ## Not responsible for
 * - Editing persistent application preferences that belong on `/settings`.
 * - Running maintenance cleanup, derived rebuilds, or updater workflows.
 * - Installing remote services or publishing public APIs automatically.
 *
 * ## Dependencies
 * - Uses Settings-owned AI state for integration preview so generated artifact logic stays single-sourced.
 * - Reuses the existing external outputs panel while moving its route ownership to `/integrations`.
 *
 * ## Performance notes
 * - External payload loads happen only on this route and remain bounded by the selected time range/scope.
 */

import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { useI18n } from '../../lib/i18n'
import { SettingsExternalOutputsPanel } from '../settings/external-outputs-panel'
import { useSettingsRouteState } from '../settings/use-settings-route-state'
import { AiIntegrationReviewSection } from './ai-integration-review-section'

/**
 * Renders external-output and generated integration artifact review surfaces.
 */
export function IntegrationsPage() {
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
          eyebrow={t('navigation.integrationsLabel')}
          title={t('settings.integrationsUnavailableTitle')}
        />
      </section>
    )
  }

  return (
    <section
      className="page-shell settings-page integrations-page"
      data-testid="integrations-page"
    >
      <div
        className="settings-overview"
        aria-labelledby="integrations-overview"
      >
        <div className="settings-overview__intro">
          <h2 id="integrations-overview">{t('settings.integrationsTitle')}</h2>
          <p>{t('settings.integrationsBody')}</p>
        </div>
        <div className="settings-advanced-grid">
          <Link className="settings-workflow-link-card" to="/settings">
            <span className="settings-workflow-link-card__title">
              {t('navigation.settingsLabel')}
            </span>
            <span>{t('settings.backToSettingsBody')}</span>
          </Link>
          <Link className="settings-workflow-link-card" to="/maintenance">
            <span className="settings-workflow-link-card__title">
              {t('navigation.maintenanceLabel')}
            </span>
            <span>{t('settings.openMaintenanceBody')}</span>
          </Link>
        </div>
      </div>

      <div className="settings-group">
        <div className="settings-group__label">
          {t('settings.groupExternalOutputs')}
        </div>
        <SettingsExternalOutputsPanel
          initialized={snapshot.config.initialized}
          unlocked={snapshot.archiveStatus.unlocked}
        />
      </div>

      <div className="settings-group">
        <div className="settings-group__label">
          {t('settings.groupGeneratedArtifacts')}
        </div>
        <AiIntegrationReviewSection state={routeState.ai} />
      </div>
    </section>
  )
}
