/**
 * This module contains route-level hooks that support the Explorer surface.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `useExplorerData`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import {
  startTransition,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from 'react'
import {
  copyReviewValue,
  type ReviewCopyFeedback,
} from '../../../components/review'
import { backend } from '../../../lib/backend-client'
import { normalizeExplorerBackgroundPrefetchPages } from '../../../lib/explorer-preferences'
import { waitForNextPaint } from '../../../lib/wait-for-next-paint'
import type {
  AiProviderConnectionTestReport,
  ExportFormat,
  ExportResult,
} from '../../../lib/types'
import { loadRecentSearches } from '../helpers'
import type {
  ExplorerMode,
  ExplorerViewMode,
  RecentSearchEntry,
} from '../types'

/**
 * Collects the inputs needed by `UseExplorerData`.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface UseExplorerDataOptions {
  archiveReady: boolean
  backgroundPrefetchPages: number
  cacheToken: number
  currentQuery: Parameters<typeof backend.queryHistory>[0]
  embeddingProviderId: string | null
  end: string | null
  historyBlockedByInvalidRegex: boolean
  labels: {
    exportFailed: string
    loadingArchive: string
    queryFailedTitle: string
    semanticRecallDegradedTitle: string
    visitFailed: string
  }
  mode: ExplorerMode
  view: ExplorerViewMode
  persistRecentSearch: (params: RecentSearchEntry['params']) => void
  refreshAppData: () => Promise<void>
  refreshRuntimeStatus: () => Promise<unknown>
  requestKey: string
  semanticQuery: Parameters<typeof backend.searchAiHistory>[0]
  semanticRequestKey: string
  setRecentSearches: Dispatch<SetStateAction<RecentSearchEntry[]>>
  start: string | null
}

/**
 * Describes a request payload in this front-end contract.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface QueueJobRequest {
  fullRebuild: boolean
  clearOnly: boolean
}

/**
 * Provides the `useExplorerData` hook.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function useExplorerData({
  archiveReady,
  backgroundPrefetchPages,
  cacheToken,
  currentQuery,
  embeddingProviderId,
  end,
  historyBlockedByInvalidRegex,
  labels,
  mode,
  view,
  persistRecentSearch,
  refreshAppData,
  refreshRuntimeStatus,
  requestKey,
  semanticQuery,
  semanticRequestKey,
  setRecentSearches,
  start,
}: UseExplorerDataOptions) {
  const historyRequestRef = useRef({
    backgroundPrefetchPages: normalizeExplorerBackgroundPrefetchPages(
      backgroundPrefetchPages,
    ),
    cacheToken,
    currentQuery,
    end,
    mode,
    view,
    persistRecentSearch,
    queryFailedTitle: labels.queryFailedTitle,
    setRecentSearches,
    start,
  })
  const semanticRequestRef = useRef({
    semanticQuery,
    semanticRecallDegradedTitle: labels.semanticRecallDegradedTitle,
  })
  const historyCacheRef = useRef(
    new Map<string, Awaited<ReturnType<typeof backend.queryHistory>>>(),
  )
  const historyPrefetchRef = useRef(
    new Map<
      string,
      Promise<Awaited<ReturnType<typeof backend.queryHistory>>>
    >(),
  )
  const historyPrefetchSequenceRef = useRef<string | null>(null)
  const [queryState, setQueryState] = useState({
    requestKey: null as string | null,
    results: null as Awaited<ReturnType<typeof backend.queryHistory>> | null,
    error: null as string | null,
  })
  const [semanticState, setSemanticState] = useState({
    requestKey: null as string | null,
    results: null as Awaited<ReturnType<typeof backend.searchAiHistory>> | null,
    error: null as string | null,
  })
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [exportResult, setExportResult] = useState<ExportResult | null>(null)
  const [copyFeedback, setCopyFeedback] = useState<ReviewCopyFeedback | null>(
    null,
  )
  const [providerProbe, setProviderProbe] =
    useState<AiProviderConnectionTestReport | null>(null)
  const [indexAction, setIndexAction] = useState<string | null>(null)
  const [queueAction, setQueueAction] = useState<string | null>(null)
  const [intelligenceError, setIntelligenceError] = useState<string | null>(
    null,
  )
  const [actionError, setActionError] = useState<string | null>(null)

  historyRequestRef.current = {
    backgroundPrefetchPages: normalizeExplorerBackgroundPrefetchPages(
      backgroundPrefetchPages,
    ),
    cacheToken,
    currentQuery,
    end,
    mode,
    view,
    persistRecentSearch,
    queryFailedTitle: labels.queryFailedTitle,
    setRecentSearches,
    start,
  }
  semanticRequestRef.current = {
    semanticQuery,
    semanticRecallDegradedTitle: labels.semanticRecallDegradedTitle,
  }

  /**
   * Builds the stable cache key used for the current Explorer history query.
   */
  function buildHistoryRequestKey(
    query: Parameters<typeof backend.queryHistory>[0],
    nextCacheToken: number,
  ) {
    return JSON.stringify({
      currentQuery: query,
      refreshKey: nextCacheToken,
    })
  }

  /**
   * Retains a small bounded cache of current and adjacent Explorer pages.
   */
  function storeHistoryCache(
    key: string,
    results: Awaited<ReturnType<typeof backend.queryHistory>>,
  ) {
    const cache = historyCacheRef.current
    cache.delete(key)
    cache.set(key, results)
    const cacheLimit = Math.max(
      6,
      historyRequestRef.current.backgroundPrefetchPages * 2 + 2,
    )
    while (cache.size > cacheLimit) {
      const oldestKey = cache.keys().next().value
      if (!oldestKey) {
        break
      }
      cache.delete(oldestKey)
    }
  }

  /**
   * Reuses cached pages or prefetches one adjacent page without blocking the current render path.
   */
  const prefetchHistoryPage = useEffectEvent(function prefetchHistoryPage(
    query: Parameters<typeof backend.queryHistory>[0],
    nextCacheToken: number,
  ) {
    const key = buildHistoryRequestKey(query, nextCacheToken)
    const cached = historyCacheRef.current.get(key)
    if (cached) {
      return Promise.resolve(cached)
    }

    const inflightRequest = historyPrefetchRef.current.get(key)
    if (inflightRequest) {
      return inflightRequest
    }

    const request = backend
      .queryHistory(query)
      .then((response) => {
        storeHistoryCache(key, response)
        return response
      })
      .finally(() => {
        historyPrefetchRef.current.delete(key)
      })

    historyPrefetchRef.current.set(key, request)
    return request
  })

  /**
   * Builds a page-pinned history query that matches the URL grammar Explorer uses for navigation.
   */
  function buildPagedHistoryQuery(
    query: Parameters<typeof backend.queryHistory>[0],
    page: number,
  ) {
    return {
      ...query,
      cursor: null,
      page: page <= 1 ? null : page,
    }
  }

  /**
   * Schedules a bounded background warmup for nearby Explorer pages.
   */
  const prefetchHistoryWindow = useEffectEvent(function prefetchHistoryWindow(
    query: Parameters<typeof backend.queryHistory>[0],
    currentPage: number,
    pageCount: number,
    nextCacheToken: number,
    nextBackgroundPrefetchPages: number,
  ) {
    const prefetchPages = normalizeExplorerBackgroundPrefetchPages(
      nextBackgroundPrefetchPages,
    )
    if (prefetchPages <= 0) {
      return
    }

    const sequenceKey = buildHistoryRequestKey(query, nextCacheToken)
    historyPrefetchSequenceRef.current = sequenceKey
    const scheduledPages: number[] = []

    for (let offset = 1; offset <= prefetchPages; offset += 1) {
      const previousPage = currentPage - offset
      if (previousPage >= 1) {
        scheduledPages.push(previousPage)
      }

      const followingPage = currentPage + offset
      if (followingPage <= pageCount) {
        scheduledPages.push(followingPage)
      }
    }

    void (async () => {
      for (const page of scheduledPages) {
        if (historyPrefetchSequenceRef.current !== sequenceKey) {
          return
        }

        try {
          await prefetchHistoryPage(
            buildPagedHistoryQuery(query, page),
            nextCacheToken,
          )
        } catch {
          return
        }
      }
    })()
  })

  useEffect(() => {
    if (!archiveReady || historyBlockedByInvalidRegex || view !== 'time') {
      setQueryState((current) =>
        current.requestKey === requestKey && current.results === null
          ? current
          : { requestKey, results: null, error: null },
      )
      return
    }
    let cancelled = false
    /**
     * Loads results.
     *
     * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const loadResults = async () => {
      const request = historyRequestRef.current
      const nextRequestKey = buildHistoryRequestKey(
        request.currentQuery,
        request.cacheToken,
      )
      const cachedResults = historyCacheRef.current.get(nextRequestKey)
      if (cachedResults) {
        startTransition(() => {
          setQueryState({
            requestKey: nextRequestKey,
            results: cachedResults,
            error: null,
          })
          setSelectedId((current) =>
            cachedResults.items.some((item) => item.id === current)
              ? current
              : (cachedResults.items[0]?.id ?? null),
          )
        })
        prefetchHistoryWindow(
          request.currentQuery,
          cachedResults.page,
          cachedResults.pageCount,
          request.cacheToken,
          request.backgroundPrefetchPages,
        )
        return
      }
      const inflightPrefetch = historyPrefetchRef.current.get(nextRequestKey)
      try {
        await waitForNextPaint()
        if (cancelled) return
        const response = inflightPrefetch
          ? await inflightPrefetch
          : await backend.queryHistory(request.currentQuery)
        if (cancelled) return
        storeHistoryCache(nextRequestKey, response)
        request.persistRecentSearch({
          q: request.currentQuery.q,
          mode: request.mode,
          view: request.view,
          regex: request.currentQuery.regexMode ? '1' : null,
          domain: request.currentQuery.domain,
          profileId: request.currentQuery.profileId,
          browserKind: request.currentQuery.browserKind,
          start: request.start,
          end: request.end,
          sort: request.currentQuery.sort ?? 'newest',
        })
        request.setRecentSearches(loadRecentSearches())
        await waitForNextPaint()
        if (cancelled) return
        startTransition(() => {
          setQueryState({ requestKey, results: response, error: null })
          setSelectedId((current) =>
            response.items.some((item) => item.id === current)
              ? current
              : (response.items[0]?.id ?? null),
          )
        })
        prefetchHistoryWindow(
          request.currentQuery,
          response.page,
          response.pageCount,
          request.cacheToken,
          request.backgroundPrefetchPages,
        )
      } catch (error) {
        if (cancelled) return
        setQueryState({
          requestKey,
          results: null,
          error:
            error instanceof Error ? error.message : request.queryFailedTitle,
        })
      }
    }
    void loadResults()
    return () => {
      cancelled = true
      historyPrefetchSequenceRef.current = null
    }
  }, [archiveReady, historyBlockedByInvalidRegex, requestKey, view])

  useEffect(() => {
    const request = semanticRequestRef.current
    if (!archiveReady || mode === 'keyword' || !request.semanticQuery.query) {
      setSemanticState({
        requestKey: semanticRequestKey,
        results: null,
        error: null,
      })
      return
    }
    let cancelled = false
    /**
     * Loads semantic results.
     *
     * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const loadSemanticResults = async () => {
      const currentRequest = semanticRequestRef.current
      try {
        const response = await backend.searchAiHistory(
          currentRequest.semanticQuery,
        )
        if (cancelled) return
        setSemanticState({
          requestKey: semanticRequestKey,
          results: response,
          error: null,
        })
        setSelectedId(
          (current) => current ?? response.items[0]?.historyId ?? null,
        )
      } catch (error) {
        if (cancelled) return
        setSemanticState({
          requestKey: semanticRequestKey,
          results: null,
          error:
            error instanceof Error
              ? error.message
              : currentRequest.semanticRecallDegradedTitle,
        })
      }
    }
    void loadSemanticResults()
    return () => {
      cancelled = true
    }
  }, [archiveReady, mode, semanticQuery.query, semanticRequestKey])

  /**
   * Refreshes queue status.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function refreshQueueStatus() {
    await refreshRuntimeStatus()
  }

  /**
   * Handles queue action.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleQueueAction(
    label: string,
    action: () => Promise<unknown>,
  ) {
    setQueueAction(label)
    setIntelligenceError(null)
    setActionError(null)
    try {
      await action()
      await Promise.all([refreshAppData(), refreshQueueStatus()])
    } catch (error) {
      setIntelligenceError(
        error instanceof Error ? error.message : labels.loadingArchive,
      )
    } finally {
      setQueueAction(null)
    }
  }

  /**
   * Handles index action.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleIndexAction(label: string, request: QueueJobRequest) {
    setIndexAction(label)
    setIntelligenceError(null)
    setActionError(null)
    try {
      await backend.buildAiIndex({
        providerId: embeddingProviderId,
        fullRebuild: request.fullRebuild,
        clearOnly: request.clearOnly,
        limit: null,
      })
      await Promise.all([refreshAppData(), refreshQueueStatus()])
    } catch (error) {
      setIntelligenceError(
        error instanceof Error
          ? error.message
          : labels.semanticRecallDegradedTitle,
      )
    } finally {
      setIndexAction(null)
    }
  }

  /**
   * Handles provider probe.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleProviderProbe() {
    if (!embeddingProviderId) return
    setIntelligenceError(null)
    setActionError(null)
    setProviderProbe(null)
    try {
      const probe = await backend.testAiProviderConnection({
        providerId: embeddingProviderId,
        purpose: 'embedding',
      })
      setProviderProbe(probe)
    } catch (error) {
      setIntelligenceError(
        error instanceof Error
          ? error.message
          : labels.semanticRecallDegradedTitle,
      )
    } finally {
      setQueueAction(null)
    }
  }

  /**
   * Handles export.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleExport(format: ExportFormat) {
    setActionError(null)
    try {
      const nextResult = await backend.exportHistory({
        format,
        query: currentQuery,
      })
      setExportResult(nextResult)
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : labels.exportFailed,
      )
    }
  }

  /**
   * Handles copy export path.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleCopyExportPath(path: string) {
    await copyReviewValue(path, {
      key: `explorer:export:${path}`,
      onFeedback: setCopyFeedback,
    })
  }

  /**
   * Handles visit.
   *
   * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleVisit(url: string) {
    setActionError(null)
    try {
      await backend.openExternalUrl(url)
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : labels.visitFailed,
      )
    }
  }

  return {
    actionError,
    cachedHistoryResults: historyCacheRef.current.get(requestKey) ?? null,
    copyFeedback,
    exportResult,
    handleCopyExportPath,
    handleExport,
    handleIndexAction,
    handleProviderProbe,
    handleQueueAction,
    handleVisit,
    indexAction,
    intelligenceError,
    providerProbe,
    queryState,
    queueAction,
    refreshQueueStatus,
    selectedId,
    semanticState,
    setQueueAction,
    setSelectedId,
  }
}
