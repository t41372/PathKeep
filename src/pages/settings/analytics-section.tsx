/**
 * Analytics consent settings section.
 *
 * Why this file exists:
 * - Extracted from the monolithic Settings route to keep each panel's
 *   presentational contract explicit and independently reviewable.
 * - Analytics is an optional, consent-driven feature with clear boundary
 *   semantics, making it a natural extraction candidate.
 *
 * Main declarations:
 * - `AnalyticsSection`
 */

import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import { useI18n } from '../../lib/i18n'
import type { AnalyticsConfig } from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Props for the extracted analytics Settings section.
 *
 * The route keeps the draft state and save behavior, while this component
 * renders the consent surface from the current route-owned snapshot.
 */
export interface AnalyticsSectionProps {
  analyticsAction: string | null
  analyticsConfigDirty: boolean
  analyticsEndpointConfigured: boolean
  currentAnalyticsSettings: AnalyticsConfig
  navItem: SettingsSectionNavItem
  onAnalyticsEnabledChange: (enabled: boolean) => void
  onSaveAnalyticsConsent: () => Promise<void>
}

/**
 * Renders the analytics consent panel from route-owned draft state.
 *
 * Keeping this surface extracted makes the Settings route smaller without
 * changing the underlying config contract or save flow.
 */
export function AnalyticsSection({
  analyticsAction,
  analyticsConfigDirty,
  analyticsEndpointConfigured,
  currentAnalyticsSettings,
  navItem,
  onAnalyticsEnabledChange,
  onSaveAnalyticsConsent,
}: AnalyticsSectionProps) {
  const { t } = useI18n()

  return (
    <div className="panel panel--optional" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
        <span className="panel-badge">{t('settings.optional')}</span>
      </div>
      <div className="panel-body settings-remote-grid">
        <StatusCallout
          tone={currentAnalyticsSettings.enabled ? 'warning' : 'info'}
          title={t('settings.analyticsBoundaryTitle')}
          body={t('settings.analyticsBoundaryBody')}
        />

        {!analyticsEndpointConfigured ? (
          <StatusCallout
            tone="warning"
            title={t('settings.analyticsEndpointMissingTitle')}
            body={t('settings.analyticsEndpointMissingBody')}
          />
        ) : null}

        <div className="settings-field-grid">
          <label className="checkbox-row">
            <input
              aria-label={t('settings.analyticsEnabled')}
              checked={currentAnalyticsSettings.enabled}
              type="checkbox"
              onChange={(event) => {
                onAnalyticsEnabledChange(event.target.checked)
              }}
            />
            <span>{t('settings.analyticsEnabled')}</span>
          </label>

          <div className="config-row">
            <span className="config-label">
              {t('settings.analyticsEndpoint')}
            </span>
            <span className="config-value mono">
              {analyticsEndpointConfigured
                ? import.meta.env.VITE_ANALYTICS_ENDPOINT
                : t('common.notAvailable')}
            </span>
          </div>

          <div className="config-row">
            <span className="config-label">
              {t('settings.analyticsConsentGrantedAt')}
            </span>
            <span className="config-value mono">
              {currentAnalyticsSettings.consentGrantedAt ??
                t('common.notAvailable')}
            </span>
          </div>

          <p className="dashboard-next-action">
            {t('settings.analyticsStatusBody')}
          </p>

          <div className="settings-action-row">
            <button
              className="btn-primary"
              type="button"
              disabled={Boolean(analyticsAction) || !analyticsConfigDirty}
              onClick={() => {
                void onSaveAnalyticsConsent()
              }}
            >
              {analyticsAction ?? t('settings.analyticsSave')}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
