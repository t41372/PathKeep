/**
 * This module formats timestamps, bytes, and durations into the human-readable evidence strings used across the shell.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `calendarDayKey`
 * - `formatDateTime`
 * - `formatDuration`
 * - `formatBytes`
 * - `formatRelativeTime`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { localeTag, type ResolvedLanguage } from './i18n'

/**
 * Explains how date part works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function datePart(parts: Intl.DateTimeFormatPart[], type: 'day' | 'month') {
  return parts.find((part) => part.type === type)?.value ?? null
}

/**
 * Explains how calendar day key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function calendarDayKey(
  value: Date | string | null | undefined,
  timeZone?: string,
) {
  if (!value) {
    return null
  }

  const date = value instanceof Date ? value : new Date(value)
  if (Number.isNaN(date.getTime())) {
    return null
  }

  const formatter = new Intl.DateTimeFormat('en-CA', {
    day: '2-digit',
    month: '2-digit',
    timeZone,
  })
  const parts = formatter.formatToParts(date)
  const month = datePart(parts, 'month')
  const day = datePart(parts, 'day')

  if (!month || !day) {
    return null
  }

  return `${month}-${day}`
}

/**
 * Formats date time for display.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function formatDateTime(
  value: string | null | undefined,
  language: ResolvedLanguage,
) {
  if (!value) {
    return null
  }

  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return null
  }

  return new Intl.DateTimeFormat(localeTag(language), {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date)
}

/**
 * Formats duration for display.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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

/**
 * Formats bytes for display.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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

/**
 * Formats relative time for display.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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
