import { useDeferredValue, useEffect, useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { backend } from '../../lib/backend'
import {
  formatDateTime,
  formatDuration,
  formatRelativeTime,
} from '../../lib/format'
import type {
  ExportFormat,
  ExportResult,
  HistoryQueryResponse,
} from '../../lib/types'

const recentSearchesStorageKey = 'pathkeep.explorer.recent-searches'

interface ExplorerQueryState {
  requestKey: string | null
  results: HistoryQueryResponse | null
  error: string | null
}

function loadRecentSearches() {
  if (typeof window === 'undefined') {
    return [] as string[]
  }

  const raw = window.localStorage.getItem(recentSearchesStorageKey)
  if (!raw) {
    return []
  }

  try {
    const parsed: unknown = JSON.parse(raw)
    return Array.isArray(parsed)
      ? parsed.filter((item) => typeof item === 'string')
      : []
  } catch {
    return []
  }
}

function persistRecentSearch(label: string) {
  if (typeof window === 'undefined' || !label.trim()) {
    return
  }

  const next = [
    label,
    ...loadRecentSearches().filter((item) => item !== label),
  ].slice(0, 4)
  window.localStorage.setItem(recentSearchesStorageKey, JSON.stringify(next))
}

function endOfDayMs(value: string) {
  const timestamp = new Date(`${value}T23:59:59.999`).getTime()
  return Number.isNaN(timestamp) ? null : timestamp
}

export function ExplorerPage() {
  const {
    error: shellError,
    loading: shellLoading,
    refreshKey,
    snapshot,
  } = useShellData()
  const [searchParams, setSearchParams] = useSearchParams()
  const [queryState, setQueryState] = useState<ExplorerQueryState>({
    requestKey: null,
    results: null,
    error: null,
  })
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [exportResult, setExportResult] = useState<ExportResult | null>(null)
  const [recentSearches, setRecentSearches] =
    useState<string[]>(loadRecentSearches)
  const [copiedExportPath, setCopiedExportPath] = useState<string | null>(null)

  const deferredQuery = useDeferredValue(searchParams.get('q') ?? '')
  const profileId = searchParams.get('profileId')
  const browserKind = searchParams.get('browserKind')
  const domain = searchParams.get('domain')
  const start = searchParams.get('start')
  const end = searchParams.get('end')
  const sort =
    (searchParams.get('sort') as 'newest' | 'oldest' | null) ?? 'newest'
  const currentQuery = useMemo(
    () => ({
      q: deferredQuery || null,
      profileId,
      browserKind,
      domain,
      startTimeMs: start ? new Date(`${start}T00:00:00.000`).getTime() : null,
      endTimeMs: end ? endOfDayMs(end) : null,
      sort,
      limit: 150,
    }),
    [browserKind, deferredQuery, domain, end, profileId, sort, start],
  )
  const archiveReady = Boolean(
    snapshot?.config.initialized && snapshot.archiveStatus.unlocked,
  )
  const requestKey = useMemo(
    () => JSON.stringify({ currentQuery, refreshKey }),
    [currentQuery, refreshKey],
  )

  useEffect(() => {
    if (!archiveReady) {
      return
    }

    let cancelled = false
    const loadResults = async () => {
      try {
        const response = await backend.queryHistory(currentQuery)
        if (cancelled) {
          return
        }

        setQueryState({
          requestKey,
          results: response,
          error: null,
        })
        setSelectedId((current) =>
          response.items.some((item) => item.id === current)
            ? current
            : (response.items[0]?.id ?? null),
        )

        const labelParts = [
          currentQuery.q,
          currentQuery.domain ? `domain:${currentQuery.domain}` : null,
          currentQuery.profileId,
        ].filter(Boolean)
        if (labelParts.length > 0) {
          const label = labelParts.join(' · ')
          persistRecentSearch(label)
          setRecentSearches(loadRecentSearches())
        }
      } catch (nextError) {
        if (!cancelled) {
          setQueryState({
            requestKey,
            results: null,
            error:
              nextError instanceof Error
                ? nextError.message
                : 'PathKeep could not read the current explorer query.',
          })
        }
      }
    }

    void loadResults()

    return () => {
      cancelled = true
    }
  }, [archiveReady, currentQuery, requestKey])

  const results =
    archiveReady && queryState.requestKey === requestKey
      ? queryState.results
      : null
  const error =
    archiveReady && queryState.requestKey === requestKey
      ? queryState.error
      : null
  const loading = archiveReady && queryState.requestKey !== requestKey

  const selectedEntry =
    results?.items.find((item) => item.id === selectedId) ??
    results?.items[0] ??
    null
  const browserKinds = Array.from(
    new Set(
      (snapshot?.browserProfiles ?? [])
        .filter((profile) => profile.historyExists)
        .map((profile) => profile.profileId.split(':')[0]),
    ),
  )

  function updateParam(key: string, value: string | null) {
    const nextParams = new URLSearchParams(searchParams)
    if (!value) {
      nextParams.delete(key)
    } else {
      nextParams.set(key, value)
    }
    setSearchParams(nextParams)
  }

  async function handleExport(format: ExportFormat) {
    const nextResult = await backend.exportHistory({
      format,
      query: currentQuery,
    })
    setExportResult(nextResult)
  }

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

  if (shellLoading && !snapshot) {
    return (
      <section className="page-shell">
        <LoadingState label="Loading explorer workspace" />
      </section>
    )
  }

  if (shellError && !snapshot) {
    return (
      <section className="page-shell">
        <ErrorState
          title="Explorer could not load the archive shell"
          description={shellError}
        />
      </section>
    )
  }

  if (!snapshot?.config.initialized) {
    return (
      <section className="page-shell">
        <EmptyState
          action={
            <Link className="primary-button" to="/onboarding">
              Initialize archive first
            </Link>
          }
          description="Explorer reads the canonical archive only after onboarding initializes the local database and the first manual backup completes."
          eyebrow="EXPLORER"
          title="History Explorer is waiting for the first archive run"
        />
      </section>
    )
  }

  if (!snapshot.archiveStatus.unlocked) {
    return (
      <section className="page-shell">
        <PermissionGate
          detail="The archive is locked right now. Unlock it first so PathKeep can query visit evidence and export the current result set."
          eyebrow="LOCKED"
          title="Explorer needs an unlocked archive"
        >
          <Link className="primary-button" to="/security">
            Review security
          </Link>
        </PermissionGate>
      </section>
    )
  }

  return (
    <section className="page-shell explorer-page" data-testid="explorer-page">
      <section className="shell-panel">
        <div className="panel-header">
          <span className="panel-title">QUERY + FILTERS</span>
          <span className="panel-action">
            {results ? `${results.total} visible records` : 'Waiting for query'}
          </span>
        </div>
        <div className="panel-body explorer-filters">
          <label className="field-stack">
            <span className="mono-kicker">KEYWORD</span>
            <input
              aria-label="Explorer keyword"
              type="search"
              value={searchParams.get('q') ?? ''}
              onChange={(event) => updateParam('q', event.target.value || null)}
            />
          </label>
          <label className="field-stack">
            <span className="mono-kicker">PROFILE</span>
            <select
              aria-label="Explorer profile"
              value={searchParams.get('profileId') ?? ''}
              onChange={(event) =>
                updateParam('profileId', event.target.value || null)
              }
            >
              <option value="">All selected profiles</option>
              {snapshot.config.selectedProfileIds.map((profileId) => (
                <option key={profileId} value={profileId}>
                  {profileId}
                </option>
              ))}
            </select>
          </label>
          <label className="field-stack">
            <span className="mono-kicker">BROWSER</span>
            <select
              aria-label="Explorer browser"
              value={searchParams.get('browserKind') ?? ''}
              onChange={(event) =>
                updateParam('browserKind', event.target.value || null)
              }
            >
              <option value="">All browsers</option>
              {browserKinds.map((browserKind) => (
                <option key={browserKind} value={browserKind}>
                  {browserKind}
                </option>
              ))}
            </select>
          </label>
          <label className="field-stack">
            <span className="mono-kicker">DOMAIN</span>
            <input
              aria-label="Explorer domain"
              type="search"
              value={searchParams.get('domain') ?? ''}
              onChange={(event) =>
                updateParam('domain', event.target.value || null)
              }
            />
          </label>
          <label className="field-stack">
            <span className="mono-kicker">START</span>
            <input
              aria-label="Explorer start date"
              type="date"
              value={searchParams.get('start') ?? ''}
              onChange={(event) =>
                updateParam('start', event.target.value || null)
              }
            />
          </label>
          <label className="field-stack">
            <span className="mono-kicker">END</span>
            <input
              aria-label="Explorer end date"
              type="date"
              value={searchParams.get('end') ?? ''}
              onChange={(event) =>
                updateParam('end', event.target.value || null)
              }
            />
          </label>
          <label className="field-stack">
            <span className="mono-kicker">SORT</span>
            <select
              aria-label="Explorer sort order"
              value={searchParams.get('sort') ?? 'newest'}
              onChange={(event) => updateParam('sort', event.target.value)}
            >
              <option value="newest">Newest first</option>
              <option value="oldest">Oldest first</option>
            </select>
          </label>
        </div>
        {recentSearches.length > 0 ? (
          <div className="panel-body recent-search-bar">
            {recentSearches.map((item) => (
              <button
                key={item}
                className="chip-button"
                type="button"
                onClick={() => updateParam('q', item)}
              >
                {item}
              </button>
            ))}
          </div>
        ) : null}
      </section>

      {loading ? (
        <LoadingState label="Searching the canonical archive" />
      ) : error ? (
        <ErrorState title="Explorer query failed" description={error} />
      ) : results && results.total === 0 ? (
        <EmptyState
          action={
            <button
              className="ghost-button"
              type="button"
              onClick={() => setSearchParams(new URLSearchParams())}
            >
              Clear filters
            </button>
          }
          description="No visible visit matched the current keyword, facet, and date filters. Adjust the query or clear the date range."
          eyebrow="NO MATCHES"
          title="Explorer did not find any visible history"
        />
      ) : (
        <div className="explorer-grid">
          <section className="shell-panel">
            <div className="panel-header">
              <span className="panel-title">RESULTS</span>
              <span className="panel-action">Evidence source · visit</span>
            </div>
            <div className="panel-body explorer-results">
              {(results?.items ?? []).map((item) => (
                <button
                  key={item.id}
                  className={`result-row ${
                    selectedEntry?.id === item.id ? 'result-row--active' : ''
                  }`}
                  type="button"
                  onClick={() => setSelectedId(item.id)}
                >
                  <div className="result-row__header">
                    <strong>{item.title || item.url}</strong>
                    <span className="mono-support">
                      {formatRelativeTime(item.visitedAt)}
                    </span>
                  </div>
                  <p>{item.url}</p>
                  <div className="result-row__meta">
                    <span className="state-chip state-chip--ready">visit</span>
                    <span className="mono-support">{item.profileId}</span>
                    <span className="mono-support">#{item.sourceVisitId}</span>
                  </div>
                </button>
              ))}
            </div>
          </section>

          <aside className="stacked-column">
            <section className="shell-panel shell-panel--accent">
              <div className="panel-header">
                <span className="panel-title">DETAIL</span>
                <span className="panel-action">Canonical visit evidence</span>
              </div>
              <div className="panel-body stack-list">
                {selectedEntry ? (
                  <>
                    <article className="list-item">
                      <strong>
                        {selectedEntry.title || selectedEntry.url}
                      </strong>
                      <span className="mono-support">{selectedEntry.url}</span>
                    </article>
                    <article className="list-item">
                      <strong>Visited at</strong>
                      <span className="mono-support">
                        {formatDateTime(selectedEntry.visitedAt, 'en') ??
                          selectedEntry.visitedAt}
                      </span>
                    </article>
                    <article className="list-item">
                      <strong>Evidence source</strong>
                      <span className="mono-support">
                        visit · {selectedEntry.profileId} · #
                        {selectedEntry.sourceVisitId}
                      </span>
                    </article>
                    <article className="list-item">
                      <strong>Interaction details</strong>
                      <span className="mono-support">
                        {formatDuration(selectedEntry.durationMs)} · transition{' '}
                        {selectedEntry.transition ?? 'n/a'}
                        {selectedEntry.appId
                          ? ` · app ${selectedEntry.appId}`
                          : ''}
                      </span>
                    </article>
                  </>
                ) : (
                  <EmptyState
                    description="Pick a result to inspect the visit, source profile, and export boundary."
                    eyebrow="DETAIL"
                    title="No result selected"
                  />
                )}
              </div>
            </section>

            <section className="shell-panel">
              <div className="panel-header">
                <span className="panel-title">EXPORT</span>
                <span className="panel-action">Visible query only</span>
              </div>
              <div className="panel-body stack-list">
                <div className="segmented-row">
                  {(
                    ['jsonl', 'markdown', 'html', 'text'] as ExportFormat[]
                  ).map((format) => (
                    <button
                      key={format}
                      className="chip-button"
                      type="button"
                      onClick={() => {
                        void handleExport(format)
                      }}
                    >
                      {format}
                    </button>
                  ))}
                </div>
                <article className="list-item">
                  <strong>Export boundary</strong>
                  <span className="mono-support">
                    Export uses the current visible query only. Reverted or
                    hidden facts stay out of this artifact.
                  </span>
                </article>
                {exportResult ? (
                  <article className="list-item">
                    <strong>Latest export</strong>
                    <span className="mono-support">{exportResult.path}</span>
                    <span className="mono-support">
                      {exportResult.count} records · {exportResult.format}
                    </span>
                    <div className="utility-block__actions">
                      <button
                        className="ghost-button"
                        type="button"
                        onClick={() => {
                          void backend.openPathInFileManager(exportResult.path)
                        }}
                      >
                        Open path
                      </button>
                      <button
                        className="ghost-button"
                        type="button"
                        onClick={() => {
                          void handleCopyExportPath(exportResult.path)
                        }}
                      >
                        Copy path
                      </button>
                    </div>
                    {copiedExportPath === exportResult.path ? (
                      <span className="mono-support">Copied path</span>
                    ) : null}
                    {copiedExportPath === `error:${exportResult.path}` ? (
                      <span className="mono-support">
                        Clipboard unavailable
                      </span>
                    ) : null}
                  </article>
                ) : null}
              </div>
            </section>
          </aside>
        </div>
      )}
    </section>
  )
}
