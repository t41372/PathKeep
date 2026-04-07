import { useEffect, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'
import { formatBytes, formatDateTime } from '../../lib/format'
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
    if (!runId) return
    let cancelled = false
    const loadDetail = async () => {
      try {
        const response = await backend.loadAuditRunDetail(runId)
        if (!cancelled) setDetailState({ runId, detail: response, error: null })
      } catch (nextError) {
        if (!cancelled)
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
      if (!navigator.clipboard?.writeText)
        throw new Error('Clipboard unavailable')
      await navigator.clipboard.writeText(path)
      setCopiedPath(path)
    } catch {
      setCopiedPath(`error:${path}`)
    }
  }

  if (shellLoading && !snapshot)
    return (
      <section className="page-shell">
        <LoadingState label="Loading audit ledger" />
      </section>
    )
  if (shellError && !snapshot)
    return (
      <section className="page-shell">
        <ErrorState
          title="Audit ledger is unavailable"
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
              className="btn-primary"
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
      {/* Manifest Chain Visualization */}
      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">MANIFEST CHAIN</span>
          <span className="panel-action">Verify integrity</span>
        </div>
        <div className="panel-body">
          <div className="chain-viz">
            {snapshot.recentRuns.slice(0, 4).map((run, i) => (
              <div key={run.id} style={{ display: 'contents' }}>
                {i > 0 && <div className="chain-link">→</div>}
                <div
                  className={`chain-block ${i >= 3 ? 'older' : ''}`}
                  style={{ cursor: 'pointer' }}
                  onClick={() => {
                    const nextParams = new URLSearchParams(searchParams)
                    nextParams.set('run', String(run.id))
                    setSearchParams(nextParams)
                  }}
                >
                  <div className="chain-hash mono">#{run.id}</div>
                  <div className="chain-meta dim">
                    <div>
                      {run.status.toUpperCase()} · {run.profilesProcessed}{' '}
                      profiles
                    </div>
                    <div className="mono">
                      {run.manifestHash
                        ? `sha256:${run.manifestHash.slice(0, 8)}...`
                        : 'pending'}
                    </div>
                  </div>
                </div>
              </div>
            ))}
            {snapshot.recentRuns.length > 4 && (
              <div className="chain-link dim">→ ···</div>
            )}
          </div>
        </div>
      </div>

      {/* Run Detail */}
      {loading ? (
        <LoadingState label="Loading run detail" />
      ) : error ? (
        <ErrorState title="Run detail is unavailable" description={error} />
      ) : detail ? (
        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">
              RUN #{detail.run.id} · MANIFEST DETAIL
            </span>
          </div>
          <div className="panel-body">
            <div className="manifest-grid">
              <div className="manifest-field">
                <span className="field-label">RUN ID</span>
                <span className="field-value mono">#{detail.run.id}</span>
              </div>
              <div className="manifest-field">
                <span className="field-label">TYPE</span>
                <span className="field-value">
                  {detail.trigger === 'manual'
                    ? 'Manual Backup'
                    : 'Scheduled Backup'}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">SOURCE</span>
                <span className="field-value">
                  {detail.profileScope.join(' · ') || 'Archive-wide'}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">EXECUTED AT</span>
                <span className="field-value mono">
                  {formatDateTime(detail.run.startedAt, 'en') ??
                    detail.run.startedAt}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">MANIFEST HASH</span>
                <span className="field-value mono">
                  {detail.manifestHash ?? 'N/A'}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">MANIFEST PATH</span>
                <span className="field-value mono">
                  {detail.manifestPath ?? 'N/A'}
                </span>
              </div>
            </div>
            <div className="detail-divider" />
            <div className="manifest-stats">
              <div className="manifest-stat">
                <span className="dim">New visits</span>
                <span className="mono accent">+{detail.run.newVisits}</span>
              </div>
              <div className="manifest-stat">
                <span className="dim">New URLs</span>
                <span className="mono">{detail.run.newUrls}</span>
              </div>
              <div className="manifest-stat">
                <span className="dim">Downloads</span>
                <span className="mono">{detail.run.newDownloads}</span>
              </div>
              <div className="manifest-stat">
                <span className="dim">Profiles</span>
                <span className="mono">{detail.run.profilesProcessed}</span>
              </div>
            </div>

            {/* Artifacts */}
            {detail.artifacts.length > 0 && (
              <>
                <div className="detail-divider" />
                <div style={{ marginTop: 'var(--space-3)' }}>
                  <span
                    className="mono-kicker"
                    style={{ marginBottom: 'var(--space-2)', display: 'block' }}
                  >
                    ARTIFACTS · {detail.artifacts.length} files
                  </span>
                  {detail.artifacts.map((artifact) => (
                    <div
                      key={`${artifact.kind}:${artifact.path}`}
                      style={{ marginBottom: 'var(--space-2)' }}
                    >
                      <div
                        style={{
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'center',
                        }}
                      >
                        <span className="mono" style={{ fontSize: '11px' }}>
                          {artifact.kind} — {artifact.path}
                        </span>
                        <span className="dim mono" style={{ fontSize: '10px' }}>
                          {formatBytes(artifact.sizeBytes ?? 0)}
                        </span>
                      </div>
                      <div
                        style={{
                          display: 'flex',
                          gap: 'var(--space-2)',
                          marginTop: 'var(--space-1)',
                        }}
                      >
                        <button
                          className="btn-tiny"
                          type="button"
                          onClick={() => {
                            void backend.openPathInFileManager(artifact.path)
                          }}
                        >
                          Open
                        </button>
                        <button
                          className="btn-tiny"
                          type="button"
                          onClick={() => {
                            void handleCopyPath(artifact.path)
                          }}
                        >
                          Copy
                        </button>
                      </div>
                      {copiedPath === artifact.path && (
                        <span className="dim mono" style={{ fontSize: '10px' }}>
                          Copied
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              </>
            )}

            {/* Warnings */}
            {detail.warnings.length > 0 && (
              <>
                <div className="detail-divider" />
                <div className="warning-box">
                  <div className="warning-icon">⚠</div>
                  <div className="warning-text">
                    {detail.warnings.map((w) => (
                      <div key={w}>{w}</div>
                    ))}
                  </div>
                </div>
              </>
            )}

            <div
              className="wizard-actions"
              style={{ marginTop: 'var(--space-4)' }}
            >
              {detail.manifestPath && (
                <>
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => {
                      void backend.openPathInFileManager(detail.manifestPath!)
                    }}
                  >
                    View Manifest
                  </button>
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => {
                      void handleCopyPath(detail.manifestPath!)
                    }}
                  >
                    Copy Path
                  </button>
                </>
              )}
            </div>
            {detail.manifestPath && copiedPath === detail.manifestPath && (
              <span className="dim mono" style={{ fontSize: '10px' }}>
                Copied
              </span>
            )}
          </div>
        </div>
      ) : (
        <EmptyState
          description="Click a block in the manifest chain above to inspect run details, artifacts, and the hash trail."
          eyebrow="DETAIL"
          title="No audit run selected"
        />
      )}
    </section>
  )
}
