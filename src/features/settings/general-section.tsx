/**
 * Settings → General: language, clock, appearance, how PathKeep sits on the desktop
 * (open at login, menu bar icon) and the two features that contact websites.
 */
import {
  SettingRow,
  SettingsGroup,
  SettingsSection,
} from '@/components/app/setting-row'
import {
  languageNames,
  supportedLanguages,
  useI18n,
  type ClockPreference,
} from '@/lib/i18n'
import { useChooseLanguage } from '@/lib/queries/language'
import { useTheme, type ThemePreference } from '@/lib/theme'
import {
  useWindowMaterial,
  windowMaterialAvailable,
  type WindowMaterial,
} from '@/lib/window-material'
import type { LanguagePreference } from '@/lib/types'
import { DesktopRows } from './desktop-rows'
import { OnlineRows } from './online-rows'
import { RowSelect } from './row-select'

export function GeneralSection() {
  const { t } = useI18n()
  return (
    <SettingsSection title={t('settings.nav.general')}>
      <LanguageRow />
      <ClockRow />
      <AppearanceRow />
      {windowMaterialAvailable() && <WindowMaterialRow />}
      <DesktopRows />
      <SettingsGroup
        title={t('settings.general.online.title')}
        note={t('settings.general.online.note')}
      />
      <OnlineRows />
    </SettingsSection>
  )
}

function ClockRow() {
  const { t, clock, setClock } = useI18n()
  return (
    <SettingRow
      title={t('settings.general.clock.title')}
      description={t('settings.general.clock.description')}
      htmlFor="settings-clock"
      control={
        <RowSelect
          id="settings-clock"
          value={clock}
          onChange={(value) => setClock(value as ClockPreference)}
          options={(['system', 'h12', 'h23'] as const).map((value) => ({
            value,
            label: t(`settings.general.clock.${value}`),
          }))}
        />
      }
    />
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

function WindowMaterialRow() {
  const { t } = useI18n()
  const [material, setMaterial] = useWindowMaterial()
  return (
    <SettingRow
      title={t('settings.general.material.title')}
      description={t('settings.general.material.description')}
      htmlFor="settings-window-material"
      control={
        <RowSelect
          id="settings-window-material"
          value={material}
          onChange={(value) => setMaterial(value as WindowMaterial)}
          options={(['solid', 'glass'] as const).map((value) => ({
            value,
            label: t(`settings.general.material.${value}`),
          }))}
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
