/**
 * @file backend-preview-search.ts
 * @description Extracted browser-preview search and history helper surface for deterministic fixture behavior.
 * @module lib/backend-preview-search
 *
 * ## Responsibilities
 * - Hold the preview-only helper logic that shapes mock search and history responses.
 * - Keep cursor parsing, pagination, filtering, and synthetic search rows deterministic.
 * - Provide a single reusable module for preview search/history helpers without changing runtime semantics.
 *
 * ## Not responsible for
 * - Dispatching backend commands or owning the browser-preview facade.
 * - Mutating preview state outside the passed-in state object.
 * - Inventing new search or history behavior beyond the existing backend fixture contract.
 *
 * ## Dependencies
 * - Depends on typed front-end contracts from `./types`.
 * - Depends on `MockBackendState` from `./backend-preview-state` for fixture-backed history reads.
 * - Depends on `SearchQueryListResult` and `SearchQuerySort` from `./core-intelligence`.
 *
 * ## Performance notes
 * - These helpers are invoked on preview reads, so they should stay cheap, synchronous, and free of hidden global state.
 * - Pagination and filtering intentionally operate on the in-memory fixture surface only.
 */

import type { MockBackendState } from './backend-preview-state'
import type {
  AiSearchRequest,
  AiSearchResponse,
  HistoryQuery,
  HistoryQueryResponse,
} from './types'
import type {
  SearchQueryListResult,
  SearchQuerySort,
} from './core-intelligence'

/**
 * Preserves the preview backend's browser-kind prefix extraction for profile ids.
 *
 * This keeps profile-scoped history filters aligned with the existing fixture contract by
 * splitting on the first colon and returning the prefix when one exists.
 */
export function browserKindFromProfileId(profileId: string) {
  const separatorIndex = profileId.indexOf(':')
  return separatorIndex === -1 ? profileId : profileId.slice(0, separatorIndex)
}

/**
 * Counts unique URLs in a history result without changing the fixture's url identity rules.
 *
 * The helper intentionally treats every exact url string as the identity boundary so preview
 * totals match the backend's current mock accounting.
 */
export function uniqueUrlCount(items: HistoryQueryResponse['items']) {
  return new Set(items.map((item) => item.url)).size
}

/**
 * Produces an ISO timestamp shifted by the given offset from now for preview search rows.
 *
 * The helper deliberately uses `Date.now()` at call time so the synthetic rows stay anchored to
 * the same relative clock behavior as the original backend helper.
 */
export function coreIsoTimestamp(offsetMs: number) {
  return new Date(Date.now() + offsetMs).toISOString()
}

/**
 * Normalizes a core-intelligence profile id to the preview backend's default profile.
 *
 * Empty or whitespace-only values fall back to `chrome:Default` so the mock search surface
 * keeps a stable, deterministic profile id in browser preview mode.
 */
export function coreIntelligenceProfileId(profileId?: string | null) {
  return profileId?.trim() || 'chrome:Default'
}

/**
 * Builds the deterministic mock search query list used by the browser preview backend.
 *
 * The function preserves the original fixture's filtering, sort order, paging defaults, and
 * synthetic row shape so search consumers see the same preview data they did in backend.ts.
 */
export function buildMockSearchQueries(request?: {
  profileId?: string | null
  browserKind?: string | null
  engine?: string | null
  domain?: string | null
  query?: string | null
  sort?: SearchQuerySort
  page?: number
  pageSize?: number
}): SearchQueryListResult {
  const profileId = coreIntelligenceProfileId(request?.profileId)
  const rows = [
    {
      visitId: 101,
      profileId,
      browserKind: 'chrome',
      searchEngine: 'google',
      displayName: 'Google',
      rawQuery: 'chrome history sqlite',
      normalizedQuery: 'chrome history sqlite',
      searchedAt: coreIsoTimestamp(-1000 * 60 * 60 * 6),
      searchedAtMs: Date.now() - 1000 * 60 * 60 * 6,
      exactRepeatCount: 2,
      familyCount: 4,
      familyId: 'family-chrome-history',
      trailId: 'trail-archive-db',
      trailInitialQuery: 'chrome history sqlite',
      trailReformulationCount: 3,
    },
    {
      visitId: 102,
      profileId,
      browserKind: 'chrome',
      searchEngine: 'google',
      displayName: 'Google',
      rawQuery: 'tauri sqlite local-first',
      normalizedQuery: 'tauri sqlite local-first',
      searchedAt: coreIsoTimestamp(-1000 * 60 * 60 * 18),
      searchedAtMs: Date.now() - 1000 * 60 * 60 * 18,
      exactRepeatCount: 1,
      familyCount: 3,
      familyId: 'family-tauri-storage',
      trailId: 'trail-tauri-storage',
      trailInitialQuery: 'tauri sqlite local-first',
      trailReformulationCount: 2,
    },
    {
      visitId: 103,
      profileId,
      browserKind: 'chrome',
      searchEngine: 'bilibili',
      displayName: 'BiliBili',
      rawQuery: 'sqlite wal 教学',
      normalizedQuery: 'sqlite wal 教学',
      searchedAt: coreIsoTimestamp(-1000 * 60 * 60 * 28),
      searchedAtMs: Date.now() - 1000 * 60 * 60 * 28,
      exactRepeatCount: 1,
      familyCount: 1,
      familyId: null,
      trailId: null,
      trailInitialQuery: null,
      trailReformulationCount: null,
    },
  ].filter((row) => {
    if (request?.browserKind && row.browserKind !== request.browserKind) {
      return false
    }
    if (request?.engine && row.searchEngine !== request.engine) {
      return false
    }
    if (request?.query) {
      const needle = request.query.trim().toLowerCase()
      if (
        !row.normalizedQuery.includes(needle) &&
        !row.rawQuery.toLowerCase().includes(needle)
      ) {
        return false
      }
    }
    return true
  })

  const sorted = [...rows].sort((left, right) => {
    switch (request?.sort) {
      case 'alphabetical':
        return left.normalizedQuery.localeCompare(right.normalizedQuery)
      case 'exact-frequency':
        return (
          right.exactRepeatCount - left.exactRepeatCount ||
          right.searchedAtMs - left.searchedAtMs
        )
      case 'family-frequency':
        return (
          right.familyCount - left.familyCount ||
          right.exactRepeatCount - left.exactRepeatCount ||
          right.searchedAtMs - left.searchedAtMs
        )
      default:
        return right.searchedAtMs - left.searchedAtMs
    }
  })
  const page = request?.page ?? 0
  const pageSize = request?.pageSize ?? 20
  const start = page * pageSize

  return {
    rows: sorted.slice(start, start + pageSize),
    total: sorted.length,
    page,
    pageSize,
  }
}

/**
 * Filters mock history rows with the same preview-only semantics as the backend facade.
 *
 * The helper preserves profile, browser-kind, domain, regex, timestamp, sort, and cursor
 * behavior so preview consumers can rely on the same fixture output shape.
 */
export function filterMockHistory(
  state: MockBackendState,
  query: HistoryQuery | undefined,
): HistoryQueryResponse {
  const rawQuery = query?.q?.trim() ?? ''
  const q = rawQuery.toLowerCase()
  const domain = query?.domain?.trim().toLowerCase() ?? ''
  const profileId = query?.profileId ?? null
  const browserKind = query?.browserKind ?? null
  const startTimeMs = query?.startTimeMs ?? null
  const endTimeMs = query?.endTimeMs ?? null
  const sort = query?.sort ?? 'newest'
  const limit = Math.max(1, Math.min(query?.limit ?? 150, 1000))
  const requestedPage = Math.max(1, Math.floor(query?.page ?? 1))
  const cursor = parseMockHistoryCursor(query?.cursor)
  const regex = query?.regexMode && rawQuery ? new RegExp(rawQuery, 'i') : null

  const filteredItems = [...state.history.items]
    .filter((item) => !profileId || item.profileId === profileId)
    .filter(
      (item) =>
        !browserKind ||
        browserKindFromProfileId(item.profileId) === browserKind,
    )
    .filter(
      (item) =>
        !q ||
        (regex
          ? regex.test(item.url) || regex.test(item.title ?? '')
          : item.url.toLowerCase().includes(q) ||
            (item.title ?? '').toLowerCase().includes(q)),
    )
    .filter((item) => !domain || item.domain.toLowerCase().includes(domain))
    .filter((item) => !startTimeMs || item.visitTime >= startTimeMs)
    .filter((item) => !endTimeMs || item.visitTime <= endTimeMs)
    .sort((left, right) =>
      sort === 'oldest'
        ? left.visitTime - right.visitTime
        : right.visitTime - left.visitTime,
    )

  const pageCount = Math.max(1, Math.ceil(filteredItems.length / limit))
  const cursorStartIndex = (() => {
    if (!cursor) return 0
    const nextIndex = filteredItems.findIndex((item) => {
      if (sort === 'oldest') {
        return (
          item.visitTime > cursor.visitTime ||
          (item.visitTime === cursor.visitTime && item.id > cursor.id)
        )
      }
      return (
        item.visitTime < cursor.visitTime ||
        (item.visitTime === cursor.visitTime && item.id < cursor.id)
      )
    })
    return nextIndex === -1 ? filteredItems.length : nextIndex
  })()
  const page =
    query?.page != null
      ? Math.min(requestedPage, pageCount)
      : Math.max(1, Math.floor(cursorStartIndex / limit) + 1)
  const startIndex =
    query?.page != null ? (page - 1) * limit : Math.max(0, cursorStartIndex)
  const items = filteredItems.slice(startIndex, startIndex + limit)
  const hasNext = startIndex + limit < filteredItems.length
  const hasPrevious = startIndex > 0

  return {
    total: filteredItems.length,
    items,
    page,
    pageSize: limit,
    pageCount,
    hasPrevious,
    hasNext,
    nextCursor:
      hasNext && items.length > 0
        ? encodeMockHistoryCursor(items[items.length - 1])
        : null,
  }
}

/**
 * Parses the preview history cursor format into the same shape the backend facade expects.
 *
 * Invalid or incomplete cursor strings return `null` so pagination falls back to the normal
 * page-based path instead of throwing.
 */
export function parseMockHistoryCursor(cursor?: string | null) {
  if (!cursor) return null
  const [visitTime, id] = cursor.split('|')
  const parsedVisitTime = Number(visitTime)
  const parsedId = Number(id)
  if (!Number.isFinite(parsedVisitTime) || !Number.isFinite(parsedId)) {
    return null
  }
  return {
    visitTime: parsedVisitTime,
    id: parsedId,
  }
}

/**
 * Serializes one history item into the preview cursor string format.
 *
 * The function intentionally mirrors the old backend helper so paging round-trips keep the same
 * `visitTime|id` cursor representation.
 */
export function encodeMockHistoryCursor(
  item: HistoryQueryResponse['items'][number],
) {
  return `${item.visitTime}|${item.id}`
}

/**
 * Paginates the preview AI search surface using the same lexical fallback contract as before.
 *
 * The helper remains intentionally simple: it walks the in-memory history fixture, assigns a
 * deterministic score curve, and advances by cursor offset only.
 */
export function paginateMockAiSearch(
  state: MockBackendState,
  request?: AiSearchRequest,
): AiSearchResponse {
  const limit = Math.max(1, Math.min(request?.limit ?? 24, 50))
  const offset = Math.max(0, Number.parseInt(request?.cursor ?? '0', 10) || 0)
  const items = state.history.items.map((item, index) => ({
    historyId: item.id,
    profileId: item.profileId,
    url: item.url,
    title: item.title,
    domain: item.domain,
    visitedAt: item.visitedAt,
    score: 0.8 - index * 0.1,
    matchReason: 'Browser preview lexical fixture',
  }))
  const pagedItems = items.slice(offset, offset + limit)
  const nextOffset = offset + pagedItems.length

  return {
    total: items.length,
    providerId: 'lexical-fallback',
    model: 'none',
    items: pagedItems,
    notes: ['Semantic retrieval is unavailable in browser preview mode.'],
    nextCursor: nextOffset < items.length ? String(nextOffset) : null,
  }
}
