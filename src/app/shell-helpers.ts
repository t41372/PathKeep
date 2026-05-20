/**
 * Pure helper functions used by the paper-shell composition.
 *
 * ## Responsibilities
 * - Read user-shell preferences (sidebar collapsed, theme, daily epigraph
 *   index) from localStorage with a defensive try/catch.
 * - Format byte sums + "since" + "last archived" labels with i18n-aware
 *   locale handling.
 * - Provide a small URL → hostname extractor that swallows malformed URLs.
 *
 * ## Not responsible for
 * - React state or hook orchestration (lives in `shell.tsx`).
 * - i18n catalog content (callers provide `t` + `language`).
 *
 * Splitting these out of `shell.tsx` keeps the shell file focused on
 * composition while giving each helper a direct unit-test entry point
 * — the shell itself is otherwise only exercised through full-app
 * integration tests, which under-cover defensive `catch` paths.
 */

import type { StorageSummary } from '@/lib/types'

export const EPIGRAPH_POOL_SIZE = 6

export function readBoolean(
  key: string,
  fallback: boolean,
  storage: Storage | null | undefined,
): boolean {
  if (!storage) return fallback
  try {
    const raw = storage.getItem(key)
    if (raw === 'true') return true
    if (raw === 'false') return false
    return fallback
  } catch {
    return fallback
  }
}

export function readTheme(
  key: string,
  storage: Storage | null | undefined,
): 'light' | 'dark' {
  if (!storage) return 'light'
  try {
    const raw = storage.getItem(key)
    if (raw === 'dark') return 'dark'
    return 'light'
  } catch {
    return 'light'
  }
}

export function readEpigraphIndex(
  key: string,
  poolSize: number,
  storage: Storage | null | undefined,
  now: Date = new Date(),
  rng: () => number = Math.random,
): number {
  if (!storage) return 0
  try {
    const today = now.toISOString().slice(0, 10)
    const stored = storage.getItem(key)
    if (stored) {
      const [storedDate, indexString] = stored.split(':')
      if (storedDate === today) {
        const parsed = Number.parseInt(indexString ?? '', 10)
        if (!Number.isNaN(parsed)) return parsed
      }
    }
    const next = Math.floor(rng() * poolSize)
    storage.setItem(key, `${today}:${next}`)
    return next
  } catch {
    return 0
  }
}

export function extractDomain(url: string | undefined): string {
  if (!url) return ''
  try {
    return new URL(url).hostname
  } catch {
    return ''
  }
}

export function sumStorageBytes(storage: StorageSummary | undefined): number {
  if (!storage) return 0
  return (
    storage.archiveDatabaseBytes +
    storage.sourceEvidenceDatabaseBytes +
    storage.searchDatabaseBytes +
    storage.intelligenceDatabaseBytes +
    storage.manifestBytes +
    storage.snapshotBytes +
    storage.exportBytes +
    storage.stagingBytes +
    storage.quarantineBytes +
    storage.semanticSidecarBytes +
    storage.intelligenceBlobBytes
  )
}

export function humanizeBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return ''
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`
}

export function formatSinceLabel(
  isoTimestamp: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
  language: string,
): string {
  try {
    const date = new Date(isoTimestamp)
    if (Number.isNaN(date.getTime())) return ''
    const locale = language === 'system' ? undefined : language
    const month = date.toLocaleString(locale, { month: 'short' })
    return t('shell.since', { month, year: date.getFullYear() })
  } catch {
    return ''
  }
}

export function formatLastArchivedLabel(
  isoTimestamp: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
  language: string,
): string {
  try {
    const date = new Date(isoTimestamp)
    if (Number.isNaN(date.getTime())) return ''
    const locale = language === 'system' ? undefined : language
    const time = date.toLocaleTimeString(locale, {
      hour: '2-digit',
      minute: '2-digit',
    })
    return t('shell.lastArchivedAt', { time })
  } catch {
    return ''
  }
}
