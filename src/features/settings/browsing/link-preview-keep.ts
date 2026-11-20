/**
 * How long link-preview images are kept, as the choices Settings offers.
 * The backend stores a cleanup rule; these are the rules the menu can pick,
 * and anything else saved (by an older version, or by hand) reads as
 * "custom" so it is shown as is and never rewritten by opening the menu.
 *
 * Shared by the "Keep images" option and "Clean up now", whose
 * confirmation names the rule it applies.
 */
import type { OgImageSettingsConfig } from '@/lib/types'

const MB = 1024 * 1024

export type KeepChoice =
  'off' | 'days30' | 'days90' | 'size100' | 'size500' | 'custom'

export const keepRules: Record<
  Exclude<KeepChoice, 'custom'>,
  OgImageSettingsConfig['cleanup']
> = {
  off: { mode: 'off' },
  days30: { mode: 'timeTtl', max_age_days: 30 },
  days90: { mode: 'timeTtl', max_age_days: 90 },
  size100: { mode: 'lru', max_bytes: 100 * MB },
  size500: { mode: 'lru', max_bytes: 500 * MB },
}

export const keepChoices = Object.keys(keepRules) as (keyof typeof keepRules)[]

export function keepChoiceOf(
  cleanup: OgImageSettingsConfig['cleanup'],
): KeepChoice {
  const match = keepChoices.find(
    (key) => JSON.stringify(keepRules[key]) === JSON.stringify(cleanup),
  )
  return match ?? 'custom'
}
