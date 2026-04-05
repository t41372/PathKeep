import { localeTag, type ResolvedLanguage } from './i18n'

export function formatDateTime(
  value: string | null | undefined,
  language: ResolvedLanguage,
) {
  if (!value) {
    return null
  }

  return new Intl.DateTimeFormat(localeTag(language), {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(value))
}

export function formatDuration(durationMs: number | null | undefined) {
  if (!durationMs || durationMs <= 0) {
    return '0s'
  }

  const totalSeconds = Math.round(durationMs / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  if (minutes === 0) {
    return `${seconds}s`
  }
  return `${minutes}m ${seconds}s`
}
