/**
 * @file url-state-derivations.ts
 * @description Pure derivation owner for Explorer URL-state parsing, signatures, and filter summaries.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Parse raw `URLSearchParams` into typed Explorer URL-state fields.
 * - Build deterministic query payloads, signatures, filter chips, and grouped date windows.
 * - Keep date-shortcut and browser-kind derivation out of the React hook so the hook can focus on Router state and effects.
 *
 * ## Not responsible for
 * - Owning React state, effects, or debounced query commits.
 * - Writing to `URLSearchParams` or local storage.
 * - Rendering Explorer UI.
 *
 * ## Dependencies
 * - Depends on Explorer helper utilities for page-size parsing, date shortcuts, and local-date formatting.
 * - Depends on translator callbacks only for user-visible filter/recent-search labels.
 *
 * ## Performance notes
 * - Pure synchronous helpers keep Explorer URL-state recalculation cheap during typing and pagination.
 */

import {
  dateShortcutWindows,
  endOfDayMs,
  parseKeywordPageSize,
  semanticPageSize,
  toLocalDateString,
} from './helpers'
import type {
  ExplorerMode,
  ExplorerSortMode,
  ExplorerViewMode,
  RecentSearchEntry,
  Translator,
} from './types'

/**
 * Rendered active-filter chip descriptor for the Explorer query shell.
 */
export interface ActiveFilter {
  id: string
  label: string
  value: string
}

/**
 * Typed snapshot of the raw Explorer query params after normalization.
 */
export interface ExplorerUrlParamState {
  browserKind: string | null
  cursor: string | null
  domain: string | null
  end: string | null
  explicitPage: number | null
  explicitProfileId: string | null
  mode: ExplorerMode
  pageSize: number
  profileId: string | null
  rawQuery: string
  regexMode: boolean
  semanticCursor: string | null
  sort: ExplorerSortMode
  start: string | null
  view: ExplorerViewMode
}

/**
 * Normalizes Explorer URL params into one typed state object so the hook does
 * not have to duplicate parsing logic in multiple `useMemo` branches.
 */
export function deriveExplorerUrlParamState(
  searchParams: URLSearchParams,
  activeProfileId: string | null,
): ExplorerUrlParamState {
  const rawQuery = searchParams.get('q') ?? ''
  const regexMode = searchParams.get('regex') === '1'
  const mode = (searchParams.get('mode') as ExplorerMode | null) ?? 'keyword'
  const requestedView = searchParams.get('view')
  const view: ExplorerViewMode =
    requestedView === 'session' || requestedView === 'trail'
      ? mode === 'keyword'
        ? requestedView
        : 'time'
      : 'time'
  const explicitProfileId = searchParams.get('profileId')
  const profileId = explicitProfileId ?? activeProfileId
  const browserKind = searchParams.get('browserKind')
  const domain = searchParams.get('domain')
  const start = searchParams.get('start')
  const end = searchParams.get('end')
  const explicitSort = searchParams.get('sort')
  const sort =
    explicitSort === 'relevance' ||
    explicitSort === 'newest' ||
    explicitSort === 'oldest'
      ? explicitSort
      : rawQuery.trim() && !regexMode && mode === 'keyword'
        ? 'relevance'
        : 'newest'
  const rawPage = searchParams.get('page')
  const parsedPage = rawPage ? Number.parseInt(rawPage, 10) : Number.NaN
  const explicitPage =
    Number.isFinite(parsedPage) && parsedPage > 0 ? parsedPage : null

  return {
    browserKind,
    cursor: searchParams.get('cursor'),
    domain,
    end,
    explicitPage,
    explicitProfileId,
    mode,
    pageSize: parseKeywordPageSize(searchParams.get('pageSize')),
    profileId,
    rawQuery,
    regexMode,
    semanticCursor: searchParams.get('semanticCursor'),
    sort,
    start,
    view,
  }
}

/**
 * Builds the canonical history query payload from the normalized Explorer URL state.
 */
export function buildExplorerHistoryQuery(args: {
  browserKind: string | null
  cursor: string | null
  deferredQuery: string
  domain: string | null
  end: string | null
  explicitPage: number | null
  pageSize: number
  profileId: string | null
  regexMode: boolean
  sort: ExplorerSortMode
  start: string | null
}) {
  return {
    q: args.deferredQuery || null,
    profileId: args.profileId,
    browserKind: args.browserKind,
    domain: args.domain,
    startTimeMs: args.start
      ? new Date(`${args.start}T00:00:00.000`).getTime()
      : null,
    endTimeMs: args.end ? endOfDayMs(args.end) : null,
    sort: args.sort,
    limit: args.pageSize,
    page: args.explicitPage,
    cursor: args.explicitPage ? null : args.cursor,
    regexMode: args.regexMode,
  }
}

/**
 * Builds the semantic query payload from the normalized Explorer URL state.
 */
export function buildExplorerSemanticQuery(args: {
  deferredQuery: string
  domain: string | null
  profileId: string | null
  semanticCursor: string | null
}) {
  return {
    query: args.deferredQuery.trim(),
    profileId: args.profileId,
    domain: args.domain,
    limit: semanticPageSize,
    cursor: args.semanticCursor,
  }
}

/**
 * Returns the stable history-query signature used to reset staged pagination
 * only when query-relevant inputs actually changed.
 */
export function buildExplorerHistoryQuerySignature(args: {
  browserKind: string | null
  domain: string | null
  end: string | null
  mode: ExplorerMode
  pageSize: number
  profileId: string | null
  rawQuery: string
  regexMode: boolean
  sort: ExplorerSortMode
  start: string | null
  view: ExplorerViewMode
}) {
  return JSON.stringify({
    q: args.rawQuery || null,
    view: args.view,
    profileId: args.profileId,
    browserKind: args.browserKind,
    domain: args.domain,
    start: args.start,
    end: args.end,
    sort: args.sort,
    pageSize: args.pageSize,
    regexMode: args.regexMode,
    mode: args.mode,
  })
}

/**
 * Returns the stable semantic-query signature used to manage cursor trails
 * without leaking stale previous-page state across semantic searches.
 */
export function buildExplorerSemanticQuerySignature(args: {
  deferredQuery: string
  domain: string | null
  mode: ExplorerMode
  profileId: string | null
}) {
  return JSON.stringify({
    query: args.deferredQuery.trim(),
    profileId: args.profileId,
    domain: args.domain,
    mode: args.mode,
  })
}

/**
 * Builds the browser-kind filter options from the selected profile ids.
 */
export function buildExplorerBrowserKinds(selectedProfileIds: string[]) {
  return Array.from(
    new Set(selectedProfileIds.map((profile) => profile.split(':')[0])),
  )
}

/**
 * Builds the active-filter chip list shown above the Explorer query controls.
 */
export function buildExplorerActiveFilters(args: {
  browserLabelForKind?: (kind: string) => string
  explorerT: Translator
  mode: ExplorerMode
  profileLabelForId?: (profileId: string) => string
  regexMode: boolean
  searchParams: URLSearchParams
  view: ExplorerViewMode
  start: string | null
  end: string | null
}) {
  const profileLabelForId =
    args.profileLabelForId ?? ((profileId: string) => profileId)
  const browserLabelForKind =
    args.browserLabelForKind ?? ((kind: string) => kind)

  return [
    args.searchParams.get('q')
      ? {
          id: 'q',
          label: args.explorerT('filterKeyword'),
          value: args.searchParams.get('q') as string,
        }
      : null,
    args.mode !== 'keyword'
      ? {
          id: 'mode',
          label: args.explorerT('activeFilterMode'),
          value:
            args.mode === 'semantic'
              ? args.explorerT('modeSemantic')
              : args.explorerT('modeHybrid'),
        }
      : null,
    args.view !== 'time'
      ? {
          id: 'view',
          label: args.explorerT('viewModeLabel'),
          value:
            args.view === 'session'
              ? args.explorerT('viewModeSession')
              : args.explorerT('viewModeTrail'),
        }
      : null,
    args.regexMode
      ? {
          id: 'regex',
          label: args.explorerT('activeFilterRegex'),
          value: args.explorerT('activeFilterRegexEnabled'),
        }
      : null,
    args.searchParams.get('domain')
      ? {
          id: 'domain',
          label: args.explorerT('filterDomain'),
          value: args.searchParams.get('domain') as string,
        }
      : null,
    args.searchParams.get('profileId')
      ? {
          id: 'profileId',
          label: args.explorerT('filterProfile'),
          value: profileLabelForId(
            args.searchParams.get('profileId') as string,
          ),
        }
      : null,
    args.searchParams.get('browserKind')
      ? {
          id: 'browserKind',
          label: args.explorerT('filterBrowser'),
          value: browserLabelForKind(
            args.searchParams.get('browserKind') as string,
          ),
        }
      : null,
    args.start
      ? {
          id: 'start',
          label: args.explorerT('filterStart'),
          value: args.start,
        }
      : null,
    args.end
      ? {
          id: 'end',
          label: args.explorerT('filterEnd'),
          value: args.end,
        }
      : null,
  ].filter((value): value is ActiveFilter => Boolean(value))
}

/**
 * Formats the user-visible recent-search label so storage persistence and
 * rendered chips stay aligned.
 *
 * `params.profileId` and `params.browserKind` are treated as already-labeled
 * display values when provided.
 */
export function buildExplorerRecentSearchLabel(args: {
  explorerT: Translator
  formatRecentDate: (value?: string | null) => string | null
  params: RecentSearchEntry['params']
}) {
  const { explorerT, formatRecentDate, params } = args
  return [
    params.mode === 'semantic'
      ? explorerT('modeSemantic')
      : params.mode === 'hybrid'
        ? explorerT('modeHybrid')
        : null,
    params.view === 'session'
      ? explorerT('viewModeSession')
      : params.view === 'trail'
        ? explorerT('viewModeTrail')
        : null,
    params.regex === '1' ? explorerT('activeFilterRegexEnabled') : null,
    params.q?.trim() ? params.q.trim() : null,
    params.domain?.trim() ? params.domain.trim() : null,
    params.profileId ? params.profileId : null,
    params.browserKind ? params.browserKind : null,
    params.start || params.end
      ? [
          formatRecentDate(params.start) ?? explorerT('allRecordedTime'),
          formatRecentDate(params.end) ?? explorerT('allRecordedTime'),
        ].join(' - ')
      : null,
  ]
    .filter(Boolean)
    .join(' · ')
}

/**
 * Resolves the grouped-view fallback date window when the user has not pinned
 * an explicit start/end range.
 */
export function resolveExplorerGroupedDateRange(
  start: string | null,
  end: string | null,
  today = new Date(),
) {
  if (start && end) {
    return { start, end }
  }

  const endDate = new Date(today)
  const startDate = new Date(today)
  startDate.setDate(endDate.getDate() - 29)
  return {
    start: toLocalDateString(startDate),
    end: toLocalDateString(endDate),
  }
}

/**
 * Resolves which date shortcut is currently active for the Explorer timeline strip.
 */
export function resolveExplorerActiveDateShortcut(
  start: string | null,
  end: string | null,
  today = new Date(),
) {
  if (!start || !end) return null

  const endString = toLocalDateString(today)
  if (end !== endString) return null

  return (
    dateShortcutWindows.find((entry) => {
      const startDate = new Date(today)
      startDate.setDate(today.getDate() - (entry.days - 1))
      return start === toLocalDateString(startDate)
    })?.key ?? null
  )
}
