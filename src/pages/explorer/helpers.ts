import type { KeyboardEvent } from 'react'
import type { RecentSearchEntry } from './types'

export const recentSearchesStorageKey = 'pathkeep.explorer.recent-searches'
export const keywordPageSize = 50
export const semanticPageSize = 8

export const dateShortcutWindows = [
  { key: 'day', days: 1, labelKey: 'shortcutDay' },
  { key: 'week', days: 7, labelKey: 'shortcutWeek' },
  { key: 'month', days: 30, labelKey: 'shortcutMonth' },
  { key: 'year', days: 365, labelKey: 'shortcutYear' },
] as const

export function endOfDayMs(value: string) {
  const timestamp = new Date(`${value}T23:59:59.999`).getTime()
  return Number.isNaN(timestamp) ? null : timestamp
}

export function toLocalDateString(value: Date) {
  return value.toLocaleDateString('en-CA')
}

export function isRecentSearchEntry(
  value: unknown,
): value is RecentSearchEntry {
  return (
    typeof value === 'object' &&
    value !== null &&
    'params' in value &&
    typeof value.params === 'object' &&
    value.params !== null
  )
}

export function loadRecentSearches() {
  if (typeof window === 'undefined') return [] as RecentSearchEntry[]
  const raw = window.localStorage.getItem(recentSearchesStorageKey)
  if (!raw) return []
  try {
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []

    return parsed
      .map((entry) => {
        if (typeof entry === 'string') {
          return {
            label: entry,
            params: { q: entry, sort: 'newest' as const },
          }
        }

        if (isRecentSearchEntry(entry)) {
          return entry
        }

        return null
      })
      .filter((entry): entry is RecentSearchEntry => entry !== null)
  } catch {
    return []
  }
}

export function browserLabel(kind: string) {
  if (kind === 'chrome') return 'Chrome'
  if (kind === 'arc') return 'Arc'
  if (kind === 'firefox') return 'Firefox'
  if (kind === 'safari') return 'Safari'
  return kind
}

export function activateRecordSelection(
  event: KeyboardEvent<HTMLDivElement>,
  onSelect: () => void,
) {
  if (event.key !== 'Enter' && event.key !== ' ') {
    return
  }

  event.preventDefault()
  onSelect()
}
