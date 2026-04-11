/**
 * This module renders the History Explorer route and keeps the keyword-first, deep-linkable recall workflow honest even when optional AI features degrade.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `recentSearchesStorageKey`
 * - `keywordPageSize`
 * - `semanticPageSize`
 * - `dateShortcutWindows`
 * - `endOfDayMs`
 * - `toLocalDateString`
 * - `isRecentSearchEntry`
 * - `loadRecentSearches`
 * - `browserLabel`
 * - `activateRecordSelection`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

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

/**
 * Explains how end of day ms works.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function endOfDayMs(value: string) {
  const timestamp = new Date(`${value}T23:59:59.999`).getTime()
  return Number.isNaN(timestamp) ? null : timestamp
}

/**
 * Explains how to local date string works.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function toLocalDateString(value: Date) {
  return value.toLocaleDateString('en-CA')
}

/**
 * Returns whether recent search entry.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
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

/**
 * Loads recent searches.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
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

/**
 * Explains how browser label works.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function browserLabel(kind: string) {
  if (kind === 'chrome') return 'Chrome'
  if (kind === 'arc') return 'Arc'
  if (kind === 'firefox') return 'Firefox'
  if (kind === 'safari') return 'Safari'
  return kind
}

/**
 * Explains how activate record selection works.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
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
