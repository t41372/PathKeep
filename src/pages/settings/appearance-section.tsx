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

import { useState } from 'react'
import { useI18n } from '@/lib/i18n'
import {
  PaperCard,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { applyPaperPreferences, type PaperPreferences } from '@/lib/paper-preferences'
import { cn } from '@/lib/cn'

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

export function AppearanceSection({ anchorId = 'appearance' }: AppearanceSectionProps) {
  const { t } = useI18n()
  const [prefs, setPrefs] = useState<PaperPreferences>(() =>
    applyPaperPreferences(null),
  )

  const update = (patch: Partial<PaperPreferences>) => {
    setPrefs((current) => applyPaperPreferences({ ...current, ...patch }))
  }

  return (
    <PaperCard testId="settings-appearance-section">
      <span id={anchorId} aria-hidden />
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

interface FieldProps {
  label: string
  help?: string
  children: React.ReactNode
}

function Field({ label, help, children }: FieldProps) {
  return (
    <div className="border-border-light py-3 first:pt-0 last:pb-0 last:border-b-0 border-b">
      <div className="mb-2 font-mono text-[10px] uppercase tracking-[0.08em] text-ink-faint">
        {label}
      </div>
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          {help ? (
            <p className="m-0 mb-2 font-sans text-[12px] text-ink-muted">
              {help}
            </p>
          ) : null}
          {children}
        </div>
      </div>
    </div>
  )
}

interface SegmentedControlProps<Id extends string> {
  options: Array<{ id: Id; label: string; hint?: string }>
  value: Id
  onChange: (id: Id) => void
  stacked?: boolean
}

function SegmentedControl<Id extends string>({
  options,
  value,
  onChange,
  stacked = false,
}: SegmentedControlProps<Id>) {
  return (
    <div
      className={cn(
        'flex gap-2',
        stacked ? 'flex-col items-stretch' : 'flex-row items-center',
      )}
      role="radiogroup"
    >
      {options.map((option) => (
        <button
          key={option.id}
          type="button"
          role="radio"
          aria-checked={option.id === value}
          onClick={() => onChange(option.id)}
          className={cn(
            'border-border-default flex items-start gap-2 border px-3 py-2 text-left transition-colors rounded-paper',
            option.id === value
              ? 'border-accent bg-accent-soft text-accent-text'
              : 'text-ink hover:border-ink-muted hover:bg-hover',
          )}
        >
          <span className="flex flex-col">
            <span className="font-sans text-[12.5px] font-medium">
              {option.label}
            </span>
            {option.hint ? (
              <span className="mt-0.5 font-mono text-[10px] text-ink-faint">
                {option.hint}
              </span>
            ) : null}
          </span>
        </button>
      ))}
    </div>
  )
}

interface ToggleProps {
  value: boolean
  onChange: (value: boolean) => void
  onLabel: string
  offLabel: string
}

function Toggle({ value, onChange, onLabel, offLabel }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={value}
      onClick={() => onChange(!value)}
      className={cn(
        'border-border-default inline-flex items-center gap-3 border px-3 py-1.5 text-[12px] transition-colors rounded-paper',
        value
          ? 'border-accent bg-accent-soft text-accent-text'
          : 'text-ink-muted hover:border-ink-muted hover:bg-hover',
      )}
    >
      <span
        className={cn(
          'inline-block h-3 w-3 rounded-full',
          value ? 'bg-accent' : 'bg-ink-faint',
        )}
      />
      {value ? onLabel : offLabel}
    </button>
  )
}
