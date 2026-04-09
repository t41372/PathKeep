import { useEffect, useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'
import { formatBytes, formatDateTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  auditSeverity,
  auditSeverityKey,
  runStatusKey,
  runTypeKey,
  runTriggerKey,
  sourceKindFromProfileScope,
} from '../../lib/trust-review'
import type { AuditRunDetail } from '../../lib/types'

interface AuditDetailState {
  runId: number | null
  detail: AuditRunDetail | null
  error: string | null
}

interface AuditFilterState {
  runType: string
  severity: 'all' | 'clear' | 'warning' | 'blocked'
  sourceKind: string
  profileId: string
  artifactType: string
}

type AuditDetailTab = 'summary' | 'artifacts' | 'warnings'

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
  const [detailCache, setDetailCache] = useState<
    Record<number, AuditRunDetail>
  >({})
  const [filters, setFilters] = useState<AuditFilterState>({
    runType: 'all',
    severity: 'all',
    sourceKind: 'all',
    profileId: 'all',
    artifactType: 'all',
  })
  const [detailTab, setDetailTab] = useState<AuditDetailTab>('summary')

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

  useEffect(() => {
    const runs = snapshot?.recentRuns ?? []
    if (!runs.length) {
      setDetailCache({})
      return
    }

    let cancelled = false
    const loadRunIndex = async () => {
      try {
        const entries = await Promise.all(
          runs.map(
            async (run) =>
              [run.id, await backend.loadAuditRunDetail(run.id)] as const,
          ),
        )
        if (cancelled) {
          return
        }
        const nextCache: Record<number, AuditRunDetail> = {}
        for (const [nextRunId, nextDetail] of entries) {
          nextCache[nextRunId] = nextDetail
        }
        setDetailCache(nextCache)
      } catch {
        if (!cancelled) {
          setDetailCache({})
        }
      }
    }

    void loadRunIndex()
    return () => {
      cancelled = true
    }
  }, [refreshKey, snapshot?.recentRuns])

  const detail = detailState.runId === runId ? detailState.detail : null
  const error = detailState.runId === runId ? detailState.error : null
  const loading = Boolean(runId) && detailState.runId !== runId
  const indexedRuns = useMemo(() => snapshot?.recentRuns ?? [], [snapshot])
  const profileOptions = useMemo(() => {
    const nextProfiles = new Set<string>()
    for (const nextDetail of Object.values(detailCache)) {
      if (nextDetail.profileScope.length === 0) {
        nextProfiles.add('archive-wide')
        continue
      }
      for (const profileId of nextDetail.profileScope) {
        nextProfiles.add(profileId)
      }
    }
    return Array.from(nextProfiles).sort()
  }, [detailCache])
  const runTypeOptions = useMemo(() => {
    const nextRunTypes = new Set<string>()
    for (const run of indexedRuns) {
      nextRunTypes.add(run.runType ?? 'backup')
    }
    return Array.from(nextRunTypes)
  }, [indexedRuns])
  const sourceOptions = useMemo(() => {
    const nextSources = new Set<string>()
    for (const run of indexedRuns) {
      const nextDetail = detailCache[run.id]
      for (const sourceKind of sourceKindFromProfileScope(
        nextDetail?.profileScope ?? run.profileScope ?? [],
      )) {
        nextSources.add(sourceKind)
      }
    }
    return Array.from(nextSources).sort()
  }, [detailCache, indexedRuns])
  const artifactOptions = useMemo(() => {
    const nextArtifacts = new Set<string>()
    for (const nextDetail of Object.values(detailCache)) {
      for (const artifact of nextDetail.artifacts) {
        nextArtifacts.add(artifact.kind)
      }
    }
    return Array.from(nextArtifacts).sort()
  }, [detailCache])
  const filteredRuns = useMemo(
    () =>
      indexedRuns.filter((run) => {
        const runType = run.runType ?? 'backup'
        const nextDetail = detailCache[run.id]
        if (filters.runType !== 'all') {
          if (runType !== filters.runType) {
            return false
          }
        }
        if (filters.severity !== 'all') {
          if (!nextDetail || auditSeverity(nextDetail) !== filters.severity) {
            return false
          }
        }
        const profileScope = nextDetail?.profileScope ?? run.profileScope ?? []
        const sourceKinds = sourceKindFromProfileScope(profileScope)
        if (filters.sourceKind !== 'all') {
          if (!sourceKinds.includes(filters.sourceKind)) {
            return false
          }
        }
        if (filters.profileId !== 'all') {
          const matchesProfile =
            profileScope.length === 0
              ? filters.profileId === 'archive-wide'
              : profileScope.includes(filters.profileId)
          if (!matchesProfile) {
            return false
          }
        }
        if (filters.artifactType !== 'all') {
          if (
            !nextDetail ||
            !nextDetail.artifacts.some(
              (artifact) => artifact.kind === filters.artifactType,
            )
          ) {
            return false
          }
        }
        return true
      }),
    [detailCache, filters, indexedRuns],
  )
  const selectedRunIndex = filteredRuns.findIndex((run) => run.id === runId)
  const previousVisibleRun =
    selectedRunIndex >= 0 ? (filteredRuns[selectedRunIndex + 1] ?? null) : null
  const deltaRows =
    detail && previousVisibleRun
      ? [
          {
            label: t('audit.deltaNewVisits'),
            value: detail.run.newVisits - previousVisibleRun.newVisits,
          },
          {
            label: t('audit.deltaNewUrls'),
            value: detail.run.newUrls - previousVisibleRun.newUrls,
          },
          {
            label: t('audit.deltaDownloads'),
            value: detail.run.newDownloads - previousVisibleRun.newDownloads,
          },
          {
            label: t('audit.deltaProfiles'),
            value:
              detail.run.profilesProcessed -
              previousVisibleRun.profilesProcessed,
          },
        ]
      : []
  const filtersLoading =
    indexedRuns.length > 0 &&
    Object.keys(detailCache).length < indexedRuns.length
  const detailSeverity = detail ? auditSeverity(detail) : null

  function sourceLabel(sourceKind: string) {
    if (sourceKind === 'chrome') return t('audit.sourceChrome')
    if (sourceKind === 'firefox') return t('audit.sourceFirefox')
    if (sourceKind === 'safari') return t('audit.sourceSafari')
    if (sourceKind === 'takeout') return t('audit.sourceTakeout')
    if (sourceKind === 'archive-wide') return t('audit.archiveWide')
    return sourceKind
  }

  useEffect(() => {
    if (!filteredRuns.length || !runId) {
      return
    }
    if (filteredRuns.some((run) => run.id === runId)) {
      return
    }
    const nextParams = new URLSearchParams(searchParams)
    nextParams.set('run', String(filteredRuns[0].id))
    setSearchParams(nextParams)
  }, [filteredRuns, runId, searchParams, setSearchParams])

  useEffect(() => {
    setDetailTab('summary')
  }, [runId])

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
          <span className="panel-title">{t('audit.filterLabel')}</span>
          <span className="panel-action">{t('audit.filterDescription')}</span>
        </div>
        <div className="panel-body">
          <div className="audit-filter-grid">
            <label className="field-stack">
              <span className="mono-kicker">{t('audit.filterRunType')}</span>
              <select
                aria-label={t('audit.filterRunType')}
                value={filters.runType}
                onChange={(event) => {
                  setFilters((current) => ({
                    ...current,
                    runType: event.target.value,
                  }))
                }}
              >
                <option value="all">{t('audit.allRunTypes')}</option>
                {runTypeOptions.map((runType) => (
                  <option key={runType} value={runType}>
                    {t(runTypeKey(runType))}
                  </option>
                ))}
              </select>
            </label>
            <label className="field-stack">
              <span className="mono-kicker">{t('audit.filterSeverity')}</span>
              <select
                aria-label={t('audit.filterSeverity')}
                value={filters.severity}
                onChange={(event) => {
                  setFilters((current) => ({
                    ...current,
                    severity: event.target
                      .value as AuditFilterState['severity'],
                  }))
                }}
              >
                <option value="all">{t('audit.allSeverities')}</option>
                <option value="clear">{t('common.statusClear')}</option>
                <option value="warning">
                  {t('common.statusNeedsAttention')}
                </option>
                <option value="blocked">{t('common.statusBlocked')}</option>
              </select>
            </label>
            <label className="field-stack">
              <span className="mono-kicker">{t('audit.filterSource')}</span>
              <select
                aria-label={t('audit.filterSource')}
                value={filters.sourceKind}
                onChange={(event) => {
                  setFilters((current) => ({
                    ...current,
                    sourceKind: event.target.value,
                  }))
                }}
              >
                <option value="all">{t('audit.allSources')}</option>
                {sourceOptions.map((sourceKind) => (
                  <option key={sourceKind} value={sourceKind}>
                    {sourceLabel(sourceKind)}
                  </option>
                ))}
              </select>
            </label>
            <label className="field-stack">
              <span className="mono-kicker">{t('audit.filterProfile')}</span>
              <select
                aria-label={t('audit.filterProfile')}
                value={filters.profileId}
                onChange={(event) => {
                  setFilters((current) => ({
                    ...current,
                    profileId: event.target.value,
                  }))
                }}
              >
                <option value="all">{t('audit.allProfiles')}</option>
                {profileOptions.map((profileId) => (
                  <option key={profileId} value={profileId}>
                    {profileId === 'archive-wide'
                      ? t('audit.archiveWide')
                      : profileId}
                  </option>
                ))}
              </select>
            </label>
            <label className="field-stack">
              <span className="mono-kicker">
                {t('audit.filterArtifactType')}
              </span>
              <select
                aria-label={t('audit.filterArtifactType')}
                value={filters.artifactType}
                onChange={(event) => {
                  setFilters((current) => ({
                    ...current,
                    artifactType: event.target.value,
                  }))
                }}
              >
                <option value="all">{t('audit.allArtifactTypes')}</option>
                {artifactOptions.map((artifactType) => (
                  <option key={artifactType} value={artifactType}>
                    {artifactType}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {filtersLoading ? (
            <p className="dashboard-next-action">{t('audit.filtersLoading')}</p>
          ) : null}
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('audit.manifestChain')}</span>
          <span className="panel-action">{t('audit.verifyIntegrity')}</span>
        </div>
        <div className="panel-body">
          {filteredRuns.length === 0 ? (
            <p className="dashboard-next-action">{t('audit.noMatchingRuns')}</p>
          ) : (
            <div
              className="chain-viz"
              role="group"
              aria-label={t('audit.manifestChain')}
            >
              {filteredRuns.slice(0, 4).map((run, index) => {
                const indexedDetail = detailCache[run.id]
                const severity = indexedDetail
                  ? auditSeverity(indexedDetail)
                  : 'clear'

                return (
                  <div key={run.id} style={{ display: 'contents' }}>
                    {index > 0 && (
                      <div className="chain-link" aria-hidden="true">
                        →
                      </div>
                    )}
                    <button
                      aria-label={`#${run.id} · ${t(runTypeKey(run.runType ?? 'backup'))} · ${t(runTriggerKey(run.trigger ?? indexedDetail?.trigger ?? 'manual'))} · ${t(runStatusKey(run.status))} · ${t(auditSeverityKey(severity))}`}
                      aria-pressed={run.id === runId}
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
                          {t(runTypeKey(run.runType ?? 'backup'))} ·{' '}
                          {t(
                            runTriggerKey(
                              run.trigger ?? indexedDetail?.trigger ?? 'manual',
                            ),
                          )}
                        </div>
                        <div>
                          {t(auditSeverityKey(severity))} ·{' '}
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
                )
              })}
              {filteredRuns.length > 4 && (
                <div className="chain-link dim" aria-hidden="true">
                  → ···
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('audit.deltaTitle')}</span>
          <span className="panel-action">
            {previousVisibleRun
              ? t('audit.deltaComparedToRun', { runId: previousVisibleRun.id })
              : t('common.pending')}
          </span>
        </div>
        <div className="panel-body">
          <p className="dashboard-next-action">{t('audit.deltaBody')}</p>
          {deltaRows.length > 0 ? (
            <div className="manifest-stats">
              {deltaRows.map((row) => (
                <div key={row.label} className="manifest-stat">
                  <span className="dim">{row.label}</span>
                  <span
                    className={`mono ${
                      row.value > 0
                        ? 'accent'
                        : row.value < 0
                          ? 'warning-text'
                          : 'dim'
                    }`}
                  >
                    {row.value > 0 ? '+' : ''}
                    {row.value}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <p className="dim">{t('audit.deltaUnavailable')}</p>
          )}
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
            {detailSeverity ? (
              <span className="panel-action">
                {t(auditSeverityKey(detailSeverity))}
              </span>
            ) : null}
          </div>
          <div className="panel-body">
            <div className="pme-tabs">
              {(
                [
                  ['summary', t('audit.summaryTab')],
                  ['artifacts', t('audit.artifactsTab')],
                  ['warnings', t('audit.warningsTab')],
                ] as const
              ).map(([tab, label]) => (
                <button
                  key={tab}
                  className={`pme-tab ${detailTab === tab ? 'active' : ''}`}
                  type="button"
                  onClick={() => setDetailTab(tab)}
                >
                  {label}
                </button>
              ))}
            </div>

            {detailTab === 'summary' ? (
              <>
                <div className="manifest-grid">
                  <div className="manifest-field">
                    <span className="field-label">{t('audit.runId')}</span>
                    <span className="field-value mono">#{detail.run.id}</span>
                  </div>
                  <div className="manifest-field">
                    <span className="field-label">{t('audit.runType')}</span>
                    <span className="field-value">
                      {t(runTypeKey(detail.run.runType ?? 'backup'))}
                    </span>
                  </div>
                  <div className="manifest-field">
                    <span className="field-label">{t('audit.runSource')}</span>
                    <span className="field-value">
                      {detail.profileScope.join(' · ') ||
                        t('audit.archiveWide')}
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
                    <span className="field-label">
                      {t('audit.manifestHash')}
                    </span>
                    <span className="field-value mono">
                      {detail.manifestHash ?? t('common.notAvailable')}
                    </span>
                  </div>
                  <div className="manifest-field">
                    <span className="field-label">
                      {t('audit.manifestPath')}
                    </span>
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
              </>
            ) : null}

            {detailTab === 'artifacts' ? (
              <div style={{ marginTop: 'var(--space-3)' }}>
                <span
                  className="mono-kicker"
                  style={{ marginBottom: 'var(--space-2)', display: 'block' }}
                >
                  {t('audit.artifacts', { count: detail.artifacts.length })}
                </span>
                {detail.artifacts.length > 0 ? (
                  detail.artifacts.map((artifact) => (
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
                      {copiedPath === artifact.path ? (
                        <span className="dim mono" style={{ fontSize: '10px' }}>
                          {t('audit.copied')}
                        </span>
                      ) : null}
                    </div>
                  ))
                ) : (
                  <p className="dashboard-next-action">
                    {t('common.notAvailable')}
                  </p>
                )}
              </div>
            ) : null}

            {detailTab === 'warnings' ? (
              detail.warnings.length > 0 ? (
                <div
                  className="warning-box"
                  style={{ marginTop: 'var(--space-3)' }}
                >
                  <div className="warning-icon">⚠</div>
                  <div className="warning-text">
                    {detail.warnings.map((warning) => (
                      <div key={warning}>{warning}</div>
                    ))}
                  </div>
                </div>
              ) : (
                <p
                  className="dashboard-next-action"
                  style={{ marginTop: 'var(--space-3)' }}
                >
                  {t('audit.noWarnings')}
                </p>
              )
            ) : null}

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
