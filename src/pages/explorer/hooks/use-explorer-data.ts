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
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from 'react'
import { backend } from '../../../lib/backend-client'
import type {
  AiQueueStatus,
  AiProviderConnectionTestReport,
  ExportFormat,
  ExportResult,
} from '../../../lib/types'
import { loadRecentSearches } from '../helpers'
import type { ExplorerMode, RecentSearchEntry } from '../types'

/**
 * Collects the inputs needed by `UseExplorerData`.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface UseExplorerDataOptions {
  archiveReady: boolean
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
  persistRecentSearch: (params: RecentSearchEntry['params']) => void
  refreshAppData: () => Promise<void>
  refreshKey: number
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
  currentQuery,
  embeddingProviderId,
  end,
  historyBlockedByInvalidRegex,
  labels,
  mode,
  persistRecentSearch,
  refreshAppData,
  refreshKey,
  requestKey,
  semanticQuery,
  semanticRequestKey,
  setRecentSearches,
  start,
}: UseExplorerDataOptions) {
  const historyRequestRef = useRef({
    currentQuery,
    end,
    mode,
    persistRecentSearch,
    queryFailedTitle: labels.queryFailedTitle,
    setRecentSearches,
    start,
  })
  const semanticRequestRef = useRef({
    semanticQuery,
    semanticRecallDegradedTitle: labels.semanticRecallDegradedTitle,
  })
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
  const [copiedExportPath, setCopiedExportPath] = useState<string | null>(null)
  const [queueStatus, setQueueStatus] = useState<AiQueueStatus | null>(null)
  const [providerProbe, setProviderProbe] =
    useState<AiProviderConnectionTestReport | null>(null)
  const [indexAction, setIndexAction] = useState<string | null>(null)
  const [queueAction, setQueueAction] = useState<string | null>(null)
  const [intelligenceError, setIntelligenceError] = useState<string | null>(
    null,
  )
  const [actionError, setActionError] = useState<string | null>(null)

  historyRequestRef.current = {
    currentQuery,
    end,
    mode,
    persistRecentSearch,
    queryFailedTitle: labels.queryFailedTitle,
    setRecentSearches,
    start,
  }
  semanticRequestRef.current = {
    semanticQuery,
    semanticRecallDegradedTitle: labels.semanticRecallDegradedTitle,
  }

  useEffect(() => {
    if (!archiveReady || historyBlockedByInvalidRegex) return
    let cancelled = false
    /**
     * Loads results.
     *
     * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const loadResults = async () => {
      const request = historyRequestRef.current
      try {
        const response = await backend.queryHistory(request.currentQuery)
        if (cancelled) return
        setQueryState({ requestKey, results: response, error: null })
        setSelectedId((current) =>
          response.items.some((item) => item.id === current)
            ? current
            : (response.items[0]?.id ?? null),
        )
        request.persistRecentSearch({
          q: request.currentQuery.q,
          mode: request.mode,
          regex: request.currentQuery.regexMode ? '1' : null,
          domain: request.currentQuery.domain,
          profileId: request.currentQuery.profileId,
          browserKind: request.currentQuery.browserKind,
          start: request.start,
          end: request.end,
          sort: request.currentQuery.sort ?? 'newest',
        })
        request.setRecentSearches(loadRecentSearches())
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
    }
  }, [archiveReady, historyBlockedByInvalidRegex, requestKey])

  useEffect(() => {
    if (!archiveReady) return
    let cancelled = false
    void backend
      .loadAiQueueStatus()
      .then((status) => {
        if (!cancelled) setQueueStatus(status)
      })
      .catch(() => {
        if (!cancelled) setQueueStatus(null)
      })
    return () => {
      cancelled = true
    }
  }, [archiveReady, refreshKey])

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
    const nextQueue = await backend.loadAiQueueStatus()
    setQueueStatus(nextQueue)
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
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error('Clipboard unavailable')
      }
      await navigator.clipboard.writeText(path)
      setCopiedExportPath(path)
    } catch {
      setCopiedExportPath(`error:${path}`)
    }
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
    copiedExportPath,
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
    queueStatus,
    refreshQueueStatus,
    selectedId,
    semanticState,
    setQueueAction,
    setSelectedId,
  }
}
