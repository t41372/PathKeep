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

import {
  PaperCard,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import {
  explorerBackgroundPrefetchPageOptions,
  normalizeExplorerBackgroundPrefetchPages,
} from '../../lib/explorer-preferences'
import { languageLabel, supportedLanguages, useI18n } from '../../lib/i18n'
import type { AppSnapshot } from '../../lib/types'
import { Field } from './paper-form-primitives'
import type { SettingsSectionNavItem } from './section-nav-items'

const SELECT_CLASS =
  'border-border-default rounded-paper bg-paper text-ink font-sans text-[12.5px] px-2 py-1 focus:border-accent focus:outline-none disabled:opacity-60'

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
 * Paper aesthetic: wraps the panel in PaperCard with PaperCardHeader carrying
 * the navItem label, and uses the shared `Field` primitive for each row. The
 * native `<select>` controls get paper-token styling inline (border-border-
 * default / rounded-paper / bg-paper / font-sans).
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
    <PaperCard testId={navItem.id}>
      <span id={navItem.id} aria-hidden />
      <PaperCardHeader title={navItem.label} />
      <PaperCardBody>
        <p className="text-ink-muted m-0 mb-4 font-serif text-[13.5px] leading-[1.55] italic">
          {t('settings.generalDescription')}
        </p>

        <Field label={t('settings.interfaceLanguage')}>
          <select
            aria-label={t('settings.interfaceLanguage')}
            className={SELECT_CLASS}
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
        </Field>

        <Field label={t('settings.currentLanguage')}>
          <span className="text-ink-muted font-mono text-[11.5px]">
            {languageLabel(language, language)}
          </span>
        </Field>

        <Field
          label={t('settings.explorerBackgroundPrefetchPages')}
          help={t('settings.explorerBackgroundPrefetchBody')}
        >
          <select
            aria-label={t('settings.explorerBackgroundPrefetchPages')}
            className={SELECT_CLASS}
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
        </Field>
      </PaperCardBody>
    </PaperCard>
  )
}
