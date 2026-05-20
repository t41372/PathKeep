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
import {
  PaperCard,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { SettingsExternalOutputsPanel } from '../settings/external-outputs-panel'
import { useSettingsRouteState } from '../settings/use-settings-route-state'
import { AiIntegrationReviewSection } from './ai-integration-review-section'

/**
 * Renders external-output and generated integration artifact review surfaces.
 *
 * The v0.3 paper aesthetic frames the route with a single composition: an
 * overview card carrying the title, intro, and link-out chips to the sibling
 * Settings / Maintenance routes, followed by two PaperCards for the two
 * functional groups (external outputs + generated artifacts). The inner
 * panels keep their existing chrome until Phase 3 sweeps the Settings
 * sub-section primitives.
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
        <div
          className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
          data-testid="integrations-page"
        >
          <LoadingState label={t('settings.loadingModules')} />
        </div>
      )
    }

    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="integrations-page"
      >
        <EmptyState
          action={
            <Link
              className="border-accent text-accent-text hover:bg-accent-soft rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px]"
              to="/security"
            >
              {t('dashboard.reviewSecurity')}
            </Link>
          }
          description={t('settings.unavailableBody')}
          eyebrow={t('navigation.integrationsLabel')}
          title={t('settings.integrationsUnavailableTitle')}
        />
      </div>
    )
  }

  return (
    <div
      className="mx-auto flex w-full max-w-[1080px] flex-col gap-4 pt-7"
      data-testid="integrations-page"
    >
      <PaperCard>
        <PaperCardHeader title={t('settings.integrationsTitle')} />
        <PaperCardBody>
          <p
            id="integrations-overview"
            className="text-ink-muted m-0 mb-4 font-serif text-[13.5px] leading-[1.55] italic"
          >
            {t('settings.integrationsBody')}
          </p>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            <Link
              to="/settings"
              className="border-border-default hover:border-ink-muted hover:bg-hover rounded-paper flex flex-col gap-1 border px-3 py-2 transition-colors"
              data-testid="integrations-back-to-settings-link"
            >
              <span className="text-ink font-sans text-[12.5px] font-medium">
                {t('navigation.settingsLabel')}
              </span>
              <span className="text-ink-muted font-sans text-[11.5px]">
                {t('settings.backToSettingsBody')}
              </span>
            </Link>
            <Link
              to="/maintenance"
              className="border-border-default hover:border-ink-muted hover:bg-hover rounded-paper flex flex-col gap-1 border px-3 py-2 transition-colors"
              data-testid="integrations-maintenance-link"
            >
              <span className="text-ink font-sans text-[12.5px] font-medium">
                {t('navigation.maintenanceLabel')}
              </span>
              <span className="text-ink-muted font-sans text-[11.5px]">
                {t('settings.openMaintenanceBody')}
              </span>
            </Link>
          </div>
        </PaperCardBody>
      </PaperCard>

      <PaperCard>
        <PaperCardHeader title={t('settings.groupExternalOutputs')} />
        <PaperCardBody>
          <SettingsExternalOutputsPanel
            initialized={snapshot.config.initialized}
            unlocked={snapshot.archiveStatus.unlocked}
          />
        </PaperCardBody>
      </PaperCard>

      <PaperCard>
        <PaperCardHeader title={t('settings.groupGeneratedArtifacts')} />
        <PaperCardBody>
          <AiIntegrationReviewSection state={routeState.ai} />
        </PaperCardBody>
      </PaperCard>
    </div>
  )
}
