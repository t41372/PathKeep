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
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import { DerivedStateSection } from '../settings/derived-state-section'
import { PlatformSection } from '../settings/platform-section'
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
 *
 * Paper aesthetic: every workflow group lives in its own PaperCard with a
 * PaperCardHeader carrying the group label. The inner workflow sections
 * keep their existing chrome until Phase 3 sweeps the Settings sub-section
 * primitives.
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
    'platform',
  ])
  const maintenanceSection = (key: SettingsSectionKey) =>
    getSettingsSectionNavItem(maintenanceNavItems, key)

  if (!snapshot) {
    if (loading || !routeState.supportStateLoaded) {
      return (
        <div
          className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
          data-testid="maintenance-page"
        >
          <LoadingState label={t('settings.loadingModules')} />
        </div>
      )
    }

    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="maintenance-page"
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
          eyebrow={t('navigation.maintenanceLabel')}
          title={t('settings.maintenanceUnavailableTitle')}
        />
      </div>
    )
  }

  return (
    <div
      className="mx-auto flex w-full max-w-[1080px] flex-col gap-4 pt-7"
      data-testid="maintenance-page"
    >
      <SettingsSectionNav
        items={maintenanceNavItems}
        label={t('navigation.maintenanceLabel')}
      />

      <PaperCard>
        <PaperCardHeader title={t('settings.maintenanceTitle')} />
        <PaperCardBody>
          <p
            id="maintenance-overview"
            className="text-ink-muted m-0 mb-4 font-serif text-[13.5px] leading-[1.55] italic"
          >
            {t('settings.maintenanceBody')}
          </p>
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            <Link
              to="/jobs"
              className="border-border-default hover:border-ink-muted hover:bg-hover rounded-paper flex flex-col gap-1 border px-3 py-2 transition-colors"
              data-testid="maintenance-jobs-link"
            >
              <span className="text-ink font-sans text-[12.5px] font-medium">
                {t('navigation.jobsLabel')}
              </span>
              <span className="text-ink-muted font-sans text-[11.5px]">
                {t('settings.openJobsBody')}
              </span>
            </Link>
            <Link
              to="/settings"
              className="border-border-default hover:border-ink-muted hover:bg-hover rounded-paper flex flex-col gap-1 border px-3 py-2 transition-colors"
              data-testid="maintenance-back-to-settings-link"
            >
              <span className="text-ink font-sans text-[12.5px] font-medium">
                {t('navigation.settingsLabel')}
              </span>
              <span className="text-ink-muted font-sans text-[11.5px]">
                {t('settings.backToSettingsBody')}
              </span>
            </Link>
          </div>
        </PaperCardBody>
      </PaperCard>

      <PaperCard>
        <PaperCardHeader title={t('settings.groupMaintenance')} />
        <PaperCardBody>
          <UpdaterSection
            navItem={maintenanceSection('updater')}
            state={routeState.updater}
          />
          <RetentionSection
            navItem={maintenanceSection('retention')}
            state={routeState.retention}
          />
        </PaperCardBody>
      </PaperCard>

      <PaperCard>
        <PaperCardHeader title={t('settings.groupDerivedData')} />
        <PaperCardBody>
          <DerivedStateSection
            navItem={maintenanceSection('derived')}
            snapshot={snapshot}
            state={routeState.derived}
          />
        </PaperCardBody>
      </PaperCard>

      <PaperCard>
        <PaperCardHeader title={t('settings.groupDiagnostics')} />
        <PaperCardBody>
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
        </PaperCardBody>
      </PaperCard>
    </div>
  )
}
