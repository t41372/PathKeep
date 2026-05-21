/**
 * Settings → Appearance section (paper redesign).
 *
 * Exposes the four shell-wide visual preferences the user asked for:
 * - Theme (light / dark)
 * - Font family (bundled Newsreader + JetBrains Mono OR system fallback)
 * - Density (comfortable / compact)
 * - Paper texture (noise + vignette opacity on/off)
 *
 * Persistence:
 * - Theme is already persisted by the shell (`pathkeep.theme`). This card
 *   exposes the same toggle so it's discoverable from Settings.
 * - Font, density, and paper-texture preferences live in localStorage under
 *   `pathkeep.fonts`, `pathkeep.density`, and `pathkeep.paperTexture`. The
 *   shell reads them on mount via `applyPaperPreferences()` so future routes
 *   see the right state.
 */

import { useEffect, useState } from 'react'
import { useI18n } from '@/lib/i18n'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import {
  PAPER_PREFERENCES_EVENT,
  applyPaperPreferences,
  type PaperPreferences,
  type PaperPreferencesEventDetail,
} from '@/lib/paper-preferences'
import { Field, SegmentedControl, Toggle } from './paper-form-primitives'

export interface AppearanceSectionProps {
  anchorId?: string
}

const FONT_OPTIONS: Array<{
  id: PaperPreferences['fonts']
  labelKey: string
  hintKey: string
}> = [
  {
    id: 'bundled',
    labelKey: 'settings.appearanceFontBundled',
    hintKey: 'settings.appearanceFontBundledHint',
  },
  {
    id: 'system',
    labelKey: 'settings.appearanceFontSystem',
    hintKey: 'settings.appearanceFontSystemHint',
  },
]

const DENSITY_OPTIONS: Array<{
  id: PaperPreferences['density']
  labelKey: string
}> = [
  { id: 'comfortable', labelKey: 'settings.appearanceDensityComfortable' },
  { id: 'compact', labelKey: 'settings.appearanceDensityCompact' },
]

const THEME_OPTIONS: Array<{
  id: PaperPreferences['theme']
  labelKey: string
}> = [
  { id: 'light', labelKey: 'settings.appearanceThemeLight' },
  { id: 'dark', labelKey: 'settings.appearanceThemeDark' },
]

export function AppearanceSection({
  anchorId = 'appearance',
}: AppearanceSectionProps) {
  const { t } = useI18n()
  const [prefs, setPrefs] = useState<PaperPreferences>(() =>
    applyPaperPreferences(null),
  )

  // Listen for preference mutations from peers (e.g. the shell's theme
  // toggle button) so this card stays in sync without holding a stale
  // mirror.
  useEffect(() => {
    function handle(event: Event) {
      const detail = (event as CustomEvent<PaperPreferencesEventDetail>).detail
      if (detail?.preferences) setPrefs(detail.preferences)
    }
    window.addEventListener(PAPER_PREFERENCES_EVENT, handle)
    return () => window.removeEventListener(PAPER_PREFERENCES_EVENT, handle)
  }, [])

  const update = (patch: Partial<PaperPreferences>) => {
    setPrefs((current) => applyPaperPreferences({ ...current, ...patch }))
  }

  return (
    <PaperCard testId="settings-appearance-section" id={anchorId}>
      <PaperCardHeader title={t('settings.appearanceTitle')} />
      <PaperCardBody>
        <p className="m-0 mb-4 font-serif text-[13.5px] italic leading-[1.55] text-ink-muted">
          {t('settings.appearanceIntro')}
        </p>

        <Field label={t('settings.appearanceTheme')}>
          <SegmentedControl
            options={THEME_OPTIONS.map((option) => ({
              id: option.id,
              label: t(option.labelKey),
            }))}
            value={prefs.theme}
            onChange={(theme) => update({ theme })}
          />
        </Field>

        <Field
          label={t('settings.appearanceFonts')}
          help={t('settings.appearanceFontsHelp')}
        >
          <SegmentedControl
            options={FONT_OPTIONS.map((option) => ({
              id: option.id,
              label: t(option.labelKey),
              hint: t(option.hintKey),
            }))}
            value={prefs.fonts}
            onChange={(fonts) => update({ fonts })}
            stacked
          />
        </Field>

        <Field label={t('settings.appearanceDensity')}>
          <SegmentedControl
            options={DENSITY_OPTIONS.map((option) => ({
              id: option.id,
              label: t(option.labelKey),
            }))}
            value={prefs.density}
            onChange={(density) => update({ density })}
          />
        </Field>

        <Field
          label={t('settings.appearancePaperTexture')}
          help={t('settings.appearancePaperTextureHelp')}
        >
          <Toggle
            value={prefs.paperTexture}
            onChange={(paperTexture) => update({ paperTexture })}
            onLabel={t('settings.appearancePaperOn')}
            offLabel={t('settings.appearancePaperOff')}
          />
        </Field>
      </PaperCardBody>
    </PaperCard>
  )
}
