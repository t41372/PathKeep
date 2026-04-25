/**
 * This module renders the Audit Ledger route, where runs, artifacts, warnings, and rollback hints stay reviewable instead of hidden behind success toasts.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `AuditPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { formatDateTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  auditSeverity,
  auditSeverityKey,
  runStatusKey,
  runTriggerKey,
  runTypeKey,
  sourceKindFromProfileScope,
} from '../../lib/trust-review'
import { useAuditData } from './hooks/use-audit-data'
import { AuditRunDetailPanel } from './panels/run-detail'
import type { AuditFilterState } from './types'

/**
 * Renders the audit route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Audit expectations in the design docs.
 */
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
  const [filters, setFilters] = useState<AuditFilterState>({
    runType: 'all',
    severity: 'all',
    sourceKind: 'all',
    profileId: 'all',
    artifactType: 'all',
  })

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
  const labels = useMemo(
    () => ({
      commonUnavailable: t('common.unavailable'),
      importPreviewUnavailable: t('audit.importPreviewUnavailable'),
      restoreConfirm: t('import.restoreConfirm'),
      restoreRecorded: t('audit.restoreRecorded'),
      revertConfirm: t('import.revertConfirm'),
      revertRecorded: t('audit.revertRecorded'),
      runDetailUnavailable: t('audit.runDetailUnavailable'),
    }),
    [t],
  )
  const {
    batchActionError,
    batchActionNotice,
    copyFeedback,
    detail,
    detailCache,
    detailSeverity,
    detailTab,
    error,
    handleCopyPath,
    handleExecuteRestore,
    handlePreviewRestore,
    handleRelatedBatchMutation,
    loading,
    loadingRelatedBatch,
    relatedBatchDetail,
    relatedBatchError,
    relatedImportBatch,
    restoreBusy,
    restoreError,
    restoreNotice,
    restorePreview,
    setDetailTab,
  } = useAuditData({
    labels,
    recentImportBatches: snapshot?.recentImportBatches ?? [],
    recentRuns: snapshot?.recentRuns ?? [],
    refreshAppData,
    refreshKey,
    runId,
    selectRun,
  })

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
        if (filters.runType !== 'all' && runType !== filters.runType) {
          return false
        }
        if (
          filters.severity !== 'all' &&
          (!nextDetail || auditSeverity(nextDetail) !== filters.severity)
        ) {
          return false
        }
        const profileScope = nextDetail?.profileScope ?? run.profileScope ?? []
        const sourceKinds = sourceKindFromProfileScope(profileScope)
        if (
          filters.sourceKind !== 'all' &&
          !sourceKinds.includes(filters.sourceKind)
        ) {
          return false
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
        if (
          filters.artifactType !== 'all' &&
          (!nextDetail ||
            !nextDetail.artifacts.some(
              (artifact) => artifact.kind === filters.artifactType,
            ))
        ) {
          return false
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

  /**
   * Explains how source label works.
   *
   * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
   */
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

  /**
   * Explains how restore kind label works.
   *
   * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  function restoreKindLabel(kind: string) {
    return kind === 'archive-safety-snapshot'
      ? t('audit.restoreKindArchiveSafety')
      : t('audit.restoreKindRawSource')
  }

  if (shellLoading && !snapshot) {
    return (
      <section className="page-shell">
        <LoadingState label={t('audit.loadingLedger')} />
      </section>
    )
  }

  if (shellError && !snapshot) {
    return (
      <section className="page-shell">
        <ErrorState
          title={t('audit.unavailableTitle')}
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
                void runBackup().catch(() => undefined)
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
        <AuditRunDetailPanel
          batchActionError={batchActionError}
          batchActionNotice={batchActionNotice}
          copyFeedback={copyFeedback}
          detail={detail}
          detailSeverity={detailSeverity}
          detailTab={detailTab}
          handleCopyPath={handleCopyPath}
          handleExecuteRestore={handleExecuteRestore}
          handlePreviewRestore={handlePreviewRestore}
          handleRelatedBatchMutation={handleRelatedBatchMutation}
          language={language}
          loadingRelatedBatch={loadingRelatedBatch}
          relatedBatchDetail={relatedBatchDetail}
          relatedBatchError={relatedBatchError}
          relatedImportBatch={relatedImportBatch}
          restoreBusy={restoreBusy}
          restoreError={restoreError}
          restoreKindLabel={restoreKindLabel}
          restoreNotice={restoreNotice}
          restorePreview={restorePreview}
          setDetailTab={setDetailTab}
          t={t}
        />
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
