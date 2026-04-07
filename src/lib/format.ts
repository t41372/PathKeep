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

export function formatBytes(
  value: number | null | undefined,
  language: ResolvedLanguage = 'en',
) {
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
      ? new Intl.NumberFormat(localeTag(language), {
          maximumFractionDigits: 0,
        }).format(size)
      : new Intl.NumberFormat(localeTag(language), {
          maximumFractionDigits: 1,
          minimumFractionDigits: 0,
        }).format(Number(size.toFixed(1)))
  return `${rounded} ${units[unitIndex]}`
}

export function formatRelativeTime(
  value: string | null | undefined,
  language: ResolvedLanguage = 'en',
) {
  if (!value) {
    return language === 'zh-CN'
      ? '尚未发生'
      : language === 'zh-TW'
        ? '尚未發生'
        : 'Not yet'
  }

  const timestamp = new Date(value).getTime()
  if (Number.isNaN(timestamp)) {
    return value
  }

  const diffMs = timestamp - Date.now()
  const absMinutes = Math.round(Math.abs(diffMs) / 60_000)

  if (absMinutes < 1) {
    return new Intl.RelativeTimeFormat(localeTag(language), {
      numeric: 'auto',
    }).format(0, 'second')
  }

  const formatter = new Intl.RelativeTimeFormat(localeTag(language), {
    numeric: 'auto',
  })

  if (absMinutes < 60) {
    return formatter.format(diffMs >= 0 ? absMinutes : -absMinutes, 'minute')
  }

  const absHours = Math.round(absMinutes / 60)
  if (absHours < 48) {
    return formatter.format(diffMs >= 0 ? absHours : -absHours, 'hour')
  }

  const absDays = Math.round(absHours / 24)
  return formatter.format(diffMs >= 0 ? absDays : -absDays, 'day')
}
