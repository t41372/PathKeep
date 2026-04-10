import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { PreviewEntryList } from '../../components/ui'
import { backend } from '../../lib/backend'
import { formatBytes, formatDateTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  auditSeverity,
  auditSeverityKey,
  importBatchStatusKey,
  importBatchStatusTone,
  runStatusKey,
  runTypeKey,
  runTriggerKey,
  sourceKindFromProfileScope,
} from '../../lib/trust-review'
import type {
  AuditRunDetail,
  ImportBatchDetail,
  ImportBatchOverview,
  SnapshotRestorePreview,
} from '../../lib/types'

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

function parseAuditTimestamp(value?: string | null) {
  if (!value) return Number.NaN
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : Number.NaN
}

function resolveBatchEventTime(
  batch: ImportBatchOverview,
  runType: string,
): number {
  if (runType === 'rollback') {
    return parseAuditTimestamp(
      batch.revertedAt ?? batch.importedAt ?? batch.createdAt,
    )
  }

  return parseAuditTimestamp(
    batch.importedAt ?? batch.revertedAt ?? batch.createdAt,
  )
}

function pickRelatedImportBatch(
  detail: AuditRunDetail | null,
  recentImportBatches: ImportBatchOverview[],
) {
  if (!detail) return null
  const runType = detail.run.runType ?? 'backup'
  if (!['import', 'rollback', 'restore'].includes(runType)) return null

  const runProfileId = detail.profileScope[0] ?? null
  const runTimestamp = parseAuditTimestamp(
    detail.run.finishedAt ?? detail.run.startedAt,
  )
  const sameProfileBatches = recentImportBatches.filter(
    (batch) => !runProfileId || batch.profileId === runProfileId,
  )

  return (
    sameProfileBatches.slice().sort((left, right) => {
      const leftDistance = Math.abs(
        resolveBatchEventTime(left, runType) - runTimestamp,
      )
      const rightDistance = Math.abs(
        resolveBatchEventTime(right, runType) - runTimestamp,
      )
      return leftDistance - rightDistance
    })[0] ?? null
  )
}

export function AuditPage() {
  const {
    error: shellError,
    loading: shellLoading,
    refreshKey,
    refreshAppData,
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
  const [relatedBatchDetail, setRelatedBatchDetail] =
    useState<ImportBatchDetail | null>(null)
  const [relatedBatchError, setRelatedBatchError] = useState<string | null>(
    null,
  )
  const [batchActionError, setBatchActionError] = useState<string | null>(null)
  const [batchActionNotice, setBatchActionNotice] = useState<string | null>(
    null,
  )
  const [filters, setFilters] = useState<AuditFilterState>({
    runType: 'all',
    severity: 'all',
    sourceKind: 'all',
    profileId: 'all',
    artifactType: 'all',
  })
  const [detailTab, setDetailTab] = useState<AuditDetailTab>('summary')
  const [restorePreview, setRestorePreview] =
    useState<SnapshotRestorePreview | null>(null)
  const [restoreError, setRestoreError] = useState<string | null>(null)
  const [restoreNotice, setRestoreNotice] = useState<string | null>(null)
  const [restoreBusy, setRestoreBusy] = useState(false)

  const runIdFromParams = Number(searchParams.get('run') ?? '')
  const runId =
    Number.isFinite(runIdFromParams) && runIdFromParams > 0
      ? runIdFromParams
      : (snapshot?.recentRuns[0]?.id ?? null)
  const selectRun = useCallback(
    (nextRunId: number) => {
      const nextParams = new URLSearchParams(searchParams)
      nextParams.set('run', String(nextRunId))
      setSearchParams(nextParams)
    },
    [searchParams, setSearchParams],
  )

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
  const relatedImportBatch = useMemo(
    () => pickRelatedImportBatch(detail, snapshot?.recentImportBatches ?? []),
    [detail, snapshot?.recentImportBatches],
  )
  const loadingRelatedBatch =
    Boolean(relatedImportBatch) &&
    relatedBatchDetail?.batch.id !== relatedImportBatch?.id &&
    !relatedBatchError

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
    selectRun(filteredRuns[0].id)
  }, [filteredRuns, runId, selectRun])

  useEffect(() => {
    setDetailTab('summary')
    setRestorePreview(null)
    setRestoreError(null)
    setRestoreNotice(null)
  }, [runId])

  useEffect(() => {
    setBatchActionError(null)
    setBatchActionNotice(null)
  }, [runId])

  useEffect(() => {
    if (!relatedImportBatch) {
      setRelatedBatchDetail(null)
      setRelatedBatchError(null)
      return
    }

    let cancelled = false
    const loadRelatedBatch = async () => {
      try {
        setRelatedBatchError(null)
        const response = await backend.previewImportBatch(relatedImportBatch.id)
        if (!cancelled) {
          setRelatedBatchDetail(response)
        }
      } catch (nextError) {
        if (!cancelled) {
          setRelatedBatchDetail(null)
          setRelatedBatchError(
            nextError instanceof Error
              ? nextError.message
              : t('audit.importPreviewUnavailable'),
          )
        }
      }
    }

    void loadRelatedBatch()
    return () => {
      cancelled = true
    }
  }, [relatedImportBatch, t])

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

  async function handleRelatedBatchMutation(action: 'revert' | 'restore') {
    if (!relatedBatchDetail) return
    const message =
      action === 'revert'
        ? t('import.revertConfirm')
        : t('import.restoreConfirm')

    if (typeof window !== 'undefined' && 'confirm' in window) {
      if (!window.confirm(message)) {
        return
      }
    }

    setBatchActionError(null)
    setBatchActionNotice(null)
    try {
      const response =
        action === 'revert'
          ? await backend.revertImportBatch(relatedBatchDetail.batch.id)
          : await backend.restoreImportBatch(relatedBatchDetail.batch.id)
      setRelatedBatchDetail(response)
      setBatchActionNotice(
        action === 'revert'
          ? t('audit.revertRecorded')
          : t('audit.restoreRecorded'),
      )
      await refreshAppData()
    } catch (nextError) {
      setBatchActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
    }
  }

  async function handlePreviewRestore(snapshotPath: string) {
    setRestoreBusy(true)
    setRestoreError(null)
    setRestoreNotice(null)
    try {
      const preview = await backend.previewSnapshotRestore({ snapshotPath })
      setRestorePreview(preview)
    } catch (nextError) {
      setRestorePreview(null)
      setRestoreError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
    } finally {
      setRestoreBusy(false)
    }
  }

  async function handleExecuteRestore() {
    if (!restorePreview?.executeSupported) {
      return
    }
    setRestoreBusy(true)
    setRestoreError(null)
    setRestoreNotice(null)
    try {
      const report = await backend.runSnapshotRestore({
        snapshotPath: restorePreview.snapshotPath,
      })
      await refreshAppData()
      setRestoreNotice(t('audit.restoreRecorded'))
      if (report.run?.id) {
        selectRun(report.run.id)
      }
    } catch (nextError) {
      setRestoreError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
    } finally {
      setRestoreBusy(false)
    }
  }

  function restoreKindLabel(kind: string) {
    return kind === 'archive-safety-snapshot'
      ? t('audit.restoreKindArchiveSafety')
      : t('audit.restoreKindRawSource')
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
          <p className="dashboard-next-action">{t('audit.timelineBody')}</p>
          {filteredRuns.length === 0 ? (
            <p className="dashboard-next-action">{t('audit.noMatchingRuns')}</p>
          ) : (
            <div
              className="chain-viz"
              role="group"
              aria-label={t('audit.manifestChain')}
            >
              {filteredRuns.map((run, index) => {
                const indexedDetail = detailCache[run.id]
                const severity = indexedDetail
                  ? auditSeverity(indexedDetail)
                  : 'clear'
                const triggerLabel = t(
                  runTriggerKey(
                    run.trigger ?? indexedDetail?.trigger ?? 'manual',
                  ),
                )

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
                      className={`chain-block ${run.id === runId ? '' : 'older'}`}
                      type="button"
                      onClick={() => selectRun(run.id)}
                    >
                      <div className="chain-hash mono">#{run.id}</div>
                      <div className="chain-meta dim">
                        <div>
                          {t(runTypeKey(run.runType ?? 'backup'))} ·{' '}
                          {triggerLabel}
                        </div>
                        <div>
                          {t(auditSeverityKey(severity))} ·{' '}
                          {t('dashboard.profilesLabel', {
                            count: run.profilesProcessed,
                          })}
                        </div>
                        <div>
                          {t('audit.deltaNewVisits')}: {run.newVisits} ·{' '}
                          {t('audit.deltaNewUrls')}: {run.newUrls}
                        </div>
                        <div className="mono">
                          {formatDateTime(
                            run.finishedAt ?? run.startedAt,
                            language,
                          ) ??
                            run.finishedAt ??
                            run.startedAt}
                        </div>
                      </div>
                    </button>
                  </div>
                )
              })}
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
                <StatusCallout
                  tone="info"
                  title={t('audit.reviewGuideTitle')}
                  body={t('audit.reviewGuideBody')}
                />
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
                    <span className="field-label">{t('common.status')}</span>
                    <span className="field-value">
                      {t(runStatusKey(detail.run.status))}
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
                      {t('audit.triggerLabel')}
                    </span>
                    <span className="field-value">
                      {t(runTriggerKey(detail.trigger ?? detail.run.trigger))}
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
                  {relatedBatchDetail ? (
                    <>
                      <div className="manifest-stat">
                        <span className="dim">{t('audit.visibleRecords')}</span>
                        <span className="mono">
                          {relatedBatchDetail.batch.visibleItems}
                        </span>
                      </div>
                      <div className="manifest-stat">
                        <span className="dim">
                          {t('audit.revertedRecords')}
                        </span>
                        <span className="mono">
                          {Math.max(
                            0,
                            relatedBatchDetail.batch.importedItems -
                              relatedBatchDetail.batch.visibleItems,
                          )}
                        </span>
                      </div>
                    </>
                  ) : null}
                </div>
                <div className="detail-divider" />
                <div className="audit-review-section">
                  <div className="audit-review-header">
                    <span className="mono-kicker">
                      {t('audit.changedRecordsTitle')}
                    </span>
                    <span className="panel-action">
                      {relatedImportBatch
                        ? t('audit.importBatchLabel', {
                            id: String(relatedImportBatch.id),
                          })
                        : t('audit.changePreviewUnavailableShort')}
                    </span>
                  </div>
                  <p className="dashboard-next-action">
                    {t('audit.changedRecordsBody')}
                  </p>
                  {loadingRelatedBatch ? (
                    <p className="dim">{t('common.loading')}</p>
                  ) : relatedBatchError ? (
                    <StatusCallout
                      tone="warning"
                      title={t('audit.importPreviewUnavailable')}
                      body={relatedBatchError}
                    />
                  ) : relatedBatchDetail ? (
                    <>
                      <div className="manifest-grid">
                        <div className="manifest-field">
                          <span className="field-label">
                            {t('import.candidateRows')}
                          </span>
                          <span className="field-value mono">
                            {relatedBatchDetail.batch.candidateItems.toLocaleString(
                              language,
                            )}
                          </span>
                        </div>
                        <div className="manifest-field">
                          <span className="field-label">
                            {t('import.importedRows')}
                          </span>
                          <span className="field-value mono">
                            {relatedBatchDetail.batch.importedItems.toLocaleString(
                              language,
                            )}
                          </span>
                        </div>
                        <div className="manifest-field">
                          <span className="field-label">
                            {t('import.duplicateRows')}
                          </span>
                          <span className="field-value mono">
                            {relatedBatchDetail.batch.duplicateItems.toLocaleString(
                              language,
                            )}
                          </span>
                        </div>
                        <div className="manifest-field">
                          <span className="field-label">
                            {t('import.visibleRows')}
                          </span>
                          <span className="field-value mono">
                            {relatedBatchDetail.batch.visibleItems.toLocaleString(
                              language,
                            )}
                          </span>
                        </div>
                      </div>
                      <div className="detail-divider" />
                      <PreviewEntryList
                        entries={relatedBatchDetail.previewEntries}
                        language={language}
                        statusLabel={(status) =>
                          t(importBatchStatusKey(status))
                        }
                        statusTone={importBatchStatusTone}
                      />
                      <div className="wizard-actions">
                        <Link
                          className="btn-secondary"
                          to={`/import?batch=${relatedBatchDetail.batch.id}`}
                        >
                          {t('audit.openImportReview')}
                        </Link>
                        {relatedBatchDetail.batch.auditPath ? (
                          <button
                            className="btn-secondary"
                            type="button"
                            onClick={() => {
                              void backend.openPathInFileManager(
                                relatedBatchDetail.batch.auditPath ?? '',
                              )
                            }}
                          >
                            {t('audit.openImportArtifact')}
                          </button>
                        ) : null}
                        <button
                          className="btn-secondary"
                          type="button"
                          onClick={() => {
                            void handleRelatedBatchMutation('revert')
                          }}
                          disabled={
                            relatedBatchDetail.batch.status === 'reverted'
                          }
                        >
                          {t('import.revertBatch')}
                        </button>
                        <button
                          className="btn-secondary"
                          type="button"
                          onClick={() => {
                            void handleRelatedBatchMutation('restore')
                          }}
                          disabled={
                            relatedBatchDetail.batch.status !== 'reverted'
                          }
                        >
                          {t('import.restoreBatch')}
                        </button>
                      </div>
                      <p className="mono-support">
                        {t(
                          importBatchStatusKey(relatedBatchDetail.batch.status),
                        )}
                      </p>
                      {batchActionNotice ? (
                        <p className="mono-support" role="status">
                          {batchActionNotice}
                        </p>
                      ) : null}
                      {batchActionError ? (
                        <p className="inline-error" role="alert">
                          {batchActionError}
                        </p>
                      ) : null}
                    </>
                  ) : (
                    <StatusCallout
                      tone="info"
                      title={t('audit.changePreviewUnavailableTitle')}
                      body={t('audit.changePreviewUnavailableBody')}
                    />
                  )}
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
                        {artifact.kind === 'snapshot' ? (
                          <button
                            className="btn-tiny"
                            type="button"
                            onClick={() => {
                              void handlePreviewRestore(artifact.path)
                            }}
                          >
                            {restoreBusy &&
                            restorePreview?.snapshotPath === artifact.path
                              ? t('common.loading')
                              : t('audit.previewRestore')}
                          </button>
                        ) : null}
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
                {restorePreview ? (
                  <div
                    className="panel"
                    style={{ marginTop: 'var(--space-4)' }}
                  >
                    <div className="panel-header">
                      <span className="panel-title">
                        {t('audit.restorePreviewTitle')}
                      </span>
                      <span className="panel-action">
                        {restorePreview.executeSupported
                          ? t('audit.restoreReady')
                          : t('audit.restoreManualOnly')}
                      </span>
                    </div>
                    <div className="panel-body">
                      <p className="dashboard-next-action">
                        {t('audit.restorePreviewBody')}
                      </p>
                      <div className="manifest-grid">
                        <div className="manifest-field">
                          <span className="field-label">
                            {t('audit.restoreKind')}
                          </span>
                          <span className="field-value">
                            {restoreKindLabel(restorePreview.snapshotKind)}
                          </span>
                        </div>
                        <div className="manifest-field">
                          <span className="field-label">
                            {t('audit.runSource')}
                          </span>
                          <span className="field-value mono">
                            {restorePreview.sourceProfileId ??
                              t('audit.archiveWide')}
                          </span>
                        </div>
                        <div className="manifest-field">
                          <span className="field-label">
                            {t('audit.executedAt')}
                          </span>
                          <span className="field-value mono">
                            {restorePreview.createdAt
                              ? (formatDateTime(
                                  restorePreview.createdAt,
                                  language,
                                ) ?? restorePreview.createdAt)
                              : t('common.notAvailable')}
                          </span>
                        </div>
                        <div className="manifest-field">
                          <span className="field-label">
                            {t('audit.restoreSnapshotPath')}
                          </span>
                          <span className="field-value mono">
                            {restorePreview.snapshotPath}
                          </span>
                        </div>
                      </div>
                      <div className="detail-divider" />
                      <div className="manifest-stats">
                        <div className="manifest-stat">
                          <span className="dim">
                            {t('audit.estimatedVisits')}
                          </span>
                          <span className="mono accent">
                            {restorePreview.estimatedVisits}
                          </span>
                        </div>
                        <div className="manifest-stat">
                          <span className="dim">
                            {t('audit.estimatedUrls')}
                          </span>
                          <span className="mono">
                            {restorePreview.estimatedUrls}
                          </span>
                        </div>
                        <div className="manifest-stat">
                          <span className="dim">
                            {t('audit.estimatedDownloads')}
                          </span>
                          <span className="mono">
                            {restorePreview.estimatedDownloads}
                          </span>
                        </div>
                      </div>
                      {restorePreview.warnings.length > 0 ? (
                        <div
                          className="warning-box"
                          style={{ marginTop: 'var(--space-3)' }}
                        >
                          <div className="warning-icon">⚠</div>
                          <div className="warning-text">
                            {restorePreview.warnings.map((warning) => (
                              <div key={warning}>{warning}</div>
                            ))}
                          </div>
                        </div>
                      ) : null}
                      <div
                        className="wizard-actions"
                        style={{ marginTop: 'var(--space-3)' }}
                      >
                        <button
                          className="btn-secondary"
                          type="button"
                          onClick={() => {
                            void backend.openPathInFileManager(
                              restorePreview.snapshotPath,
                            )
                          }}
                        >
                          {t('common.openAction')}
                        </button>
                        <button
                          className="btn-primary"
                          type="button"
                          disabled={
                            !restorePreview.executeSupported || restoreBusy
                          }
                          onClick={() => {
                            void handleExecuteRestore()
                          }}
                        >
                          {restoreBusy
                            ? t('common.loading')
                            : t('audit.executeRestore')}
                        </button>
                      </div>
                      {restoreNotice ? (
                        <p className="mono-support" role="status">
                          {restoreNotice}
                        </p>
                      ) : null}
                    </div>
                  </div>
                ) : null}
                {restoreError ? (
                  <p className="inline-error" role="alert">
                    {restoreError}
                  </p>
                ) : null}
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
