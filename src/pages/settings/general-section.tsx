/**
 * General settings section: language and performance preferences.
 *
 * Why this file exists:
 * - Extracted from the monolithic Settings route to keep each panel's
 *   presentational contract explicit and independently reviewable.
 * - General settings are low-stakes informational rows, so they get the
 *   lightest visual treatment.
 *
 * Main declarations:
 * - `GeneralSection`
 */

import { Glyph } from '../../components/ui'
import {
  explorerBackgroundPrefetchPageOptions,
  normalizeExplorerBackgroundPrefetchPages,
} from '../../lib/explorer-preferences'
import { languageLabel, supportedLanguages, useI18n } from '../../lib/i18n'
import type { AppSnapshot } from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Props for the extracted general Settings section.
 *
 * The route still owns mutations and side effects; this component only renders
 * the panel and forwards section-local interactions back to the route.
 */
export interface GeneralSectionProps {
  explorerBackgroundPrefetchPages: number
  navItem: SettingsSectionNavItem
  onExplorerBackgroundPrefetchPagesChange: (pages: number) => Promise<void>
  saving: boolean
  snapshot: AppSnapshot
  onLanguageChange: (language: string) => Promise<void>
}

/**
 * Renders the general Settings panel from route-owned state and callbacks.
 *
 * This keeps diagnostics rows, language controls, and build metadata in a
 * dedicated render module while the route retains the actual mutation logic.
 */
export function GeneralSection({
  explorerBackgroundPrefetchPages,
  navItem,
  onExplorerBackgroundPrefetchPagesChange,
  saving,
  snapshot,
  onLanguageChange,
}: GeneralSectionProps) {
  const { language, t } = useI18n()

  return (
    <div className="panel" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
      </div>
      <div className="panel-body panel-body--compact">
        <p className="dashboard-next-action">
          {t('settings.generalDescription')}
        </p>
        <div className="config-row">
          <span className="config-label">
            {t('settings.interfaceLanguage')}
          </span>
          <select
            aria-label={t('settings.interfaceLanguage')}
            className="settings-select"
            disabled={saving}
            value={snapshot.config.preferredLanguage}
            onChange={(event) => {
              void onLanguageChange(event.target.value)
            }}
          >
            <option value="system">{t('common.followSystem')}</option>
            {supportedLanguages.map((entry) => (
              <option key={entry} value={entry}>
                {languageLabel(entry, language)}
              </option>
            ))}
          </select>
        </div>
        <div className="config-row">
          <span className="config-label">{t('settings.currentLanguage')}</span>
          <span className="config-value">
            {languageLabel(language, language)}
          </span>
        </div>
        <div className="config-row">
          <span className="config-label">
            {t('settings.explorerBackgroundPrefetchPages')}
          </span>
          <select
            aria-label={t('settings.explorerBackgroundPrefetchPages')}
            className="settings-select"
            disabled={saving}
            value={normalizeExplorerBackgroundPrefetchPages(
              explorerBackgroundPrefetchPages,
            )}
            onChange={(event) => {
              void onExplorerBackgroundPrefetchPagesChange(
                Number.parseInt(event.target.value, 10),
              )
            }}
          >
            {explorerBackgroundPrefetchPageOptions.map((option) => (
              <option key={option} value={option}>
                {option === 0
                  ? t('settings.explorerBackgroundPrefetchDisabled')
                  : t('settings.explorerBackgroundPrefetchOption', {
                      count: option,
                    })}
              </option>
            ))}
          </select>
        </div>
        <p className="dashboard-next-action">
          {t('settings.explorerBackgroundPrefetchBody')}
        </p>
      </div>
    </div>
  )
}
