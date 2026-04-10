import type { BrowserProfile } from './types'

type Translate = (
  key: string,
  values?: Record<string, number | string>,
) => string

export function browserRetentionMeta(
  profile: BrowserProfile,
  t: Translate,
): {
  label: string
  body: string
} {
  if (profile.retentionBoundary.kind === 'macos-safari') {
    return {
      label: t('browserRetentionSafariLabel', {
        days: profile.retentionBoundary.localDays ?? 365,
      }),
      body: t('browserRetentionSafariBody'),
    }
  }

  return {
    label: t('browserRetentionManagedLabel'),
    body: t('browserRetentionManagedBody'),
  }
}
