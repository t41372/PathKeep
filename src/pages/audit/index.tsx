import { useEffect, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'
import { formatBytes, formatDateTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
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
  const { language, t } = useI18n()
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
                : t('audit.runDetailUnavailable'),
          })
      }
    }
    void loadDetail()
    return () => {
      cancelled = true
    }
  }, [refreshKey, runId, t])

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
        <LoadingState label={t('audit.loadingLedger')} />
      </section>
    )
  if (shellError && !snapshot)
    return (
      <section className="page-shell">
        <ErrorState
          title={t('audit.unavailableTitle')}
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
              {t('audit.finishOnboarding')}
            </Link>
          }
          description={t('audit.emptyLedgerBody')}
          eyebrow={t('navigation.auditLabel')}
          title={t('audit.emptyLedgerTitle')}
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
              {t('audit.runManualBackup')}
            </button>
          }
          description={t('audit.noRunsBody')}
          eyebrow={t('navigation.auditLabel')}
          title={t('audit.noRunsTitle')}
        />
      </section>
    )
  }

  return (
    <section className="page-shell audit-page" data-testid="audit-page">
      <StatusCallout
        tone="info"
        title={t('audit.repairRoutesTitle')}
        body={t('audit.repairRoutesBody')}
        actions={
          <>
            <Link className="btn-secondary" to="/import">
              {t('audit.repairImports')}
            </Link>
            <Link className="btn-secondary" to="/schedule">
              {t('audit.repairSchedule')}
            </Link>
            <Link className="btn-secondary" to="/security">
              {t('audit.repairSecurity')}
            </Link>
          </>
        }
      />

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('audit.manifestChain')}</span>
          <span className="panel-action">{t('audit.verifyIntegrity')}</span>
        </div>
        <div className="panel-body">
          <div className="chain-viz">
            {snapshot.recentRuns.slice(0, 4).map((run, index) => (
              <div key={run.id} style={{ display: 'contents' }}>
                {index > 0 && <div className="chain-link">→</div>}
                <button
                  className={`chain-block ${index >= 3 ? 'older' : ''}`}
                  type="button"
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
                      {t('dashboard.profilesLabel', {
                        count: run.profilesProcessed,
                      })}
                    </div>
                    <div className="mono">
                      {run.manifestHash
                        ? `sha256:${run.manifestHash.slice(0, 8)}...`
                        : t('common.pending')}
                    </div>
                  </div>
                </button>
              </div>
            ))}
            {snapshot.recentRuns.length > 4 && (
              <div className="chain-link dim">→ ···</div>
            )}
          </div>
        </div>
      </div>

      {loading ? (
        <LoadingState label={t('audit.loadingRunDetail')} />
      ) : error ? (
        <ErrorState
          title={t('audit.runDetailUnavailable')}
          description={error}
        />
      ) : detail ? (
        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">
              {t('audit.manifestDetail', { runId: detail.run.id })}
            </span>
          </div>
          <div className="panel-body">
            <div className="manifest-grid">
              <div className="manifest-field">
                <span className="field-label">{t('audit.runId')}</span>
                <span className="field-value mono">#{detail.run.id}</span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.runType')}</span>
                <span className="field-value">
                  {detail.trigger === 'manual'
                    ? t('audit.manualBackup')
                    : t('audit.scheduledBackup')}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.runSource')}</span>
                <span className="field-value">
                  {detail.profileScope.join(' · ') || t('audit.archiveWide')}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.executedAt')}</span>
                <span className="field-value mono">
                  {formatDateTime(detail.run.startedAt, language) ??
                    detail.run.startedAt}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.manifestHash')}</span>
                <span className="field-value mono">
                  {detail.manifestHash ?? t('common.notAvailable')}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.manifestPath')}</span>
                <span className="field-value mono">
                  {detail.manifestPath ?? t('common.notAvailable')}
                </span>
              </div>
            </div>
            <div className="detail-divider" />
            <div className="manifest-stats">
              <div className="manifest-stat">
                <span className="dim">{t('audit.newVisits')}</span>
                <span className="mono accent">+{detail.run.newVisits}</span>
              </div>
              <div className="manifest-stat">
                <span className="dim">{t('audit.newUrls')}</span>
                <span className="mono">{detail.run.newUrls}</span>
              </div>
              <div className="manifest-stat">
                <span className="dim">{t('audit.downloads')}</span>
                <span className="mono">{detail.run.newDownloads}</span>
              </div>
              <div className="manifest-stat">
                <span className="dim">{t('audit.profiles')}</span>
                <span className="mono">{detail.run.profilesProcessed}</span>
              </div>
            </div>

            {detail.artifacts.length > 0 && (
              <>
                <div className="detail-divider" />
                <div style={{ marginTop: 'var(--space-3)' }}>
                  <span
                    className="mono-kicker"
                    style={{ marginBottom: 'var(--space-2)', display: 'block' }}
                  >
                    {t('audit.artifacts', { count: detail.artifacts.length })}
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
                          {formatBytes(artifact.sizeBytes ?? 0, language)}
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
                          {t('common.openAction')}
                        </button>
                        <button
                          className="btn-tiny"
                          type="button"
                          onClick={() => {
                            void handleCopyPath(artifact.path)
                          }}
                        >
                          {t('common.copyAction')}
                        </button>
                      </div>
                      {copiedPath === artifact.path && (
                        <span className="dim mono" style={{ fontSize: '10px' }}>
                          {t('audit.copied')}
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              </>
            )}

            {detail.warnings.length > 0 && (
              <>
                <div className="detail-divider" />
                <div className="warning-box">
                  <div className="warning-icon">⚠</div>
                  <div className="warning-text">
                    {detail.warnings.map((warning) => (
                      <div key={warning}>{warning}</div>
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
                    {t('audit.viewManifest')}
                  </button>
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={() => {
                      void handleCopyPath(detail.manifestPath!)
                    }}
                  >
                    {t('audit.copyPath')}
                  </button>
                </>
              )}
            </div>
            {detail.manifestPath && copiedPath === detail.manifestPath && (
              <span className="dim mono" style={{ fontSize: '10px' }}>
                {t('audit.copied')}
              </span>
            )}
          </div>
        </div>
      ) : (
        <EmptyState
          description={t('audit.detailEmptyBody')}
          eyebrow={t('navigation.auditLabel')}
          title={t('audit.detailEmptyTitle')}
        />
      )}
    </section>
  )
}
