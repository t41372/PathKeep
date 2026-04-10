import {
  startTransition,
  type KeyboardEvent,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import {
  SkeletonExplorer,
  SkeletonExplorerResults,
} from '../../components/primitives/skeleton'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend'
import {
  formatDateTime,
  formatDuration,
  formatRelativeTime,
} from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  aiStatusMeta,
  assistantHref,
  evidenceHref,
  scoreBand,
  selectedAiProvider,
} from '../../lib/intelligence'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import type {
  AiProviderConnectionTestReport,
  AiQueueStatus,
  AiSearchResponse,
  ExportFormat,
  ExportResult,
  HistoryQueryResponse,
} from '../../lib/types'

const recentSearchesStorageKey = 'pathkeep.explorer.recent-searches'
const keywordPageSize = 50
const semanticPageSize = 8

interface ExplorerQueryState {
  requestKey: string | null
  results: HistoryQueryResponse | null
  error: string | null
}

interface SemanticQueryState {
  requestKey: string | null
  results: AiSearchResponse | null
  error: string | null
}

interface RecentSearchEntry {
  label: string
  params: {
    q?: string | null
    mode?: 'keyword' | 'semantic' | 'hybrid' | null
    domain?: string | null
    profileId?: string | null
    browserKind?: string | null
    start?: string | null
    end?: string | null
    regex?: '1' | null
    sort?: 'newest' | 'oldest'
  }
}

const dateShortcutWindows = [
  { key: 'DAY', days: 1 },
  { key: 'WEEK', days: 7 },
  { key: 'MONTH', days: 30 },
  { key: 'YEAR', days: 365 },
] as const

function endOfDayMs(value: string) {
  const timestamp = new Date(`${value}T23:59:59.999`).getTime()
  return Number.isNaN(timestamp) ? null : timestamp
}

function toLocalDateString(value: Date) {
  return value.toLocaleDateString('en-CA')
}

function buildRecentSearchLabel(params: RecentSearchEntry['params']) {
  const labelParts = [
    params.mode && params.mode !== 'keyword' ? `mode:${params.mode}` : null,
    params.regex === '1' ? 'regex' : null,
    params.q?.trim() ? `q:${params.q.trim()}` : null,
    params.domain?.trim() ? `domain:${params.domain.trim()}` : null,
    params.profileId ? `profile:${params.profileId}` : null,
    params.browserKind ? `browser:${params.browserKind}` : null,
    params.start || params.end
      ? `range:${params.start ?? '…'}→${params.end ?? '…'}`
      : null,
  ].filter(Boolean)

  return labelParts.join(' · ')
}

function isRecentSearchEntry(value: unknown): value is RecentSearchEntry {
  return (
    typeof value === 'object' &&
    value !== null &&
    'label' in value &&
    typeof value.label === 'string' &&
    'params' in value &&
    typeof value.params === 'object' &&
    value.params !== null
  )
}

function loadRecentSearches() {
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

function persistRecentSearch(params: RecentSearchEntry['params']) {
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
    ...loadRecentSearches().filter((entry) => entry.label !== label),
  ].slice(0, 4)
  window.localStorage.setItem(recentSearchesStorageKey, JSON.stringify(next))
}

function browserLabel(kind: string) {
  if (kind === 'chrome') return 'Chrome'
  if (kind === 'arc') return 'Arc'
  if (kind === 'firefox') return 'Firefox'
  if (kind === 'safari') return 'Safari'
  return kind
}

function activateRecordSelection(
  event: KeyboardEvent<HTMLDivElement>,
  onSelect: () => void,
) {
  if (event.key !== 'Enter' && event.key !== ' ') {
    return
  }

  event.preventDefault()
  onSelect()
}

export function ExplorerPage() {
  const {
    error: shellError,
    loading: shellLoading,
    refreshKey,
    refreshAppData,
    snapshot,
  } = useShellData()
  const { language, t, ns } = useI18n()
  const { activeProfileId } = useProfileScope()
  const commonT = ns('common')
  const explorerT = ns('explorer')
  const intelligenceT = ns('intelligence')
  const [searchParams, setSearchParams] = useSearchParams()
  const [queryState, setQueryState] = useState<ExplorerQueryState>({
    requestKey: null,
    results: null,
    error: null,
  })
  const [semanticState, setSemanticState] = useState<SemanticQueryState>({
    requestKey: null,
    results: null,
    error: null,
  })
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [exportResult, setExportResult] = useState<ExportResult | null>(null)
  const [recentSearches, setRecentSearches] =
    useState<RecentSearchEntry[]>(loadRecentSearches)
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

  const rawQuery = searchParams.get('q') ?? ''
  const [queryInput, setQueryInput] = useState(rawQuery)
  const deferredQuery = useDeferredValue(rawQuery)
  const regexMode = searchParams.get('regex') === '1'
  const regexValid = useMemo(() => {
    if (!regexMode || !queryInput.trim()) return true

    try {
      new RegExp(queryInput)
      return true
    } catch {
      return false
    }
  }, [queryInput, regexMode])
  const mode =
    (searchParams.get('mode') as 'keyword' | 'semantic' | 'hybrid' | null) ??
    'keyword'
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

  const archiveReady = Boolean(
    snapshot?.config.initialized && snapshot.archiveStatus.unlocked,
  )
  const aiMeta = snapshot
    ? aiStatusMeta(snapshot.aiStatus, intelligenceT)
    : null
  const embeddingProvider = snapshot
    ? selectedAiProvider(snapshot.config.ai, 'embedding')
    : null
  const requestKey = useMemo(
    () => JSON.stringify({ currentQuery, refreshKey }),
    [currentQuery, refreshKey],
  )
  const semanticRequestKey = useMemo(
    () => JSON.stringify({ semanticQuery, mode, refreshKey }),
    [mode, refreshKey, semanticQuery],
  )

  const historyBlockedByInvalidRegex =
    archiveReady && regexMode && Boolean(queryInput.trim()) && !regexValid

  useEffect(() => {
    if (!archiveReady || historyBlockedByInvalidRegex) return
    let cancelled = false
    const loadResults = async () => {
      try {
        const response = await backend.queryHistory(currentQuery)
        if (cancelled) return
        setQueryState({ requestKey, results: response, error: null })
        setSelectedId((current) =>
          response.items.some((item) => item.id === current)
            ? current
            : (response.items[0]?.id ?? null),
        )
        persistRecentSearch({
          q: currentQuery.q,
          mode,
          regex: currentQuery.regexMode ? '1' : null,
          domain: currentQuery.domain,
          profileId: currentQuery.profileId,
          browserKind: currentQuery.browserKind,
          start,
          end,
          sort: currentQuery.sort ?? 'newest',
        })
        setRecentSearches(loadRecentSearches())
      } catch (nextError) {
        if (!cancelled)
          setQueryState({
            requestKey,
            results: null,
            error:
              nextError instanceof Error
                ? nextError.message
                : explorerT('queryFailedTitle'),
          })
      }
    }
    void loadResults()
    return () => {
      cancelled = true
    }
  }, [
    archiveReady,
    currentQuery,
    end,
    explorerT,
    historyBlockedByInvalidRegex,
    mode,
    requestKey,
    start,
  ])

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
    if (!archiveReady || mode === 'keyword' || !semanticQuery.query) {
      setSemanticState({
        requestKey: semanticRequestKey,
        results: null,
        error: null,
      })
      return
    }
    let cancelled = false
    const loadSemanticResults = async () => {
      try {
        const response = await backend.searchAiHistory(semanticQuery)
        if (!cancelled) {
          setSemanticState({
            requestKey: semanticRequestKey,
            results: response,
            error: null,
          })
          setSelectedId(
            (current) => current ?? response.items[0]?.historyId ?? null,
          )
        }
      } catch (nextError) {
        if (!cancelled) {
          setSemanticState({
            requestKey: semanticRequestKey,
            results: null,
            error:
              nextError instanceof Error
                ? nextError.message
                : explorerT('semanticRecallDegradedTitle'),
          })
        }
      }
    }
    void loadSemanticResults()
    return () => {
      cancelled = true
    }
  }, [archiveReady, explorerT, mode, semanticQuery, semanticRequestKey])

  const results =
    archiveReady &&
    (historyBlockedByInvalidRegex || queryState.requestKey === requestKey)
      ? queryState.results
      : null
  const semanticResults =
    archiveReady && semanticState.requestKey === semanticRequestKey
      ? semanticState.results
      : null
  const semanticError =
    archiveReady && semanticState.requestKey === semanticRequestKey
      ? semanticState.error
      : null
  const semanticLoading =
    archiveReady &&
    mode !== 'keyword' &&
    Boolean(semanticQuery.query) &&
    semanticState.requestKey !== semanticRequestKey
  const error =
    archiveReady &&
    (historyBlockedByInvalidRegex || queryState.requestKey === requestKey)
      ? queryState.error
      : null
  const loading =
    archiveReady &&
    !historyBlockedByInvalidRegex &&
    queryState.requestKey !== requestKey
  const historyPage = results?.page ?? explicitPage ?? 1
  const historyPageCount = results?.pageCount ?? 1
  const selectedEntry =
    results?.items.find((item) => item.id === selectedId) ??
    results?.items[0] ??
    null

  useEffect(() => {
    setHistoryPageInput(String(historyPage))
  }, [historyPage])

  useEffect(() => {
    const pendingKey = pendingHistoryScrollKeyRef.current
    if (!pendingKey) return
    if (pendingKey !== `${historyQuerySignature}|${historyPage}`) return
    const scrollContainer = document.querySelector('.workspace-scroll')
    if (!(scrollContainer instanceof HTMLElement)) {
      pendingHistoryScrollKeyRef.current = null
      return
    }
    const nextScrollTop = historyScrollPositionsRef.current[pendingKey]
    if (typeof nextScrollTop === 'number') {
      scrollContainer.scrollTop = nextScrollTop
    }
    pendingHistoryScrollKeyRef.current = null
  }, [historyPage, historyQuerySignature])

  const browserKinds = Array.from(
    new Set(
      (snapshot?.config.selectedProfileIds ?? []).map(
        (profile) => profile.split(':')[0] ?? profile,
      ),
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
  ].filter((value): value is { id: string; label: string; value: string } =>
    Boolean(value),
  )

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
      setSemanticCursorTrail((current) => ({
        ...current,
        [semanticQuerySignature]: [],
      }))
      pendingHistoryScrollKeyRef.current = null
    }
    setSearchParams(next)
  }

  function goToSemanticPage(nextCursor: string | null) {
    updateParam('semanticCursor', nextCursor, { resetPagination: false })
  }

  function historyPageKey(targetPage: number) {
    return `${historyQuerySignature}|${targetPage}`
  }

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

  function handleFirstHistoryPage() {
    if (historyPage <= 1) return
    goToHistoryPage(1)
  }

  function handleLastHistoryPage() {
    if (historyPage >= historyPageCount) return
    goToHistoryPage(historyPageCount)
  }

  function handleNextHistoryPage() {
    if (!results?.hasNext) return
    goToHistoryPage(historyPage + 1)
  }

  function handlePreviousHistoryPage() {
    if (!results?.hasPrevious) return
    goToHistoryPage(historyPage - 1)
  }

  function handleHistoryPageJump() {
    if (!results) return
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

  function handleNextSemanticPage() {
    if (!semanticResults?.nextCursor) return
    setSemanticCursorTrail((current) => ({
      ...current,
      [semanticQuerySignature]: [
        ...(current[semanticQuerySignature] ?? []),
        semanticCursor ?? '',
      ],
    }))
    goToSemanticPage(semanticResults.nextCursor)
  }

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

  async function refreshQueueStatus() {
    const nextQueue = await backend.loadAiQueueStatus()
    setQueueStatus(nextQueue)
  }

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
        error instanceof Error ? error.message : explorerT('loadingArchive'),
      )
    } finally {
      setQueueAction(null)
    }
  }

  async function handleIndexAction(
    label: string,
    request: { fullRebuild: boolean; clearOnly: boolean },
  ) {
    setIndexAction(label)
    setIntelligenceError(null)
    setActionError(null)
    try {
      await backend.buildAiIndex({
        providerId: embeddingProvider?.id ?? null,
        fullRebuild: request.fullRebuild,
        clearOnly: request.clearOnly,
        limit: null,
      })
      await Promise.all([refreshAppData(), refreshQueueStatus()])
    } catch (error) {
      setIntelligenceError(
        error instanceof Error
          ? error.message
          : explorerT('semanticRecallDegradedTitle'),
      )
    } finally {
      setIndexAction(null)
    }
  }

  async function handleProviderProbe() {
    if (!embeddingProvider) return
    setQueueAction(explorerT('testingProviderAction'))
    setIntelligenceError(null)
    setActionError(null)
    setProviderProbe(null)
    try {
      const probe = await backend.testAiProviderConnection({
        providerId: embeddingProvider.id,
        purpose: 'embedding',
      })
      setProviderProbe(probe)
    } catch (error) {
      setIntelligenceError(
        error instanceof Error
          ? error.message
          : explorerT('semanticRecallDegradedTitle'),
      )
    } finally {
      setQueueAction(null)
    }
  }

  function clearDateRange() {
    setSemanticCursorTrail((current) => ({
      ...current,
      [semanticQuerySignature]: [],
    }))
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
    setSemanticCursorTrail((current) => ({
      ...current,
      [semanticQuerySignature]: [],
    }))
    setSearchParams(next)
    pendingHistoryScrollKeyRef.current = null
  }

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
        error instanceof Error ? error.message : explorerT('exportFailed'),
      )
    }
  }

  async function handleCopyExportPath(path: string) {
    try {
      if (!navigator.clipboard?.writeText)
        throw new Error('Clipboard unavailable')
      await navigator.clipboard.writeText(path)
      setCopiedExportPath(path)
    } catch {
      setCopiedExportPath(`error:${path}`)
    }
  }

  async function handleVisit(url: string) {
    setActionError(null)
    try {
      await backend.openExternalUrl(url)
    } catch (error) {
      setActionError(
        error instanceof Error ? error.message : explorerT('visitFailed'),
      )
    }
  }

  if (shellLoading && !snapshot)
    return <SkeletonExplorer label={t('common.loadingExplorer')} />
  if (shellError && !snapshot)
    return (
      <section className="page-shell">
        <ErrorState
          title={explorerT('couldNotLoadTitle')}
          description={shellError}
        />
      </section>
    )
  if (!snapshot?.config.initialized) {
    return (
      <section className="page-shell">
        <EmptyState
          action={
            <Link className="btn-primary" to="/onboarding">
              {t('common.initializeFirst')}
            </Link>
          }
          description={explorerT('uninitializedDescription')}
          eyebrow={explorerT('eyebrow')}
          title={explorerT('uninitializedTitle')}
        />
      </section>
    )
  }
  if (!snapshot.archiveStatus.unlocked) {
    return (
      <section className="page-shell">
        <PermissionGate
          detail={explorerT('lockedDescription')}
          eyebrow={explorerT('lockedEyebrow')}
          title={explorerT('lockedTitle')}
        >
          <Link className="btn-primary" to="/security">
            {t('dashboard.reviewSecurity')}
          </Link>
        </PermissionGate>
      </section>
    )
  }

  return (
    <section className="page-shell explorer-page" data-testid="explorer-page">
      <div className="timeline-bar">
        <div className="timeline-controls">
          {dateShortcutWindows.map((entry) => (
            <button
              key={entry.key}
              className={`tl-btn ${
                activeDateShortcut() === entry.key ? 'active' : ''
              }`}
              type="button"
              onClick={() => applyDateShortcut(entry.days)}
            >
              {entry.key}
            </button>
          ))}
        </div>
        <div className="timeline-track">
          <span className="timeline-label">
            {results
              ? explorerT('historyPageSummary', {
                  page: historyPage,
                  loaded: results.items.length,
                  total: results.total,
                })
              : explorerT('waitingForQuery')}
          </span>
          <span className="timeline-label">
            {start || end
              ? `${start ?? '…'} → ${end ?? '…'}`
              : explorerT('allRecordedTime')}
          </span>
          {(start || end) && (
            <button className="tl-today" type="button" onClick={clearDateRange}>
              {explorerT('clearRange')}
            </button>
          )}
        </div>
      </div>

      {activeFilters.length > 0 && (
        <div className="filter-bar">
          <div className="filter-tags">
            {activeFilters.map((filter) => (
              <div key={`${filter.id}:${filter.value}`} className="filter-tag">
                <span>
                  {filter.label}: {filter.value}
                </span>
                <button
                  className="filter-remove"
                  type="button"
                  onClick={() => {
                    updateParam(filter.id, null)
                  }}
                >
                  ×
                </button>
              </div>
            ))}
          </div>
          <div className="filter-actions">
            <button
              className="filter-btn"
              type="button"
              onClick={() => setSearchParams(new URLSearchParams())}
            >
              {explorerT('clearAllFilters')}
            </button>
          </div>
        </div>
      )}

      {intelligenceError ? (
        <ErrorState
          title={explorerT('semanticRecallDegradedTitle')}
          description={intelligenceError}
        />
      ) : null}

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{explorerT('queryFiltersTitle')}</span>
          <span className="panel-action">
            {results
              ? explorerT('visibleRecords', { count: results.total })
              : explorerT('waitingForQuery')}
          </span>
        </div>
        <div className="panel-body">
          <div
            className="segmented-row"
            style={{ marginBottom: 'var(--space-4)' }}
          >
            {(['keyword', 'semantic', 'hybrid'] as const).map((option) => (
              <button
                key={option}
                className={`chip-button ${
                  mode === option ? 'chip-button--active' : ''
                }`}
                type="button"
                onClick={() =>
                  updateParam('mode', option === 'keyword' ? null : option)
                }
              >
                {option === 'keyword'
                  ? explorerT('modeKeyword')
                  : option === 'semantic'
                    ? explorerT('modeSemantic')
                    : explorerT('modeHybrid')}
              </button>
            ))}
          </div>
          <div className="explorer-filters">
            <div
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">
                {explorerT('filterKeyword')}
                {regexMode ? <span className="regex-badge">[.*]</span> : null}
              </span>
              <div className="regex-input-row">
                <input
                  aria-label={explorerT('filterKeywordAria')}
                  className={regexMode && !regexValid ? 'input-invalid' : ''}
                  type="search"
                  value={queryInput}
                  onChange={(event) => setQueryInput(event.target.value)}
                />
                <button
                  aria-label={explorerT('toggleRegex')}
                  aria-pressed={regexMode}
                  className={`regex-toggle ${
                    regexMode ? 'regex-toggle--active' : ''
                  }`}
                  title={explorerT('toggleRegex')}
                  type="button"
                  onClick={() => updateParam('regex', regexMode ? null : '1')}
                >
                  .*
                </button>
              </div>
              {regexMode && queryInput.trim() ? (
                <span
                  className={regexValid ? 'regex-valid' : 'regex-error'}
                  role={regexValid ? undefined : 'alert'}
                >
                  {regexValid
                    ? explorerT('regexValid')
                    : explorerT('regexInvalid')}
                </span>
              ) : null}
            </div>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterDomain')}</span>
              <input
                aria-label={explorerT('filterDomain')}
                type="search"
                value={searchParams.get('domain') ?? ''}
                onChange={(event) =>
                  updateParam('domain', event.target.value || null)
                }
              />
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterProfile')}</span>
              <select
                aria-label={explorerT('filterProfileAria')}
                value={profileId ?? ''}
                onChange={(event) =>
                  updateParam('profileId', event.target.value || null)
                }
              >
                <option value="">{explorerT('allProfiles')}</option>
                {snapshot.config.selectedProfileIds.map((id) => (
                  <option key={id} value={id}>
                    {id}
                  </option>
                ))}
              </select>
              {!explicitProfileId && activeProfileId ? (
                <span className="mono-support">
                  {explorerT('scopeInherited', {
                    profile: profileIdLabel(activeProfileId),
                  })}
                </span>
              ) : null}
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterBrowser')}</span>
              <select
                aria-label={explorerT('filterBrowser')}
                value={searchParams.get('browserKind') ?? ''}
                onChange={(event) =>
                  updateParam('browserKind', event.target.value || null)
                }
              >
                <option value="">{explorerT('allBrowsers')}</option>
                {browserKinds.map((kind) => (
                  <option key={kind} value={kind}>
                    {browserLabel(kind)}
                  </option>
                ))}
              </select>
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterStart')}</span>
              <input
                aria-label={explorerT('filterStart')}
                type="date"
                value={start ?? ''}
                onChange={(event) =>
                  updateParam('start', event.target.value || null)
                }
              />
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterEnd')}</span>
              <input
                aria-label={explorerT('filterEnd')}
                type="date"
                value={end ?? ''}
                onChange={(event) =>
                  updateParam('end', event.target.value || null)
                }
              />
            </label>
            <label
              className="field-stack"
              style={{ border: 'none', background: 'transparent', padding: 0 }}
            >
              <span className="mono-kicker">{explorerT('filterSort')}</span>
              <select
                aria-label={explorerT('filterSort')}
                value={searchParams.get('sort') ?? 'newest'}
                onChange={(event) => updateParam('sort', event.target.value)}
              >
                <option value="newest">{explorerT('sortNewest')}</option>
                <option value="oldest">{explorerT('sortOldest')}</option>
              </select>
            </label>
          </div>
        </div>
        <div
          className="panel-body"
          style={{
            borderTop: '1px solid var(--border)',
            paddingTop: 'var(--space-2)',
          }}
        >
          <div className="recent-search-bar">
            {recentSearches.length > 0 ? (
              recentSearches.map((entry) => (
                <button
                  key={entry.label}
                  className="chip-button"
                  type="button"
                  onClick={() =>
                    setSearchParams(
                      new URLSearchParams(
                        Object.entries(entry.params).flatMap(([key, value]) =>
                          value ? [[key, value]] : [],
                        ),
                      ),
                    )
                  }
                >
                  {entry.label}
                </button>
              ))
            ) : (
              <span className="mono-support">
                {explorerT('recentFiltersEmpty')}
              </span>
            )}
          </div>
        </div>
      </div>

      {snapshot && aiMeta && mode !== 'keyword' && (
        <div className="intelligence-grid intelligence-grid--explorer">
          <StatusCallout
            tone={aiMeta.tone}
            eyebrow={explorerT('semanticStatusEyebrow')}
            title={aiMeta.label}
            body={aiMeta.description}
            actions={
              <div className="intelligence-actions">
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() =>
                    void handleIndexAction(explorerT('buildingIndexAction'), {
                      fullRebuild: false,
                      clearOnly: false,
                    })
                  }
                  disabled={Boolean(indexAction) || !embeddingProvider}
                >
                  {explorerT('buildIndex')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() =>
                    void handleIndexAction(explorerT('rebuildingIndexAction'), {
                      fullRebuild: true,
                      clearOnly: false,
                    })
                  }
                  disabled={Boolean(indexAction) || !embeddingProvider}
                >
                  {explorerT('fullRebuild')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() =>
                    void handleIndexAction(explorerT('clearingIndexAction'), {
                      fullRebuild: false,
                      clearOnly: true,
                    })
                  }
                  disabled={Boolean(indexAction) || !embeddingProvider}
                >
                  {explorerT('clearIndex')}
                </button>
                <Link className="btn-secondary" to="/settings">
                  {explorerT('openSettings')}
                </Link>
              </div>
            }
          />

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">
                {explorerT('providerQueueTitle')}
              </span>
              <span className="panel-action">
                {embeddingProvider
                  ? `${embeddingProvider.name} / ${embeddingProvider.defaultModel}`
                  : explorerT('noEmbeddingProviderSelected')}
              </span>
            </div>
            <div className="panel-body intelligence-stack">
              <div className="intelligence-stat-row">
                <div className="summary-stat">
                  <span className="dim">{explorerT('queueQueued')}</span>
                  <span className="mono">
                    {queueStatus?.queued ?? snapshot.aiStatus.queuedJobs}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{explorerT('queueRunning')}</span>
                  <span className="mono">
                    {queueStatus?.running ?? snapshot.aiStatus.runningJobs}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{explorerT('queueFailed')}</span>
                  <span className="mono">
                    {queueStatus?.failed ?? snapshot.aiStatus.failedJobs}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{explorerT('queueState')}</span>
                  <span className="mono">
                    {(queueStatus?.paused ?? snapshot.aiStatus.queuePaused)
                      ? explorerT('queueStatePaused')
                      : explorerT('queueStateLive')}
                  </span>
                </div>
              </div>

              <div className="intelligence-actions">
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() =>
                    void handleQueueAction(
                      explorerT('refreshingQueueAction'),
                      refreshQueueStatus,
                    )
                  }
                  disabled={Boolean(queueAction)}
                >
                  {explorerT('refreshQueue')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() =>
                    void handleQueueAction(
                      explorerT('runningQueueAction'),
                      () => backend.runAiQueueJobs(2),
                    )
                  }
                  disabled={Boolean(queueAction)}
                >
                  {explorerT('drainQueue')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => void handleProviderProbe()}
                  disabled={Boolean(queueAction) || !embeddingProvider}
                >
                  {explorerT('testProvider')}
                </button>
              </div>

              {indexAction || queueAction ? (
                <LoadingState
                  compact
                  label={
                    indexAction ?? queueAction ?? explorerT('preparingRecall')
                  }
                  detail={explorerT('semanticRecallNeedsAttentionBody')}
                  progressLabel={explorerT('queueProgressLabel', {
                    queued: (
                      queueStatus?.queued ?? snapshot.aiStatus.queuedJobs
                    ).toLocaleString(language),
                    running: (
                      queueStatus?.running ?? snapshot.aiStatus.runningJobs
                    ).toLocaleString(language),
                  })}
                  progressValue={indexAction ? 50 : 75}
                />
              ) : null}

              {providerProbe && (
                <div className="result-row">
                  <div className="result-row__header">
                    <strong>
                      {providerProbe.ok
                        ? explorerT('providerReachable')
                        : explorerT('providerNeedsAttention')}
                    </strong>
                    <span className="mono-support">
                      {explorerT('providerProbeLatency', {
                        model: providerProbe.model,
                        latency:
                          providerProbe.latencyMs.toLocaleString(language),
                      })}
                    </span>
                  </div>
                  <p>{providerProbe.message}</p>
                  {providerProbe.actionHint ? (
                    <p className="mono-support">{providerProbe.actionHint}</p>
                  ) : null}
                </div>
              )}

              <div className="intelligence-job-list">
                {(queueStatus?.recentJobs ?? snapshot.aiStatus.recentJobs).map(
                  (job) => (
                    <div key={job.id} className="result-row">
                      <div className="result-row__header">
                        <strong>
                          {job.jobType} · #{job.id}
                        </strong>
                        <span className="mono-support">{job.state}</span>
                      </div>
                      <p>
                        {job.summary ??
                          job.errorMessage ??
                          explorerT('noJobSummary')}
                      </p>
                      <div className="intelligence-actions">
                        <button
                          className="btn-tiny"
                          type="button"
                          onClick={() =>
                            void handleQueueAction(
                              explorerT('replayingJobAction'),
                              () => backend.replayAiJob(job.id),
                            )
                          }
                          disabled={
                            Boolean(queueAction) ||
                            ![
                              'failed',
                              'cancelled',
                              'stale',
                              'paused',
                            ].includes(job.state)
                          }
                        >
                          {explorerT('replayJob')}
                        </button>
                        <button
                          className="btn-tiny"
                          type="button"
                          onClick={() =>
                            void handleQueueAction(
                              explorerT('cancellingJobAction'),
                              () => backend.cancelAiJob(job.id),
                            )
                          }
                          disabled={
                            Boolean(queueAction) || job.state === 'running'
                          }
                        >
                          {explorerT('cancelJob')}
                        </button>
                      </div>
                    </div>
                  ),
                )}
              </div>
            </div>
          </div>
        </div>
      )}

      {mode !== 'keyword' && (
        <div className="panel intelligence-panel">
          <div className="panel-header">
            <span className="panel-title">
              {explorerT('semanticRecallTitle')}
            </span>
            <span className="panel-action">
              {semanticResults
                ? explorerT('semanticPageSummary', {
                    page: semanticTrail.length + 1,
                    loaded: semanticResults.items.length,
                    total: semanticResults.total,
                  })
                : semanticQuery.query
                  ? explorerT('preparingRecall')
                  : explorerT('enterQueryToRank')}
            </span>
          </div>
          <div className="panel-body intelligence-stack">
            {!semanticQuery.query ? (
              <p className="dashboard-next-action">
                {explorerT('semanticPrompt')}
              </p>
            ) : semanticLoading ? (
              <LoadingState
                compact
                label={explorerT('rankingSemanticEvidence')}
                detail={explorerT('preparingRecall')}
                progressLabel={`1 / ${mode === 'hybrid' ? 3 : 2}`}
                progressValue={mode === 'hybrid' ? 33 : 50}
              />
            ) : semanticError ? (
              <ErrorState
                title={explorerT('semanticRecallDegradedTitle')}
                description={semanticError}
              />
            ) : semanticResults && semanticResults.total > 0 ? (
              <>
                {semanticResults.notes.length > 0 && (
                  <div className="intelligence-note-list">
                    {semanticResults.notes.map((note) => (
                      <p key={note} className="mono-support">
                        {note}
                      </p>
                    ))}
                  </div>
                )}
                <div className="intelligence-result-list">
                  {semanticResults.items.map((item) => {
                    const band = scoreBand(item.score, intelligenceT)
                    return (
                      <div key={item.historyId} className="result-row">
                        <div className="result-row__header">
                          <strong>{item.title ?? item.url}</strong>
                          <span className={`status-badge status-${band.tone}`}>
                            {band.label}
                          </span>
                        </div>
                        <p>{item.matchReason}</p>
                        <div className="result-row__meta">
                          <span className="mono-support">
                            {item.profileId} ·{' '}
                            {formatDateTime(item.visitedAt, language) ??
                              item.visitedAt}
                          </span>
                          <span className="mono-support">{item.url}</span>
                        </div>
                        <div className="intelligence-actions">
                          <button
                            className="btn-tiny"
                            type="button"
                            onClick={() => setSelectedId(item.historyId)}
                          >
                            {explorerT('jumpToRecord')}
                          </button>
                          <Link className="btn-tiny" to={evidenceHref(item)}>
                            {explorerT('openEvidence')}
                          </Link>
                          <Link
                            className="btn-tiny"
                            to={assistantHref(
                              explorerT('assistantExplainPrompt', {
                                item: item.title ?? item.url,
                                query: semanticQuery.query,
                              }),
                            )}
                          >
                            {explorerT('askAssistant')}
                          </Link>
                        </div>
                      </div>
                    )
                  })}
                </div>
                <div className="intelligence-actions">
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={handlePreviousSemanticPage}
                    disabled={semanticTrail.length === 0}
                  >
                    {explorerT('previousEvidencePage')}
                  </button>
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={handleNextSemanticPage}
                    disabled={!semanticResults.nextCursor}
                  >
                    {explorerT('nextEvidencePage')}
                  </button>
                </div>
              </>
            ) : (
              <EmptyState
                description={explorerT('noSemanticDescription')}
                eyebrow={explorerT('noSemanticEyebrow')}
                title={explorerT('noSemanticTitle')}
              />
            )}
          </div>
        </div>
      )}

      {loading ? (
        <SkeletonExplorerResults label={t('common.loadingExplorerResults')} />
      ) : historyBlockedByInvalidRegex ? (
        <StatusCallout
          tone="blocked"
          eyebrow={explorerT('regexEyebrow')}
          title={explorerT('regexInvalid')}
          body={explorerT('regexInvalidDetail')}
        />
      ) : error ? (
        <ErrorState title={explorerT('queryFailedTitle')} description={error} />
      ) : results && results.total === 0 ? (
        <EmptyState
          action={
            <button
              className="btn-secondary"
              type="button"
              onClick={() => setSearchParams(new URLSearchParams())}
            >
              {explorerT('clearFilters')}
            </button>
          }
          description={explorerT('noMatchesDescription')}
          eyebrow={explorerT('noMatchesEyebrow')}
          title={explorerT('noMatchesTitle')}
        />
      ) : (
        <div className="explorer-grid">
          <div className="record-list">
            <div className="record-group">
              <div className="record-group-header">
                {explorerT('resultsSummary', {
                  page: historyPage,
                  loaded: results?.items.length ?? 0,
                  total: results?.total ?? 0,
                })}
              </div>
              {(results?.items ?? []).map((item) => (
                <div
                  key={item.id}
                  className={`record-item ${selectedEntry?.id === item.id ? 'selected' : ''}`}
                  role="button"
                  tabIndex={0}
                  aria-pressed={selectedEntry?.id === item.id}
                  onClick={() => setSelectedId(item.id)}
                  onKeyDown={(event) =>
                    activateRecordSelection(event, () => setSelectedId(item.id))
                  }
                >
                  <div className="favicon-placeholder">
                    {(item.domain ?? '?')[0].toUpperCase()}
                  </div>
                  <div className="record-main">
                    <div className="record-title">{item.title || item.url}</div>
                    <div className="record-url dim mono">{item.url}</div>
                  </div>
                  <div className="record-meta">
                    <span className="dim mono" style={{ fontSize: '10px' }}>
                      {formatRelativeTime(item.visitedAt, language)}
                    </span>
                    <button
                      className="btn-tiny"
                      type="button"
                      onClick={(event) => {
                        event.stopPropagation()
                        void handleVisit(item.url)
                      }}
                    >
                      {explorerT('visitRecord')}
                    </button>
                  </div>
                </div>
              ))}
              <div
                className="intelligence-actions"
                style={{ padding: 'var(--space-3) 0 0' }}
              >
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={handleFirstHistoryPage}
                  disabled={!results?.hasPrevious}
                >
                  {explorerT('firstPage')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={handlePreviousHistoryPage}
                  disabled={!results?.hasPrevious}
                >
                  {explorerT('previousPage')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={handleNextHistoryPage}
                  disabled={!results?.hasNext}
                >
                  {explorerT('nextPage')}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={handleLastHistoryPage}
                  disabled={!results?.hasNext}
                >
                  {explorerT('lastPage')}
                </button>
                <label className="history-page-jump">
                  <span className="history-page-jump__label">
                    {explorerT('pageNumberLabel')}
                  </span>
                  <input
                    className="history-page-jump__input"
                    inputMode="numeric"
                    min={1}
                    type="number"
                    value={historyPageInput}
                    onChange={(event) =>
                      setHistoryPageInput(event.target.value)
                    }
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') {
                        event.preventDefault()
                        handleHistoryPageJump()
                      }
                    }}
                  />
                </label>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={handleHistoryPageJump}
                >
                  {explorerT('jumpToPage')}
                </button>
              </div>
            </div>
          </div>

          <div className="detail-panel">
            <div className="detail-header">
              <span className="crosshair-mark small">+</span>
              <span className="detail-label">{explorerT('recordDetail')}</span>
            </div>
            {selectedEntry ? (
              <div className="detail-body">
                <div className="detail-section">
                  <div className="detail-field">
                    <span className="field-label">
                      {explorerT('fieldTitle')}
                    </span>
                    <span className="field-value">
                      {selectedEntry.title || selectedEntry.url}
                    </span>
                  </div>
                  <div className="detail-field">
                    <span className="field-label">{explorerT('fieldUrl')}</span>
                    <span
                      className="field-value"
                      style={{ wordBreak: 'break-all' }}
                    >
                      {selectedEntry.url}
                    </span>
                  </div>
                </div>
                <div className="detail-divider" />
                <div className="detail-row">
                  <div className="detail-field half">
                    <span className="field-label">
                      {explorerT('visitedAt')}
                    </span>
                    <span className="field-value">
                      {formatDateTime(selectedEntry.visitedAt, language) ??
                        selectedEntry.visitedAt}
                    </span>
                  </div>
                  <div className="detail-field half">
                    <span className="field-label">{explorerT('duration')}</span>
                    <span className="field-value">
                      {formatDuration(selectedEntry.durationMs)}
                    </span>
                  </div>
                </div>
                <div className="detail-row">
                  <div className="detail-field half">
                    <span className="field-label">
                      {explorerT('fieldProfile')}
                    </span>
                    <span className="field-value">
                      {selectedEntry.profileId}
                    </span>
                  </div>
                  <div className="detail-field half">
                    <span className="field-label">
                      {explorerT('transition')}
                    </span>
                    <span className="field-value">
                      {selectedEntry.transition ?? commonT('notAvailable')}
                    </span>
                  </div>
                </div>
                <div className="detail-divider" />
                <div
                  className="intelligence-actions"
                  style={{ marginBottom: 'var(--space-3)' }}
                >
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => {
                      void handleVisit(selectedEntry.url)
                    }}
                  >
                    {explorerT('visitRecord')}
                  </button>
                </div>
                {actionError ? (
                  <p className="inline-error" role="alert">
                    {actionError}
                  </p>
                ) : null}
                <div className="summary-label">
                  {explorerT('exportVisibleQuery')}
                </div>
                <p className="dashboard-next-action">
                  {explorerT('exportDescription')}
                </p>
                <div style={{ display: 'flex', gap: 'var(--space-2)' }}>
                  {(
                    ['jsonl', 'markdown', 'html', 'text'] as ExportFormat[]
                  ).map((format) => (
                    <button
                      key={format}
                      className="btn-tiny"
                      disabled={historyBlockedByInvalidRegex}
                      type="button"
                      onClick={() => {
                        void handleExport(format)
                      }}
                    >
                      {format}
                    </button>
                  ))}
                </div>
                {exportResult && (
                  <div
                    style={{ marginTop: 'var(--space-3)', fontSize: '11px' }}
                  >
                    <span className="dim mono">{exportResult.path}</span>
                    <div
                      style={{
                        marginTop: 'var(--space-1)',
                        display: 'flex',
                        gap: 'var(--space-2)',
                      }}
                    >
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() => {
                          void backend.openPathInFileManager(exportResult.path)
                        }}
                      >
                        {commonT('openAction')}
                      </button>
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() => {
                          void handleCopyExportPath(exportResult.path)
                        }}
                      >
                        {commonT('copyAction')}
                      </button>
                    </div>
                    {copiedExportPath === exportResult.path && (
                      <span className="dim mono" style={{ fontSize: '10px' }}>
                        {explorerT('copied')}
                      </span>
                    )}
                    {copiedExportPath === `error:${exportResult.path}` && (
                      <span className="dim mono" style={{ fontSize: '10px' }}>
                        {explorerT('copyFailed')}
                      </span>
                    )}
                  </div>
                )}
              </div>
            ) : (
              <div className="detail-body">
                <EmptyState
                  description={explorerT('noResultDescription')}
                  eyebrow={explorerT('noResultEyebrow')}
                  title={explorerT('noResultTitle')}
                />
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  )
}
