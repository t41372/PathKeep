/**
 * Settings → General: language, appearance, how PathKeep sits on the desktop
 * (open at login, menu bar icon) and the two features that contact websites.
 */
import {
  SettingRow,
  SettingsGroup,
  SettingsSection,
} from '@/components/app/setting-row'
import { languageNames, supportedLanguages, useI18n } from '@/lib/i18n'
import { useChooseLanguage } from '@/lib/queries/language'
import { useTheme, type ThemePreference } from '@/lib/theme'
import type { LanguagePreference } from '@/lib/types'
import { DesktopRows } from './desktop-rows'
import { OnlineRows } from './online-rows'
import { RowSelect } from './row-select'

export function GeneralSection() {
  const { t } = useI18n()
  return (
    <SettingsSection title={t('settings.nav.general')}>
      <LanguageRow />
      <AppearanceRow />
      <DesktopRows />
      <SettingsGroup
        title={t('settings.general.online.title')}
        note={t('settings.general.online.note')}
      />
      <OnlineRows />
    </SettingsSection>
  )
}

function LanguageRow() {
  const { t, preference } = useI18n()
  const chooseLanguage = useChooseLanguage()
  const choose = (value: string) => chooseLanguage(value as LanguagePreference)

  return (
    <SettingRow
      title={t('settings.general.language.title')}
      description={t('settings.general.language.description')}
      htmlFor="settings-language"
      control={
        <RowSelect
          id="settings-language"
          value={preference}
          onChange={choose}
          options={[
            { value: 'system', label: t('settings.general.language.system') },
            ...supportedLanguages.map((lang) => ({
              value: lang,
              label: languageNames[lang],
            })),
          ]}
        />
      }
    />
  )
}

function AppearanceRow() {
  const { t } = useI18n()
  const { preference, setPreference } = useTheme()
  return (
    <SettingRow
      title={t('settings.general.appearance.title')}
      description={t('settings.general.appearance.description')}
      htmlFor="settings-appearance"
      control={
        <RowSelect
          id="settings-appearance"
          value={preference}
          onChange={(value) => setPreference(value as ThemePreference)}
          options={(['light', 'dark', 'system'] as const).map((value) => ({
            value,
            label: t(`shell.theme.${value}`),
          }))}
        />
      }
    />
  )
}
