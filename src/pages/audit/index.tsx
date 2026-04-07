import { useEffect, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'
import {
  formatBytes,
  formatDateTime,
  formatRelativeTime,
} from '../../lib/format'
import type { AuditRunDetail } from '../../lib/types'

interface AuditDetailState {
  runId: number | null
  detail: AuditRunDetail | null
  error: string | null
}

export function AuditPage() {
  const {
    error: shellError,
    loading: shellLoading,
    refreshKey,
    runBackup,
    snapshot,
  } = useShellData()
  const [searchParams, setSearchParams] = useSearchParams()
  const [detailState, setDetailState] = useState<AuditDetailState>({
    runId: null,
    detail: null,
    error: null,
  })
  const [copiedPath, setCopiedPath] = useState<string | null>(null)

  const runIdFromParams = Number(searchParams.get('run') ?? '')
  const runId =
    Number.isFinite(runIdFromParams) && runIdFromParams > 0
      ? runIdFromParams
      : (snapshot?.recentRuns[0]?.id ?? null)

  useEffect(() => {
    if (!runId) {
      return
    }

    let cancelled = false
    const loadDetail = async () => {
      try {
        const response = await backend.loadAuditRunDetail(runId)
        if (!cancelled) {
          setDetailState({
            runId,
            detail: response,
            error: null,
          })
        }
      } catch (nextError) {
        if (!cancelled) {
          setDetailState({
            runId,
            detail: null,
            error:
              nextError instanceof Error
                ? nextError.message
                : 'PathKeep could not load the selected audit run.',
          })
        }
      }
    }

    void loadDetail()

    return () => {
      cancelled = true
    }
  }, [refreshKey, runId])

  const detail = detailState.runId === runId ? detailState.detail : null
  const error = detailState.runId === runId ? detailState.error : null
  const loading = Boolean(runId) && detailState.runId !== runId

  async function handleCopyPath(path: string) {
    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error('Clipboard unavailable')
      }
      await navigator.clipboard.writeText(path)
      setCopiedPath(path)
    } catch {
      setCopiedPath(`error:${path}`)
    }
  }

  if (shellLoading && !snapshot) {
    return (
      <section className="page-shell">
        <LoadingState label="Loading audit ledger" />
      </section>
    )
  }

  if (shellError && !snapshot) {
    return (
      <section className="page-shell">
        <ErrorState
          title="Audit ledger is unavailable"
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
              Finish onboarding
            </Link>
          }
          description="Audit records appear after the first successful backup writes a manifest and artifact trail."
          eyebrow="AUDIT"
          title="The audit ledger has no archive runs yet"
        />
      </section>
    )
  }

  if (snapshot.recentRuns.length === 0) {
    return (
      <section className="page-shell">
        <EmptyState
          action={
            <button
              className="primary-button"
              type="button"
              onClick={() => {
                void runBackup()
              }}
            >
              Run a manual backup
            </button>
          }
          description="The audit ledger will populate as soon as a manual backup finishes and PathKeep writes the manifest chain."
          eyebrow="AUDIT"
          title="No backup runs recorded yet"
        />
      </section>
    )
  }

  return (
    <section className="page-shell audit-page" data-testid="audit-page">
      <div className="explorer-grid">
        <section className="shell-panel">
          <div className="panel-header">
            <span className="panel-title">RUN LEDGER</span>
            <span className="panel-action">
              {snapshot.recentRuns.length} recent backup runs
            </span>
          </div>
          <div className="panel-body explorer-results">
            {snapshot.recentRuns.map((run) => (
              <button
                key={run.id}
                className={`result-row ${detail?.run.id === run.id ? 'result-row--active' : ''}`}
                type="button"
                onClick={() => {
                  const nextParams = new URLSearchParams(searchParams)
                  nextParams.set('run', String(run.id))
                  setSearchParams(nextParams)
                }}
              >
                <div className="result-row__header">
                  <strong>Run #{run.id}</strong>
                  <span className="mono-support">
                    {formatRelativeTime(run.finishedAt ?? run.startedAt)}
                  </span>
                </div>
                <p>
                  {run.status} · {run.profilesProcessed} profiles ·{' '}
                  {run.newVisits} visits
                </p>
                <div className="result-row__meta">
                  <span className="state-chip state-chip--ready">
                    {run.status}
                  </span>
                  <span className="mono-support">
                    {run.manifestHash ?? 'manifest pending'}
                  </span>
                </div>
              </button>
            ))}
          </div>
          {snapshot.recentImportBatches.length > 0 ? (
            <div className="panel-body stack-list">
              <article className="list-item">
                <strong>Recent import batches</strong>
                <span className="mono-support">
                  {snapshot.recentImportBatches.length} batch entries stay
                  available for rollback review.
                </span>
              </article>
            </div>
          ) : null}
        </section>

        <aside className="stacked-column">
          {loading ? (
            <LoadingState label="Loading run detail" />
          ) : error ? (
            <ErrorState title="Run detail is unavailable" description={error} />
          ) : detail ? (
            <>
              <section className="shell-panel shell-panel--accent">
                <div className="panel-header">
                  <span className="panel-title">RUN DETAIL</span>
                  <span className="panel-action">{detail.trigger}</span>
                </div>
                <div className="panel-body stack-list">
                  <article className="list-item">
                    <strong>Summary</strong>
                    <span className="mono-support">
                      {detail.run.status} · {detail.run.newVisits} visits ·{' '}
                      {detail.run.newUrls} URLs · {detail.run.newDownloads}{' '}
                      downloads
                    </span>
                  </article>
                  <article className="list-item">
                    <strong>Started</strong>
                    <span className="mono-support">
                      {formatDateTime(detail.run.startedAt, 'en') ??
                        detail.run.startedAt}
                    </span>
                  </article>
                  <article className="list-item">
                    <strong>Profile scope</strong>
                    <span className="mono-support">
                      {detail.profileScope.join(' · ') || 'Archive-wide'}
                    </span>
                  </article>
                  <article className="list-item">
                    <strong>Manifest</strong>
                    <span className="mono-support">
                      {detail.manifestPath ?? 'No manifest artifact recorded'}
                    </span>
                    <div className="utility-block__actions">
                      {detail.manifestPath ? (
                        <>
                          <button
                            className="ghost-button"
                            type="button"
                            onClick={() => {
                              void backend.openPathInFileManager(
                                detail.manifestPath!,
                              )
                            }}
                          >
                            Open path
                          </button>
                          <button
                            className="ghost-button"
                            type="button"
                            onClick={() => {
                              void handleCopyPath(detail.manifestPath!)
                            }}
                          >
                            Copy path
                          </button>
                        </>
                      ) : null}
                    </div>
                    {detail.manifestPath &&
                    copiedPath === detail.manifestPath ? (
                      <span className="mono-support">Copied path</span>
                    ) : null}
                    {detail.manifestPath &&
                    copiedPath === `error:${detail.manifestPath}` ? (
                      <span className="mono-support">
                        Clipboard unavailable
                      </span>
                    ) : null}
                  </article>
                </div>
              </section>

              <section className="shell-panel">
                <div className="panel-header">
                  <span className="panel-title">ARTIFACTS</span>
                  <span className="panel-action">
                    {detail.artifacts.length} files
                  </span>
                </div>
                <div className="panel-body stack-list">
                  {detail.artifacts.map((artifact) => (
                    <article
                      key={`${artifact.kind}:${artifact.path}`}
                      className="list-item"
                    >
                      <strong>{artifact.kind}</strong>
                      <span className="mono-support">{artifact.path}</span>
                      <span className="mono-support">
                        {artifact.reason ?? 'Artifact recorded'} ·{' '}
                        {formatBytes(artifact.sizeBytes ?? 0)}
                      </span>
                      <div className="utility-block__actions">
                        <button
                          className="ghost-button"
                          type="button"
                          onClick={() => {
                            void backend.openPathInFileManager(artifact.path)
                          }}
                        >
                          Open path
                        </button>
                        <button
                          className="ghost-button"
                          type="button"
                          onClick={() => {
                            void handleCopyPath(artifact.path)
                          }}
                        >
                          Copy path
                        </button>
                      </div>
                      {copiedPath === artifact.path ? (
                        <span className="mono-support">Copied path</span>
                      ) : null}
                      {copiedPath === `error:${artifact.path}` ? (
                        <span className="mono-support">
                          Clipboard unavailable
                        </span>
                      ) : null}
                    </article>
                  ))}
                </div>
              </section>

              <section className="shell-panel">
                <div className="panel-header">
                  <span className="panel-title">WARNINGS + NOTES</span>
                  <span className="panel-action">
                    {detail.warnings.length} warning
                    {detail.warnings.length === 1 ? '' : 's'}
                  </span>
                </div>
                <div className="panel-body stack-list">
                  {detail.warnings.length > 0 ? (
                    detail.warnings.map((warning) => (
                      <article key={warning} className="list-item">
                        <strong>Warning</strong>
                        <span className="mono-support">{warning}</span>
                      </article>
                    ))
                  ) : (
                    <article className="list-item">
                      <strong>No warnings recorded</strong>
                      <span className="mono-support">
                        This run finished without audit warnings.
                      </span>
                    </article>
                  )}
                </div>
              </section>
            </>
          ) : (
            <EmptyState
              description="Pick a run from the ledger to inspect the manifest path, snapshot artifacts, and warning trail."
              eyebrow="DETAIL"
              title="No audit run selected"
            />
          )}
        </aside>
      </div>
    </section>
  )
}
