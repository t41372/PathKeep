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
export const explorerPageSizeStorageKey = 'pathkeep.explorer.page-size'
export const defaultKeywordPageSize = 50
export const keywordPageSizeOptions = [25, 50, 100, 200] as const
export const semanticPageSize = 8

export type ExplorerPrefetchDirection = 'forward' | 'backward' | 'neutral'
const sensitiveQueryParamPattern =
  /\b(token|code|state|email|callbackUrl|session|otp|nonce|auth|password)=([^&\s]+)/gi
const emailPattern = /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi

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

export function parseKeywordPageSize(value: string | null) {
  const parsed = value ? Number.parseInt(value, 10) : Number.NaN
  return keywordPageSizeOptions.includes(
    parsed as (typeof keywordPageSizeOptions)[number],
  )
    ? parsed
    : defaultKeywordPageSize
}

export function loadStoredKeywordPageSize() {
  if (typeof window === 'undefined') {
    return defaultKeywordPageSize
  }

  return parseKeywordPageSize(
    window.localStorage.getItem(explorerPageSizeStorageKey),
  )
}

export function persistKeywordPageSize(pageSize: number) {
  if (typeof window === 'undefined') {
    return
  }

  const normalizedPageSize = keywordPageSizeOptions.includes(
    pageSize as (typeof keywordPageSizeOptions)[number],
  )
    ? pageSize
    : defaultKeywordPageSize

  window.localStorage.setItem(
    explorerPageSizeStorageKey,
    String(normalizedPageSize),
  )
}

/**
 * Builds the ordered page list Explorer should warm in the background.
 *
 * Direction-aware ordering matters for rapid paging because the first useful
 * page to fetch is usually the next boundary page in the direction the user is
 * already moving, not a symmetric page behind them.
 */
export function buildHistoryPrefetchPages(
  currentPage: number,
  pageCount: number,
  prefetchPages: number,
  direction: ExplorerPrefetchDirection,
) {
  if (prefetchPages <= 0 || pageCount <= 1) {
    return [] as number[]
  }

  const pages: number[] = []
  const pushForward = (offset: number) => {
    const page = currentPage + offset
    if (page <= pageCount) {
      pages.push(page)
    }
  }
  const pushBackward = (offset: number) => {
    const page = currentPage - offset
    if (page >= 1) {
      pages.push(page)
    }
  }

  if (direction === 'forward') {
    for (let offset = 1; offset <= prefetchPages; offset += 1) {
      pushForward(offset)
    }
    for (let offset = 1; offset <= prefetchPages; offset += 1) {
      pushBackward(offset)
    }
    return pages
  }

  if (direction === 'backward') {
    for (let offset = 1; offset <= prefetchPages; offset += 1) {
      pushBackward(offset)
    }
    for (let offset = 1; offset <= prefetchPages; offset += 1) {
      pushForward(offset)
    }
    return pages
  }

  for (let offset = 1; offset <= prefetchPages; offset += 1) {
    pushBackward(offset)
    pushForward(offset)
  }
  return pages
}

/**
 * Builds the stable cache key used by Explorer's lazy favicon hydration path.
 */
export function historyFaviconLookupKey(
  profileId: string,
  url: string,
  visitTime?: number | null,
) {
  return `${profileId}\n${url}\n${visitTime ?? 0}`
}

/**
 * Builds the cache key used by the og:image hydration hook. og:images are
 * page-level (no profile / visit-time scoping) so the key is just the URL.
 */
export function historyOgImageLookupKey(url: string) {
  return url
}

/**
 * Hosts whose result pages serve a knowledge-panel `<meta og:image>` that
 * does NOT identify the search page itself (Google shows the top entity's
 * brand image, Bing shows the answer-card image, etc.). The Browse row
 * must NOT show this as the page icon — that's where the "Google search
 * for 吉野家 → Yoshinoya logo" cross-contamination came from.
 *
 * Kept in sync with `vault-core::visit_taxonomy::url::is_search_engine_host`.
 */
const SERP_REGISTRABLE_DOMAINS = new Set([
  'baidu.com',
  'bing.com',
  'brave.com',
  'duckduckgo.com',
  'google.com',
  'kagi.com',
  'sogou.com',
  'so.com',
  'startpage.com',
  'yahoo.com',
  'yandex.com',
  'yandex.ru',
])

/**
 * Returns true when the URL is a search-engine result page (host on the
 * SERP list AND a query parameter present). Callers use this to suppress
 * the og:image-as-icon fallback for SERP rows, because the og:image on a
 * SERP describes the top entity, not the search page itself.
 */
export function isSearchResultUrl(url: string | null | undefined): boolean {
  if (typeof url !== 'string' || url.length === 0) return false
  try {
    const parsed = new URL(url)
    const host = parsed.hostname.toLowerCase()
    const registrable = host.split('.').slice(-2).join('.')
    if (
      !SERP_REGISTRABLE_DOMAINS.has(registrable) &&
      !host.startsWith('www.google.') &&
      host !== 'search.brave.com'
    ) {
      return false
    }
    if (parsed.searchParams.size === 0) return false
    return ['q', 'p', 'query', 'wd', 'text'].some((key) =>
      parsed.searchParams.has(key),
    )
  } catch {
    return false
  }
}

function compactMiddle(text: string, maxLength: number) {
  if (text.length <= maxLength) return text

  const head = Math.max(12, Math.floor((maxLength - 1) / 2))
  const tail = Math.max(8, maxLength - head - 1)
  return `${text.slice(0, head)}…${text.slice(-tail)}`
}

function sanitizeUrlForDisplay(rawUrl: string, maxLength: number) {
  try {
    const parsed = new URL(rawUrl)
    const hostname = parsed.hostname.replace(/^www\./, '')
    const decodedPath = decodeURIComponent(parsed.pathname)
      .replace(/\/+$/, '')
      .replace(/^$/, '/')
    const path = decodedPath === '/' ? '' : decodedPath
    return compactMiddle(`${hostname}${path}`, maxLength)
  } catch {
    return compactMiddle(rawUrl, maxLength)
  }
}

/**
 * Redacts callback URLs, token-like query params, and email-like strings before
 * they become Explorer titles or chips.
 */
export function sanitizeExplorerDisplayText(
  text: string | null | undefined,
  maxLength = 96,
) {
  if (!text?.trim()) return ''

  const withRedactedParams = text
    .trim()
    .replace(emailPattern, '…')
    .replace(sensitiveQueryParamPattern, (_match, key: string) => `${key}=…`)
    .replace(/https?:\/\/\S+/gi, (url) => sanitizeUrlForDisplay(url, 64))

  return compactMiddle(withRedactedParams, maxLength)
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
