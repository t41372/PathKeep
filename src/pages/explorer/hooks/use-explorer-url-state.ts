/**
 * This module contains route-level hooks that support the Explorer surface.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `useExplorerUrlState`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import {
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useState,
} from 'react'
import { useSearchParams } from 'react-router-dom'
import { localeTag, type ResolvedLanguage } from '../../../lib/i18n'
import {
  profileIdLabel,
  type ProfileScopeValue,
} from '../../../lib/profile-scope-context'
import {
  browserLabel,
  defaultKeywordPageSize,
  keywordPageSizeOptions,
  loadRecentSearches,
  loadStoredKeywordPageSize,
  persistKeywordPageSize,
  recentSearchesStorageKey,
  toLocalDateString,
} from '../helpers'
import type { ExplorerViewMode, RecentSearchEntry, Translator } from '../types'
import {
  buildExplorerActiveFilters,
  buildExplorerBrowserKinds,
  buildExplorerHistoryQuery,
  buildExplorerHistoryQuerySignature,
  buildExplorerRecentSearchLabel,
  buildExplorerSemanticQuery,
  buildExplorerSemanticQuerySignature,
  deriveExplorerUrlParamState,
  resolveExplorerActiveDateShortcut,
  resolveExplorerGroupedDateRange,
} from '../url-state-derivations'
import { isRustRegexCompatible } from '../regex-validation'

/**
 * Collects the inputs needed by `UseExplorerUrlState`.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface UseExplorerUrlStateOptions {
  activeProfileId: ProfileScopeValue['activeProfileId']
  explorerT: Translator
  language: ResolvedLanguage
  selectedProfileIds: string[]
}

function explorerProfileLabel(profileId: string) {
  const browserKind = profileId.split(':')[0]
  const profileLabel = profileIdLabel(profileId)
  return `${browserLabel(browserKind)} · ${profileLabel}`
}

/**
 * Provides the `useExplorerUrlState` hook.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function useExplorerUrlState({
  activeProfileId,
  explorerT,
  language,
  selectedProfileIds,
}: UseExplorerUrlStateOptions) {
  const [searchParams, setSearchParams] = useSearchParams()
  const [storedPageSize, setStoredPageSize] = useState(
    loadStoredKeywordPageSize,
  )
  const [recentSearches, setRecentSearches] =
    useState<RecentSearchEntry[]>(loadRecentSearches)
  const {
    browserKind,
    cursor,
    domain,
    end,
    explicitPage,
    explicitProfileId,
    mode,
    pageSize: routePageSize,
    profileId,
    rawQuery,
    regexMode,
    semanticCursor,
    sort,
    start,
    view,
  } = deriveExplorerUrlParamState(searchParams, activeProfileId)
  const pageSize = searchParams.get('pageSize') ? routePageSize : storedPageSize
  const [queryInput, setQueryInput] = useState(rawQuery)
  const deferredQuery = useDeferredValue(rawQuery)
  const [semanticCursorTrail, setSemanticCursorTrail] = useState<
    Record<string, string[]>
  >({})
  const [historyPageInput, setHistoryPageInput] = useState('1')

  const regexValid = useMemo(() => {
    if (!regexMode || !queryInput.trim()) return true
    return isRustRegexCompatible(queryInput)
  }, [queryInput, regexMode])

  const formatRecentDate = useCallback(
    (value?: string | null) => {
      if (!value) return null
      const parsed = new Date(`${value}T00:00:00`)
      if (Number.isNaN(parsed.getTime())) return value
      return parsed.toLocaleDateString(localeTag(language), {
        month: 'short',
        day: 'numeric',
      })
    },
    [language],
  )

  const buildRecentSearchLabel = useCallback(
    (params: RecentSearchEntry['params']) => {
      return buildExplorerRecentSearchLabel({
        explorerT,
        formatRecentDate,
        params: {
          ...params,
          profileId: params.profileId
            ? explorerProfileLabel(params.profileId)
            : null,
          browserKind: params.browserKind
            ? browserLabel(params.browserKind)
            : null,
        },
      })
    },
    [explorerT, formatRecentDate],
  )

  const persistRecentSearch = useCallback(
    (params: RecentSearchEntry['params']) => {
      const label = buildRecentSearchLabel(params)
      if (!label) return

      const nextEntry: RecentSearchEntry = {
        label,
        params: {
          ...params,
          sort: params.sort ?? 'newest',
        },
      }
      const next = [
        nextEntry,
        ...loadRecentSearches().filter(
          (entry) =>
            JSON.stringify(entry.params) !== JSON.stringify(nextEntry.params),
        ),
      ].slice(0, 4)
      window.localStorage.setItem(
        recentSearchesStorageKey,
        JSON.stringify(next),
      )
    },
    [buildRecentSearchLabel],
  )

  const currentQuery = useMemo(
    () =>
      buildExplorerHistoryQuery({
        browserKind,
        cursor,
        deferredQuery,
        domain,
        end,
        explicitPage,
        pageSize,
        profileId,
        regexMode,
        sort,
        start,
      }),
    [
      browserKind,
      cursor,
      deferredQuery,
      domain,
      end,
      explicitPage,
      pageSize,
      profileId,
      regexMode,
      sort,
      start,
    ],
  )
  const semanticQuery = useMemo(
    () =>
      buildExplorerSemanticQuery({
        deferredQuery,
        domain,
        profileId,
        semanticCursor,
      }),
    [deferredQuery, domain, profileId, semanticCursor],
  )
  const historyQuerySignature = useMemo(
    () =>
      buildExplorerHistoryQuerySignature({
        browserKind,
        domain,
        end,
        mode,
        pageSize,
        profileId,
        rawQuery,
        regexMode,
        sort,
        start,
        view,
      }),
    [
      browserKind,
      domain,
      end,
      mode,
      pageSize,
      profileId,
      rawQuery,
      regexMode,
      sort,
      start,
      view,
    ],
  )
  const semanticQuerySignature = useMemo(
    () =>
      buildExplorerSemanticQuerySignature({
        deferredQuery,
        domain,
        mode,
        profileId,
      }),
    [deferredQuery, domain, mode, profileId],
  )
  const semanticTrail = semanticCursorTrail[semanticQuerySignature] ?? []

  // Sync the local draft to the URL whenever the submitted query (`rawQuery`)
  // changes — on an explicit submit, a recent-search/deep-link navigation, or a
  // `See in context` jump that clears `q`. This is the ONLY coupling between the
  // URL query and the input; typing never writes `q` (the route's `onSubmit` is
  // the single writer), so the backend is only ever hit on an explicit submit.
  useEffect(() => {
    setQueryInput(rawQuery)
  }, [rawQuery])

  const browserKinds = buildExplorerBrowserKinds(selectedProfileIds)
  const activeFilters = buildExplorerActiveFilters({
    browserLabelForKind: browserLabel,
    end,
    explorerT,
    mode,
    profileLabelForId: explorerProfileLabel,
    regexMode,
    searchParams,
    start,
    view,
  })

  /**
   * Explains how reset semantic pagination works.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function resetSemanticPagination() {
    setSemanticCursorTrail((current) => ({
      ...current,
      [semanticQuerySignature]: [],
    }))
  }

  /**
   * Explains how clear all filters works.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function clearAllFilters() {
    setSearchParams(new URLSearchParams())
  }

  /**
   * Explains how update param works.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function updateParam(
    key: string,
    value: string | null,
    options?: { resetPagination?: boolean },
  ) {
    const resetPagination = options?.resetPagination ?? true
    const next = new URLSearchParams(searchParams)
    if (!value) next.delete(key)
    else next.set(key, value)
    if (resetPagination) {
      next.delete('page')
      next.delete('cursor')
      next.delete('semanticCursor')
      resetSemanticPagination()
    }
    setSearchParams(next)
  }

  /**
   * Updates the explorer grouping mode and assigns a default recent window when grouped views need one.
   */
  function setView(nextView: ExplorerViewMode) {
    const next = new URLSearchParams(searchParams)
    if (nextView === 'time') {
      next.delete('view')
    } else {
      next.set('view', nextView)
      if (!start || !end) {
        const endDate = new Date()
        const startDate = new Date(endDate)
        startDate.setDate(endDate.getDate() - 29)
        next.set('start', toLocalDateString(startDate))
        next.set('end', toLocalDateString(endDate))
      }
    }
    next.delete('page')
    next.delete('cursor')
    next.delete('semanticCursor')
    resetSemanticPagination()
    setSearchParams(next)
  }

  /**
   * Renders the go to semantic route.
   *
   * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
   */
  function goToSemanticPage(nextCursor: string | null) {
    updateParam('semanticCursor', nextCursor, { resetPagination: false })
  }

  /**
   * Renders the go to history route.
   *
   * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
   */
  function goToHistoryPage(nextPage: number) {
    const normalizedPage = Math.max(1, nextPage)
    const next = new URLSearchParams(searchParams)
    if (normalizedPage <= 1) {
      next.delete('page')
    } else {
      next.set('page', String(normalizedPage))
    }
    next.delete('cursor')
    setSearchParams(next)
  }

  /**
   * Updates the number of visible history rows per page.
   */
  function setHistoryPageSize(nextPageSize: number) {
    const normalizedPageSize = keywordPageSizeOptions.includes(
      nextPageSize as (typeof keywordPageSizeOptions)[number],
    )
      ? nextPageSize
      : defaultKeywordPageSize

    persistKeywordPageSize(normalizedPageSize)
    setStoredPageSize(normalizedPageSize)

    const next = new URLSearchParams(searchParams)
    if (normalizedPageSize === defaultKeywordPageSize) {
      next.delete('pageSize')
    } else {
      next.set('pageSize', String(normalizedPageSize))
    }
    next.delete('page')
    next.delete('cursor')
    setSearchParams(next)
  }

  /**
   * Renders the handle first history route.
   *
   * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
   */
  function handleFirstHistoryPage(historyPage: number) {
    if (historyPage <= 1) return
    goToHistoryPage(1)
  }

  /**
   * Renders the handle last history route.
   *
   * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
   */
  function handleLastHistoryPage(
    historyPage: number,
    historyPageCount: number,
  ) {
    if (historyPage >= historyPageCount) return
    goToHistoryPage(historyPageCount)
  }

  /**
   * Renders the handle next history route.
   *
   * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
   */
  function handleNextHistoryPage(historyPage: number) {
    goToHistoryPage(historyPage + 1)
  }

  /**
   * Renders the handle previous history route.
   *
   * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
   */
  function handlePreviousHistoryPage(historyPage: number) {
    goToHistoryPage(historyPage - 1)
  }

  /**
   * Handles history page jump.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function handleHistoryPageJump(
    historyPage: number,
    historyPageCount: number,
  ) {
    const parsed = Number.parseInt(historyPageInput, 10)
    if (!Number.isFinite(parsed)) {
      setHistoryPageInput(String(historyPage))
      return
    }
    const nextPage = Math.min(historyPageCount, Math.max(1, parsed))
    setHistoryPageInput(String(nextPage))
    if (nextPage === historyPage) return
    goToHistoryPage(nextPage)
  }

  /**
   * Renders the handle next semantic route.
   *
   * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
   */
  function handleNextSemanticPage(nextCursor: string | null) {
    if (!nextCursor) return
    setSemanticCursorTrail((current) => ({
      ...current,
      [semanticQuerySignature]: [
        ...(current[semanticQuerySignature] ?? []),
        semanticCursor ?? '',
      ],
    }))
    goToSemanticPage(nextCursor)
  }

  /**
   * Renders the handle previous semantic route.
   *
   * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
   */
  function handlePreviousSemanticPage() {
    const previousCursor = semanticTrail[semanticTrail.length - 1] || null
    setSemanticCursorTrail((current) => ({
      ...current,
      [semanticQuerySignature]: (current[semanticQuerySignature] ?? []).slice(
        0,
        -1,
      ),
    }))
    goToSemanticPage(previousCursor)
  }

  /**
   * Explains how clear date range works.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function clearDateRange() {
    resetSemanticPagination()
    setSearchParams((current) => {
      const next = new URLSearchParams(current)
      next.delete('page')
      next.delete('start')
      next.delete('end')
      next.delete('cursor')
      next.delete('semanticCursor')
      return next
    })
  }

  /**
   * Explains how apply date shortcut works.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function applyDateShortcut(days: number) {
    const endDate = new Date()
    const startDate = new Date(endDate)
    startDate.setDate(endDate.getDate() - (days - 1))
    const next = new URLSearchParams(searchParams)
    next.delete('page')
    next.set('start', toLocalDateString(startDate))
    next.set('end', toLocalDateString(endDate))
    next.delete('cursor')
    next.delete('semanticCursor')
    resetSemanticPagination()
    setSearchParams(next)
  }

  /**
   * Explains how active date shortcut works.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function activeDateShortcut() {
    return resolveExplorerActiveDateShortcut(start, end)
  }

  const groupedDateRange = useMemo(() => {
    return resolveExplorerGroupedDateRange(start, end)
  }, [end, start])

  return {
    activeDateShortcut,
    activeFilters,
    applyDateShortcut,
    browserKind,
    browserKinds,
    buildRecentSearchLabel,
    clearAllFilters,
    clearDateRange,
    currentQuery,
    cursor,
    deferredQuery,
    domain,
    end,
    explicitPage,
    pageSize,
    explicitProfileId,
    groupedDateRange,
    handleFirstHistoryPage,
    handleHistoryPageJump,
    handleLastHistoryPage,
    handleNextHistoryPage,
    handleNextSemanticPage,
    handlePreviousHistoryPage,
    handlePreviousSemanticPage,
    historyPageInput,
    historyQuerySignature,
    mode,
    persistRecentSearch,
    profileId,
    queryInput,
    // The submitted query — equals the URL `q`. Since typing never writes `q`,
    // this is the last-submitted value the route compares the draft against to
    // gate the Search button + surface the stale-results banner.
    rawQuery,
    recentSearches,
    regexMode,
    regexValid,
    searchParams,
    setHistoryPageSize,
    semanticCursor,
    semanticQuery,
    semanticQuerySignature,
    semanticTrail,
    setView,
    setHistoryPageInput,
    setQueryInput,
    setRecentSearches,
    setSearchParams,
    sort,
    start,
    updateParam,
    view,
  }
}
