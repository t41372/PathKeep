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
  startTransition,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
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
  dateShortcutWindows,
  endOfDayMs,
  keywordPageSize,
  loadRecentSearches,
  recentSearchesStorageKey,
  semanticPageSize,
  toLocalDateString,
} from '../helpers'
import type { ExplorerMode, RecentSearchEntry, Translator } from '../types'

/**
 * Defines the typed shape for active filter.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface ActiveFilter {
  id: string
  label: string
  value: string
}

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
  const [recentSearches, setRecentSearches] =
    useState<RecentSearchEntry[]>(loadRecentSearches)
  const rawQuery = searchParams.get('q') ?? ''
  const [queryInput, setQueryInput] = useState(rawQuery)
  const deferredQuery = useDeferredValue(rawQuery)
  const regexMode = searchParams.get('regex') === '1'
  const mode = (searchParams.get('mode') as ExplorerMode | null) ?? 'keyword'
  const explicitProfileId = searchParams.get('profileId')
  const profileId = explicitProfileId ?? activeProfileId
  const browserKind = searchParams.get('browserKind')
  const domain = searchParams.get('domain')
  const start = searchParams.get('start')
  const end = searchParams.get('end')
  const sort =
    (searchParams.get('sort') as 'newest' | 'oldest' | null) ?? 'newest'
  const explicitPage = (() => {
    const raw = searchParams.get('page')
    if (!raw) return null
    const parsed = Number.parseInt(raw, 10)
    return Number.isFinite(parsed) && parsed > 0 ? parsed : null
  })()
  const cursor = searchParams.get('cursor')
  const semanticCursor = searchParams.get('semanticCursor')
  const [semanticCursorTrail, setSemanticCursorTrail] = useState<
    Record<string, string[]>
  >({})
  const [historyPageInput, setHistoryPageInput] = useState('1')
  const historyScrollPositionsRef = useRef<Record<string, number>>({})
  const pendingHistoryScrollKeyRef = useRef<string | null>(null)

  const regexValid = useMemo(() => {
    if (!regexMode || !queryInput.trim()) return true

    try {
      new RegExp(queryInput)
      return true
    } catch {
      return false
    }
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
      const labelParts = [
        params.mode === 'semantic'
          ? explorerT('modeSemantic')
          : params.mode === 'hybrid'
            ? explorerT('modeHybrid')
            : null,
        params.regex === '1' ? explorerT('activeFilterRegexEnabled') : null,
        params.q?.trim() ? params.q.trim() : null,
        params.domain?.trim() ? params.domain.trim() : null,
        params.profileId ? profileIdLabel(params.profileId) : null,
        params.browserKind ? browserLabel(params.browserKind) : null,
        params.start || params.end
          ? [
              formatRecentDate(params.start) ?? explorerT('allRecordedTime'),
              formatRecentDate(params.end) ?? explorerT('allRecordedTime'),
            ].join(' - ')
          : null,
      ].filter(Boolean)

      return labelParts.join(' · ')
    },
    [explorerT, formatRecentDate],
  )

  const persistRecentSearch = useCallback(
    (params: RecentSearchEntry['params']) => {
      if (typeof window === 'undefined') return

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
    () => ({
      q: deferredQuery || null,
      profileId,
      browserKind,
      domain,
      startTimeMs: start ? new Date(`${start}T00:00:00.000`).getTime() : null,
      endTimeMs: end ? endOfDayMs(end) : null,
      sort,
      limit: keywordPageSize,
      page: explicitPage,
      cursor: explicitPage ? null : cursor,
      regexMode,
    }),
    [
      browserKind,
      cursor,
      deferredQuery,
      domain,
      end,
      explicitPage,
      profileId,
      regexMode,
      sort,
      start,
    ],
  )
  const semanticQuery = useMemo(
    () => ({
      query: deferredQuery.trim(),
      profileId,
      domain,
      limit: semanticPageSize,
      cursor: semanticCursor,
    }),
    [deferredQuery, domain, profileId, semanticCursor],
  )
  const historyQuerySignature = useMemo(
    () =>
      JSON.stringify({
        q: rawQuery || null,
        profileId,
        browserKind,
        domain,
        start,
        end,
        sort,
        regexMode,
      }),
    [browserKind, rawQuery, domain, end, profileId, regexMode, sort, start],
  )
  const semanticQuerySignature = useMemo(
    () =>
      JSON.stringify({
        query: deferredQuery.trim(),
        profileId,
        domain,
        mode,
      }),
    [deferredQuery, domain, mode, profileId],
  )
  const semanticTrail = semanticCursorTrail[semanticQuerySignature] ?? []

  useEffect(() => {
    setQueryInput(rawQuery)
  }, [rawQuery])

  useEffect(() => {
    if (queryInput === rawQuery) {
      return
    }

    const timeoutId = window.setTimeout(() => {
      pendingHistoryScrollKeyRef.current = null
      setSemanticCursorTrail((current) => ({
        ...current,
        [semanticQuerySignature]: [],
      }))
      startTransition(() => {
        setSearchParams((current) => {
          const next = new URLSearchParams(current)
          if (queryInput) {
            next.set('q', queryInput)
          } else {
            next.delete('q')
          }
          next.delete('page')
          next.delete('cursor')
          next.delete('semanticCursor')
          return next
        })
      })
    }, 180)

    return () => {
      window.clearTimeout(timeoutId)
    }
  }, [queryInput, rawQuery, semanticQuerySignature, setSearchParams])

  const browserKinds = Array.from(
    new Set(
      selectedProfileIds.map((profile) => profile.split(':')[0] ?? profile),
    ),
  )
  const activeFilters = [
    searchParams.get('q')
      ? {
          id: 'q',
          label: explorerT('filterKeyword'),
          value: searchParams.get('q') as string,
        }
      : null,
    mode !== 'keyword'
      ? {
          id: 'mode',
          label: explorerT('activeFilterMode'),
          value:
            mode === 'semantic'
              ? explorerT('modeSemantic')
              : explorerT('modeHybrid'),
        }
      : null,
    regexMode
      ? {
          id: 'regex',
          label: explorerT('activeFilterRegex'),
          value: explorerT('activeFilterRegexEnabled'),
        }
      : null,
    searchParams.get('domain')
      ? {
          id: 'domain',
          label: explorerT('filterDomain'),
          value: searchParams.get('domain') as string,
        }
      : null,
    searchParams.get('profileId')
      ? {
          id: 'profileId',
          label: explorerT('filterProfile'),
          value: searchParams.get('profileId') as string,
        }
      : null,
    searchParams.get('browserKind')
      ? {
          id: 'browserKind',
          label: explorerT('filterBrowser'),
          value: searchParams.get('browserKind') as string,
        }
      : null,
    start
      ? {
          id: 'start',
          label: explorerT('filterStart'),
          value: start,
        }
      : null,
    end
      ? {
          id: 'end',
          label: explorerT('filterEnd'),
          value: end,
        }
      : null,
  ].filter((value): value is ActiveFilter => Boolean(value))

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
      pendingHistoryScrollKeyRef.current = null
    }
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
   * Explains how history page key works.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function historyPageKey(targetPage: number) {
    return `${historyQuerySignature}|${targetPage}`
  }

  /**
   * Explains how queue history scroll restore works.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function queueHistoryScrollRestore(targetPage: number) {
    const scrollContainer = document.querySelector('.workspace-scroll')
    if (!(scrollContainer instanceof HTMLElement)) {
      pendingHistoryScrollKeyRef.current = null
      return
    }
    const key = historyPageKey(targetPage)
    historyScrollPositionsRef.current[key] = scrollContainer.scrollTop
    pendingHistoryScrollKeyRef.current = key
  }

  /**
   * Renders the go to history route.
   *
   * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Explorer expectations in the design docs.
   */
  function goToHistoryPage(nextPage: number) {
    const normalizedPage = Math.max(1, nextPage)
    queueHistoryScrollRestore(normalizedPage)
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
    pendingHistoryScrollKeyRef.current = null
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
    pendingHistoryScrollKeyRef.current = null
  }

  /**
   * Explains how active date shortcut works.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function activeDateShortcut() {
    if (!start || !end) return null

    const today = new Date()
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
    explicitProfileId,
    handleFirstHistoryPage,
    handleHistoryPageJump,
    handleLastHistoryPage,
    handleNextHistoryPage,
    handleNextSemanticPage,
    handlePreviousHistoryPage,
    handlePreviousSemanticPage,
    historyPageInput,
    historyQuerySignature,
    historyScrollPositionsRef,
    mode,
    pendingHistoryScrollKeyRef,
    persistRecentSearch,
    profileId,
    queryInput,
    recentSearches,
    regexMode,
    regexValid,
    searchParams,
    semanticCursor,
    semanticQuery,
    semanticQuerySignature,
    semanticTrail,
    setHistoryPageInput,
    setQueryInput,
    setRecentSearches,
    setSearchParams,
    sort,
    start,
    updateParam,
  }
}
