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

export function formatBytes(value: number | null | undefined) {
  if (!value || value <= 0) {
    return '0 B'
  }

  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = value
  let unitIndex = 0

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024
    unitIndex += 1
  }

  const rounded =
    size >= 10 || unitIndex === 0
      ? Math.round(size).toString()
      : Number(size.toFixed(1)).toString()
  return `${rounded} ${units[unitIndex]}`
}

export function formatRelativeTime(value: string | null | undefined) {
  if (!value) {
    return 'Not yet'
  }

  const timestamp = new Date(value).getTime()
  if (Number.isNaN(timestamp)) {
    return value
  }

  const diffMs = timestamp - Date.now()
  const absMinutes = Math.round(Math.abs(diffMs) / 60_000)

  if (absMinutes < 1) {
    return 'Just now'
  }

  if (absMinutes < 60) {
    return `${absMinutes}m ${diffMs >= 0 ? 'from now' : 'ago'}`
  }

  const absHours = Math.round(absMinutes / 60)
  if (absHours < 48) {
    return `${absHours}h ${diffMs >= 0 ? 'from now' : 'ago'}`
  }

  const absDays = Math.round(absHours / 24)
  return `${absDays}d ${diffMs >= 0 ? 'from now' : 'ago'}`
}
