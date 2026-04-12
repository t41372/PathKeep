/**
 * This module renders the Insights route and keeps deterministic, evidence-first insight surfaces aligned with shared profile scope.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `InsightsPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useEffect, useEffectEvent, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { SkeletonInsights } from '../../components/primitives/skeleton'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import {
  calendarDayKey,
  formatBytes,
  formatDateTime,
  formatRelativeTime,
} from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  compactInsightText,
  formatInsightCoverage,
  runtimeJobMutationNeedsRefresh,
} from '../../lib/intelligence-presentation'
import {
  resolveInsightOnThisDay,
  resolveInsightPeriodicSummary,
  resolveInsightTopDomains,
} from '../../lib/insight-canonical'
import {
  aiStatusMeta,
  assistantHref,
  evidenceHref,
} from '../../lib/intelligence'
import {
  deterministicModuleLabel,
  deterministicModuleStatusLabel,
  enrichmentPluginLabel,
  intelligenceRuntimeJobStateLabel,
} from '../../lib/intelligence-runtime'
import {
  profileIdLabel,
  useProfileScope,
} from '../../lib/profile-scope-context'
import {
  storageAnalyticsSlices,
  storageGrowthEvidence,
} from '../../lib/storage-analytics'
import type {
  DeterministicRebuildQueueReport,
  InsightEvidenceItem,
  InsightExplanation,
  InsightSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'

const topicColors = ['#FF7832', '#4ECDC4', '#FFE66D', '#FF6B6B', '#89CFF0']

/**
 * Explains how query stage label works.
 *
 * Keeping this as a named declaration makes the Insights surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function queryStageLabel(
  stage: string,
  t: (key: string, values?: Record<string, number | string>) => string,
) {
  switch (stage) {
    case 'compare':
      return t('queryStageCompare')
    case 'site-restrict':
      return t('queryStageSiteRestrict')
    case 'error-driven':
      return t('queryStageErrorDriven')
    case 'narrowing':
      return t('queryStageNarrowing')
    case 'broadening':
      return t('queryStageBroadening')
    default:
      return t('queryStageBroad')
  }
}

/**
 * Renders the insights route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Insights expectations in the design docs.
 */
export function InsightsPage() {
  const { language, ns } = useI18n()
  const { dashboard, refreshAppData, refreshKey, snapshot } = useShellData()
  const { activeProfileId } = useProfileScope()
  const [insights, setInsights] = useState<InsightSnapshot | null>(null)
  const [explanation, setExplanation] = useState<InsightExplanation | null>(
    null,
  )
  const [selectedInsight, setSelectedInsight] = useState<{
    id: string
    kind: string
    title: string
    profileId?: string | null
    windowDays?: number
  } | null>(null)
  const [action, setAction] = useState<string | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [runtime, setRuntime] = useState<IntelligenceRuntimeSnapshot | null>(
    null,
  )
  const [refreshQueueReport, setRefreshQueueReport] =
    useState<DeterministicRebuildQueueReport | null>(null)
  const [runtimeError, setRuntimeError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const insightsT = ns('insights')
  const intelligenceT = ns('intelligence')
  const commonT = ns('common')
  const settingsT = ns('settings')
  useEffect(() => {
    let cancelled = false
    setLoading(true)
    setInsights(null)
    setSelectedInsight(null)
    setExplanation(null)
    setLoadError(null)
    /**
     * Explains how load works.
     *
     * Keeping this as a named declaration makes the Insights surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const load = async () => {
      try {
        const result = await backend.loadInsights({
          fullRebuild: false,
          profileId: activeProfileId,
        })
        if (!cancelled) {
          setInsights(result)
          setLoadError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setInsights(null)
          setSelectedInsight(null)
          setExplanation(null)
          setLoadError(
            error instanceof Error ? error.message : insightsT('loadingLabel'),
          )
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    void load()
    return () => {
      cancelled = true
    }
  }, [activeProfileId, insightsT, refreshKey])

  useEffect(() => {
    let cancelled = false
    /**
     * Loads runtime.
     *
     * Keeping this as a named declaration makes the Insights surface easier to review and test than burying the behavior inside another anonymous callback.
     */
    const loadRuntime = async () => {
      try {
        const nextRuntime = await backend.loadIntelligenceRuntime()
        if (!cancelled) {
          setRuntime(nextRuntime)
          setRuntimeError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setRuntime(null)
          setRuntimeError(
            error instanceof Error ? error.message : commonT('notAvailable'),
          )
        }
      }
    }

    void loadRuntime()
    return () => {
      cancelled = true
    }
  }, [commonT, refreshKey])

  const pollQueuedRefreshJob = useEffectEvent(async (jobId: number) => {
    try {
      const nextRuntime = await backend.loadIntelligenceRuntime()
      setRuntime(nextRuntime)
      setRuntimeError(null)
      const trackedJob = nextRuntime.recentJobs.find((job) => job.id === jobId)
      if (trackedJob && ['queued', 'running'].includes(trackedJob.state)) {
        return false
      }
      await refreshAppData()
      setRefreshQueueReport(null)
      return true
    } catch (error) {
      setRuntimeError(
        error instanceof Error ? error.message : commonT('notAvailable'),
      )
      return false
    }
  })

  useEffect(() => {
    const jobId = refreshQueueReport?.jobId
    if (!jobId) {
      return
    }
    let cancelled = false
    let timer: number | null = null
    const schedule = (delayMs: number) => {
      timer = window.setTimeout(() => {
        void tick()
      }, delayMs)
    }
    const tick = async () => {
      const completed = await pollQueuedRefreshJob(jobId)
      if (!cancelled && !completed) {
        schedule(3000)
      }
    }
    schedule(3000)
    return () => {
      cancelled = true
      if (timer !== null) {
        window.clearTimeout(timer)
      }
    }
  }, [refreshQueueReport?.jobId])

  const aiMeta = snapshot
    ? aiStatusMeta(snapshot.aiStatus, intelligenceT)
    : null
  const todayKey = calendarDayKey(new Date())
  const onThisDay = useMemo(
    () =>
      insights
        ? resolveInsightOnThisDay(insights, todayKey).filter((item) => {
            const visitedYear = new Date(item.visitedAt).getFullYear()
            return visitedYear < new Date().getFullYear()
          })
        : [],
    [insights, todayKey],
  )
  const siteAnalytics = useMemo(
    () => (insights ? resolveInsightTopDomains(insights) : []),
    [insights],
  )
  const periodicSummary = useMemo(() => {
    if (!insights) return []
    return resolveInsightPeriodicSummary(insights, insightsT)
  }, [insights, insightsT])
  const storageEvidence = useMemo(
    () => storageGrowthEvidence(dashboard),
    [dashboard],
  )
  const storageSlices = useMemo(
    () => (dashboard ? storageAnalyticsSlices(dashboard.storage) : []),
    [dashboard],
  )
  const runtimeLeadJob =
    runtime?.recentJobs.find((job) => job.state === 'running') ??
    runtime?.recentJobs.find((job) => job.state === 'failed') ??
    runtime?.recentJobs.find((job) => job.state === 'queued') ??
    null

  /**
   * Handles refresh insights.
   *
   * Keeping this as a named declaration makes the Insights surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleRefreshInsights() {
    setAction(insightsT('refreshingAction'))
    setLoadError(null)
    try {
      const report = await backend.queueInsightsRebuild({
        fullRebuild: false,
        profileId: activeProfileId,
      })
      setRefreshQueueReport(report)
      const nextRuntime = await backend.loadIntelligenceRuntime()
      setRuntime(nextRuntime)
      setRuntimeError(null)
      await refreshAppData()
    } catch (error) {
      setLoadError(
        error instanceof Error
          ? error.message
          : insightsT('refreshAttentionTitle'),
      )
    } finally {
      setAction(null)
    }
  }

  /**
   * Handles retry runtime job.
   *
   * Keeping this as a named declaration makes the Insights surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleRetryRuntimeJob(jobId: number) {
    setAction(settingsT('retryRuntimeJob'))
    try {
      const nextRuntime = await backend.retryIntelligenceJob(jobId)
      setRuntime(nextRuntime)
      setRuntimeError(null)
    } catch (error) {
      const message =
        error instanceof Error ? error.message : commonT('notAvailable')
      if (runtimeJobMutationNeedsRefresh(message)) {
        try {
          const refreshedRuntime = await backend.loadIntelligenceRuntime()
          setRuntime(refreshedRuntime)
          setRuntimeError(null)
          return
        } catch (refreshError) {
          setRuntimeError(
            refreshError instanceof Error
              ? refreshError.message
              : commonT('notAvailable'),
          )
          return
        }
      }
      setRuntimeError(message)
    } finally {
      setAction(null)
    }
  }

  /**
   * Handles cancel runtime job.
   *
   * Keeping this as a named declaration makes the Insights surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleCancelRuntimeJob(jobId: number) {
    setAction(settingsT('cancelRuntimeJob'))
    try {
      const nextRuntime = await backend.cancelIntelligenceJob(jobId)
      setRuntime(nextRuntime)
      setRuntimeError(null)
    } catch (error) {
      const message =
        error instanceof Error ? error.message : commonT('notAvailable')
      if (runtimeJobMutationNeedsRefresh(message)) {
        try {
          const refreshedRuntime = await backend.loadIntelligenceRuntime()
          setRuntime(refreshedRuntime)
          setRuntimeError(null)
          return
        } catch (refreshError) {
          setRuntimeError(
            refreshError instanceof Error
              ? refreshError.message
              : commonT('notAvailable'),
          )
          return
        }
      }
      setRuntimeError(message)
    } finally {
      setAction(null)
    }
  }

  /**
   * Handles explain.
   *
   * Keeping this as a named declaration makes the Insights surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleExplain(input: {
    id: string
    kind: string
    title: string
    profileId?: string | null
    windowDays?: number
  }) {
    setAction(insightsT('explainingAction'))
    setLoadError(null)
    setSelectedInsight(input)
    setExplanation(null)
    try {
      const nextExplanation = await backend.explainInsight({
        insightId: input.id,
        insightKind: input.kind,
        profileId: input.profileId ?? activeProfileId,
        windowDays: input.windowDays,
      })
      setExplanation(nextExplanation)
    } catch (error) {
      setExplanation(null)
      setLoadError(
        error instanceof Error ? error.message : insightsT('explainability'),
      )
    } finally {
      setAction(null)
    }
  }

  if (!snapshot?.config.initialized) {
    return (
      <section className="page-shell">
        <EmptyState
          description={insightsT('archiveNotInitializedDescription')}
          eyebrow={insightsT('intelligenceEyebrow')}
          title={insightsT('archiveNotInitializedTitle')}
          action={
            <Link className="btn-primary" to="/onboarding">
              {insightsT('goToSetup')}
            </Link>
          }
        />
      </section>
    )
  }

  if (loading && !insights) {
    return <SkeletonInsights label={commonT('loadingInsights')} />
  }

  if (loadError && !insights) {
    return (
      <section className="page-shell">
        <ErrorState
          title={insightsT('unavailableTitle')}
          description={loadError}
        />
      </section>
    )
  }

  if (!insights) {
    return (
      <section className="page-shell">
        <EmptyState
          description={insightsT('emptyDescription')}
          eyebrow={insightsT('intelligenceEyebrow')}
          title={insightsT('emptyTitle')}
          action={
            <Link className="btn-secondary" to="/explorer">
              {insightsT('openExplorer')}
            </Link>
          }
        />
      </section>
    )
  }

  return (
    <section className="page-shell insights-page" data-testid="insights-page">
      {activeProfileId ? (
        <StatusCallout
          tone="info"
          eyebrow={commonT('profileScope')}
          title={insightsT('scopedViewTitle')}
          body={insightsT('scopedViewBody', {
            profile: profileIdLabel(activeProfileId),
          })}
        />
      ) : null}

      {loadError ? (
        <ErrorState
          title={insightsT('refreshAttentionTitle')}
          description={loadError}
        />
      ) : null}

      {refreshQueueReport ? (
        <StatusCallout
          tone="info"
          title={insightsT('refreshQueuedTitle')}
          body={insightsT('refreshQueuedBody', {
            jobId: refreshQueueReport.jobId,
          })}
          actions={
            <div className="intelligence-actions">
              <Link className="btn-secondary" to="/jobs">
                {settingsT('runtimeQueueTitle')}
              </Link>
            </div>
          }
        />
      ) : null}

      {action ? (
        <LoadingState
          compact
          label={action}
          detail={
            selectedInsight
              ? selectedInsight.title
              : insightsT('storageAnalyticsDescription')
          }
          progressLabel={selectedInsight ? '2 / 2' : '1 / 2'}
          progressValue={selectedInsight ? 100 : 50}
        />
      ) : null}

      <div className="insights-runtime-digest">
        <span className="mono-support">
          {aiMeta ? aiMeta.label : settingsT('firstPartyRuntimeTitle')}
        </span>
        <span className="mono-support">
          {runtimeError
            ? runtimeError
            : settingsT('runtimeQueueSummary', {
                queued: runtime?.queue.queued ?? 0,
                running: runtime?.queue.running ?? 0,
                failed: runtime?.queue.failed ?? 0,
              })}
        </span>
        <Link className="btn-tiny" to="/jobs">
          {settingsT('runtimeQueueTitle')}
        </Link>
      </div>

      <div className="insights-hero-grid">
        <div className="panel insights-hero-card insights-hero-card--wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('overviewTitle')}</span>
            <span className="panel-action">
              {insightsT('snapshotLabel', {
                time:
                  formatDateTime(insights.generatedAt, language) ??
                  insights.generatedAt,
              })}
            </span>
          </div>
          <div className="panel-body insights-hero-stack">
            <div className="insights-hero-copy">
              <h2>{insightsT('overviewHeadline')}</h2>
              <p>{insightsT('overviewBody')}</p>
              <div className="insights-hero-notes">
                <p className="mono-support">
                  {activeProfileId
                    ? insightsT('scopedViewBody', {
                        profile: profileIdLabel(activeProfileId),
                      })
                    : insightsT('archiveWideBody')}
                </p>
                {aiMeta ? (
                  <p className="mono-support">{aiMeta.description}</p>
                ) : null}
              </div>
            </div>
            <div className="insights-summary">
              <div className="insight-kpi">
                <div className="kpi-label">{insightsT('window')}</div>
                <div className="kpi-value">
                  {insightsT('windowDaysCompact', {
                    days: insights.windowDays,
                  })}
                </div>
                <div className="kpi-sublabel">
                  {insightsT('generatedAt', {
                    time: formatRelativeTime(insights.generatedAt, language),
                  })}
                </div>
              </div>
              <div className="insight-kpi">
                <div className="kpi-label">{insightsT('cards')}</div>
                <div className="kpi-value">{insights.cards.length}</div>
                <div className="kpi-sublabel">
                  {insightsT('cardsDescription')}
                </div>
              </div>
              <div className="insight-kpi">
                <div className="kpi-label">{insightsT('topics')}</div>
                <div className="kpi-value">{insights.topics.length}</div>
                <div className="kpi-sublabel">
                  {insightsT('topicsDescription')}
                </div>
              </div>
              <div className="insight-kpi">
                <div className="kpi-label">{insightsT('coverage')}</div>
                <div className="kpi-value">
                  {formatInsightCoverage(
                    insights.status.contentCoverage,
                    language,
                  )}
                </div>
                <div className="kpi-sublabel">
                  {insightsT('coverageDescription')}
                </div>
              </div>
            </div>
            <div className="intelligence-actions">
              <button
                className="btn-secondary"
                type="button"
                onClick={() => void handleRefreshInsights()}
                disabled={Boolean(action)}
              >
                {insightsT('refreshInsights')}
              </button>
              <Link className="btn-secondary" to="/explorer?mode=hybrid">
                {insightsT('openExplorer')}
              </Link>
              <Link
                className="btn-secondary"
                to={assistantHref(insightsT('assistantSummaryPrompt'))}
              >
                {insightsT('askAssistant')}
              </Link>
            </div>
          </div>
        </div>

        <div className="panel insights-hero-card">
          <div className="panel-header">
            <span className="panel-title">
              {settingsT('runtimeQueueTitle')}
            </span>
            <span className="panel-action">
              {settingsT('runtimeQueueSummary', {
                queued: runtime?.queue.queued ?? 0,
                running: runtime?.queue.running ?? 0,
                failed: runtime?.queue.failed ?? 0,
              })}
            </span>
          </div>
          <div className="panel-body insights-hero-stack">
            <p className="summary-text">{insightsT('queueReviewBody')}</p>
            {runtimeLeadJob ? (
              <div className="result-row">
                <div className="result-row__header">
                  <strong>
                    {enrichmentPluginLabel(
                      runtimeLeadJob.pluginId ?? runtimeLeadJob.jobType,
                      settingsT,
                    )}
                  </strong>
                  <span className="mono-support">
                    {intelligenceRuntimeJobStateLabel(
                      runtimeLeadJob.state,
                      settingsT,
                    )}
                  </span>
                </div>
                <p>
                  {compactInsightText(
                    runtimeLeadJob.title ??
                      runtimeLeadJob.url ??
                      settingsT('runtimeNoJobs'),
                    72,
                  )}{' '}
                  ·{' '}
                  {settingsT('runtimeJobAttempt', {
                    attempt: runtimeLeadJob.attempt,
                  })}
                </p>
                {runtimeLeadJob.lastError ? (
                  <p className="mono-support">{runtimeLeadJob.lastError}</p>
                ) : null}
                {runtimeLeadJob.retryable || runtimeLeadJob.cancellable ? (
                  <div className="intelligence-actions">
                    {runtimeLeadJob.retryable ? (
                      <button
                        className="btn-secondary"
                        type="button"
                        disabled={Boolean(action)}
                        onClick={() => {
                          void handleRetryRuntimeJob(runtimeLeadJob.id)
                        }}
                      >
                        {settingsT('retryRuntimeJob')}
                      </button>
                    ) : null}
                    {runtimeLeadJob.cancellable ? (
                      <button
                        className="btn-secondary"
                        type="button"
                        disabled={Boolean(action)}
                        onClick={() => {
                          void handleCancelRuntimeJob(runtimeLeadJob.id)
                        }}
                      >
                        {settingsT('cancelRuntimeJob')}
                      </button>
                    ) : null}
                  </div>
                ) : null}
              </div>
            ) : (
              <p className="mono-support">{settingsT('runtimeNoJobs')}</p>
            )}
            <div className="intelligence-actions">
              <Link className="btn-secondary" to="/jobs">
                {settingsT('runtimeQueueTitle')}
              </Link>
            </div>
          </div>
        </div>
      </div>

      <div className="insights-grid">
        <div className="insights-section-heading panel-wide">
          <span className="panel-title">{insightsT('spotlightTitle')}</span>
          <p>{insightsT('spotlightBody')}</p>
        </div>

        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">{insightsT('onThisDay')}</span>
            <span className="panel-action">{todayKey}</span>
          </div>
          <div className="panel-body intelligence-stack">
            {onThisDay.length > 0 ? (
              onThisDay.map((item) => (
                <Link
                  key={`${item.historyId}-${item.url}`}
                  className="result-row"
                  to={evidenceHref({
                    ...item,
                    profileId: item.profileId ?? activeProfileId,
                  })}
                >
                  <div className="result-row__header">
                    <strong>{item.title ?? item.url}</strong>
                    <span className="mono-support">
                      {formatDateTime(item.visitedAt, language) ??
                        item.visitedAt}
                    </span>
                  </div>
                  <p>{item.note ?? insightsT('nothingForDayDescription')}</p>
                </Link>
              ))
            ) : (
              <EmptyState
                description={insightsT('nothingForDayDescription')}
                eyebrow={insightsT('nothingForDayEyebrow')}
                title={insightsT('nothingForDayTitle')}
              />
            )}
          </div>
        </div>

        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">{insightsT('siteAnalytics')}</span>
            <span className="panel-action">
              {insightsT('currentEvidenceSample')}
            </span>
          </div>
          <div className="panel-body">
            {siteAnalytics.length > 0 ? (
              <div className="domain-list">
                {siteAnalytics.map((item, index) => (
                  <Link
                    key={item.domain}
                    className="domain-item"
                    to={evidenceHref({
                      domain: item.domain,
                      profileId: activeProfileId,
                    })}
                  >
                    <span className="domain-rank mono dim">
                      {String(index + 1).padStart(2, '0')}
                    </span>
                    <span className="domain-name mono">{item.domain}</span>
                    <div className="domain-bar-container">
                      <div
                        className="domain-bar"
                        style={{ width: `${item.pct}%` }}
                      />
                    </div>
                    <span className="domain-count mono">{item.count}</span>
                  </Link>
                ))}
              </div>
            ) : (
              <EmptyState
                description={insightsT('noSiteAnalyticsDescription')}
                eyebrow={insightsT('noSiteAnalyticsEyebrow')}
                title={insightsT('noSiteAnalyticsTitle')}
              />
            )}
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('storageAnalytics')}</span>
            <span className="panel-action">
              {activeProfileId
                ? insightsT('archiveWideBadge')
                : insightsT('growthSignal')}
            </span>
          </div>
          <div className="panel-body intelligence-stack">
            {dashboard ? (
              <>
                <p className="summary-text">
                  {insightsT('storageAnalyticsDescription')}
                </p>
                <div className="summary-stats">
                  <div className="summary-stat">
                    <span className="dim">{insightsT('trackedStorage')}</span>
                    <span className="mono">
                      {formatBytes(storageEvidence.totalTrackedBytes)}
                    </span>
                  </div>
                  <div className="summary-stat">
                    <span className="dim">{insightsT('reclaimableSpace')}</span>
                    <span className="mono">
                      {formatBytes(storageEvidence.reclaimableBytes)}
                    </span>
                  </div>
                  <div className="summary-stat">
                    <span className="dim">{insightsT('dominantStorage')}</span>
                    <span className="mono">
                      {insightsT(
                        storageEvidence.dominantSlice.id === 'core'
                          ? 'coreStorage'
                          : storageEvidence.dominantSlice.id === 'audit'
                            ? 'auditStorage'
                            : storageEvidence.dominantSlice.id === 'exports'
                              ? 'exportStorage'
                              : 'rebuildableStorage',
                      )}
                    </span>
                  </div>
                </div>
                <div className="domain-list">
                  {storageSlices.map((slice) => (
                    <div key={slice.id} className="domain-item">
                      <span className="domain-name mono">
                        {insightsT(
                          slice.id === 'core'
                            ? 'coreStorage'
                            : slice.id === 'audit'
                              ? 'auditStorage'
                              : slice.id === 'exports'
                                ? 'exportStorage'
                                : 'rebuildableStorage',
                        )}
                      </span>
                      <div className="domain-bar-container">
                        <div
                          className="domain-bar"
                          style={{
                            width: `${Math.min(
                              100,
                              storageEvidence.totalTrackedBytes === 0
                                ? 0
                                : Math.round(
                                    (slice.bytes /
                                      storageEvidence.totalTrackedBytes) *
                                      100,
                                  ),
                            )}%`,
                          }}
                        />
                      </div>
                      <span className="domain-count mono">
                        {formatBytes(slice.bytes)}
                      </span>
                    </div>
                  ))}
                </div>
                {storageEvidence.latestRunId ? (
                  <Link
                    className="result-row"
                    to={`/audit?run=${storageEvidence.latestRunId}`}
                  >
                    <div className="result-row__header">
                      <strong>{insightsT('latestRunGrowth')}</strong>
                      <span className="mono">
                        #{storageEvidence.latestRunId}
                      </span>
                    </div>
                    <p>
                      {insightsT('latestRunGrowthBody', {
                        visits: storageEvidence.latestVisitGrowth,
                        urls: storageEvidence.latestUrlGrowth,
                        downloads: storageEvidence.latestDownloadGrowth,
                      })}
                    </p>
                  </Link>
                ) : (
                  <EmptyState
                    description={insightsT('noGrowthEvidenceDescription')}
                    eyebrow={insightsT('growthSignal')}
                    title={insightsT('noGrowthEvidenceTitle')}
                  />
                )}
              </>
            ) : (
              <EmptyState
                description={insightsT('noGrowthEvidenceDescription')}
                eyebrow={insightsT('growthSignal')}
                title={insightsT('noGrowthEvidenceTitle')}
              />
            )}
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('periodicSummary')}</span>
            <span className="panel-action">
              {insightsT('snapshotLabel', {
                time:
                  formatDateTime(insights.generatedAt, language) ??
                  insights.generatedAt,
              })}
            </span>
          </div>
          <div className="panel-body intelligence-stack">
            {insights.templateSummaries.length > 0 ? (
              <div className="intelligence-result-list">
                {insights.templateSummaries.map((summary) => (
                  <div key={summary.summaryId} className="result-row">
                    <div className="result-row__header">
                      <strong>{compactInsightText(summary.title, 88)}</strong>
                      <span className="mono-support">
                        {Math.round(summary.confidence * 100)}%
                      </span>
                    </div>
                    <p>{summary.body}</p>
                    <div className="intelligence-actions">
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() =>
                          void handleExplain({
                            id: summary.summaryId,
                            kind: 'template-summary',
                            title: summary.title,
                            profileId: summary.profileId,
                            windowDays: insights.windowDays,
                          })
                        }
                        disabled={Boolean(action)}
                      >
                        {insightsT('explain')}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              periodicSummary.map((paragraph) => (
                <p key={paragraph} className="summary-text">
                  {paragraph}
                </p>
              ))
            )}
            {insights.notes.length > 0 && (
              <div className="intelligence-note-list">
                {insights.notes.map((note) => (
                  <p key={note} className="mono-support">
                    {note}
                  </p>
                ))}
              </div>
            )}
            <div className="summary-stats">
              <div className="summary-stat">
                <span className="dim">{insightsT('threads')}</span>
                <span className="mono">{insights.threads.length}</span>
              </div>
              <div className="summary-stat">
                <span className="dim">{insightsT('cardsStat')}</span>
                <span className="mono">{insights.cards.length}</span>
              </div>
              <div className="summary-stat">
                <span className="dim">{insightsT('topicsStat')}</span>
                <span className="mono">{insights.topics.length}</span>
              </div>
            </div>
          </div>
        </div>

        <div className="insights-section-heading panel-wide">
          <span className="panel-title">
            {insightsT('researchSignalsTitle')}
          </span>
          <p>{insightsT('researchSignalsBody')}</p>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('queryGroups')}</span>
            <span className="panel-action">{insights.queryGroups.length}</span>
          </div>
          <div className="panel-body intelligence-stack">
            {insights.queryGroups.length > 0 ? (
              <div className="intelligence-result-list">
                {insights.queryGroups.map((group) => {
                  const params = new URLSearchParams()
                  params.set('q', group.latestQuery)
                  params.set('mode', 'keyword')
                  params.set('profileId', group.profileId)
                  const displayTitle = compactInsightText(group.title, 88)
                  const displaySteps = group.steps
                    .map((step) => compactInsightText(step, 56))
                    .join(' -> ')

                  return (
                    <div key={group.queryGroupId} className="result-row">
                      <div className="result-row__header">
                        <strong>{displayTitle}</strong>
                        <span className="mono-support">
                          {Math.round(group.confidence * 100)}%
                        </span>
                      </div>
                      <p>{displaySteps}</p>
                      <div className="result-row__meta">
                        <span className="mono-support">
                          {insightsT('queryEvolutionSteps', {
                            count: group.stepCount,
                          })}
                        </span>
                        <span className="mono-support">
                          {group.evidenceTier}
                        </span>
                      </div>
                      <div className="intelligence-actions">
                        <button
                          className="btn-tiny"
                          type="button"
                          onClick={() =>
                            void handleExplain({
                              id: group.queryGroupId,
                              kind: 'query-group',
                              title: displayTitle,
                              profileId: group.profileId,
                              windowDays: insights.windowDays,
                            })
                          }
                          disabled={Boolean(action)}
                        >
                          {insightsT('explain')}
                        </button>
                        <Link
                          className="btn-tiny"
                          to={`/explorer?${params.toString()}`}
                        >
                          {insightsT('openExplorer')}
                        </Link>
                      </div>
                    </div>
                  )
                })}
              </div>
            ) : (
              <EmptyState
                description={insightsT('queryGroupsEmptyDescription')}
                eyebrow={insightsT('queryGroups')}
                title={insightsT('queryGroupsEmptyTitle')}
              />
            )}
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('topicTimeline')}</span>
            <span className="panel-action">
              {insightsT('lastDays', { days: insights.windowDays })}
            </span>
          </div>
          <div className="panel-body">
            <div className="topic-timeline">
              {insights.topics.map((topic, index) => (
                <div key={topic.topicId} className="topic-row">
                  <div className="topic-name">
                    <div
                      className="topic-dot"
                      style={{
                        background: topicColors[index % topicColors.length],
                      }}
                    />
                    <span>{topic.label}</span>
                  </div>
                  <div className="topic-bars">
                    <div className="topic-bar-track">
                      <div
                        className="topic-bar"
                        style={{
                          width: `${Math.min(100, Math.max(12, topic.visitCount * 6))}%`,
                          background: topicColors[index % topicColors.length],
                          opacity: 0.8,
                        }}
                      />
                    </div>
                  </div>
                  <Link
                    className="topic-count mono"
                    to={evidenceHref({
                      title: topic.label,
                      profileId: activeProfileId,
                    })}
                  >
                    {topic.visitCount}
                  </Link>
                </div>
              ))}
            </div>
            <div className="topic-axis">
              <span>
                {insightsT('windowAxis', { days: insights.windowDays })}
              </span>
              <span>{insights.generatedAt.slice(0, 10)}</span>
            </div>
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('queryEvolution')}</span>
            <span className="panel-action">
              {insightsT('chromiumEnhanced')}
            </span>
          </div>
          <div className="panel-body intelligence-stack">
            <p className="summary-text">
              {insightsT('queryEvolutionDescription')}
            </p>
            {insights.queryLadders.length > 0 ? (
              <div className="intelligence-result-list">
                {insights.queryLadders.map((ladder, index) => {
                  const params = new URLSearchParams()
                  params.set(
                    'q',
                    ladder.steps[ladder.steps.length - 1] ?? ladder.rootTerm,
                  )
                  params.set('mode', 'keyword')
                  if (ladder.profileId) {
                    params.set('profileId', ladder.profileId)
                  }
                  const ladderKey = [
                    ladder.profileId ?? 'all-profiles',
                    ladder.rootTerm,
                    ladder.steps.join('->'),
                    ladder.stages.join('->'),
                    index,
                  ].join('::')

                  return (
                    <Link
                      key={ladderKey}
                      className="result-row"
                      to={`/explorer?${params.toString()}`}
                    >
                      <div className="result-row__header">
                        <strong>{ladder.rootTerm}</strong>
                        <span className="mono-support">
                          {insightsT('queryEvolutionSteps', {
                            count: ladder.steps.length,
                          })}
                        </span>
                      </div>
                      <p>{ladder.steps.join(' -> ')}</p>
                      <div className="result-row__meta">
                        <span className="mono-support">
                          {ladder.stages
                            .map((stage) => queryStageLabel(stage, insightsT))
                            .join(' · ')}
                        </span>
                        <span className="mono-support">
                          {profileIdLabel(ladder.profileId)}
                        </span>
                      </div>
                    </Link>
                  )
                })}
              </div>
            ) : (
              <EmptyState
                description={insightsT('queryEvolutionEmptyDescription')}
                eyebrow={insightsT('queryEvolution')}
                title={insightsT('queryEvolutionEmptyTitle')}
              />
            )}
          </div>
        </div>

        <div className="insights-section-heading panel-wide">
          <span className="panel-title">
            {insightsT('evidenceLibraryTitle')}
          </span>
          <p>{insightsT('evidenceLibraryBody')}</p>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('referencePages')}</span>
            <span className="panel-action">
              {insights.referencePages.length}
            </span>
          </div>
          <div className="panel-body intelligence-stack">
            {insights.referencePages.length > 0 ? (
              <div className="intelligence-result-list">
                {insights.referencePages.map((page) => (
                  <div key={page.referencePageId} className="result-row">
                    <div className="result-row__header">
                      <strong>
                        {compactInsightText(page.title ?? page.url, 88)}
                      </strong>
                      <span className="mono-support">{page.domain}</span>
                    </div>
                    <p>
                      {insightsT('referencePagesBody', {
                        groups: page.queryGroupCount,
                        threads: page.threadCount,
                        revisits: page.revisitCount,
                      })}
                    </p>
                    <div className="intelligence-actions">
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() =>
                          void handleExplain({
                            id: page.referencePageId,
                            kind: 'reference-page',
                            title: compactInsightText(
                              page.title ?? page.url,
                              88,
                            ),
                            profileId: page.profileId,
                            windowDays: insights.windowDays,
                          })
                        }
                        disabled={Boolean(action)}
                      >
                        {insightsT('explain')}
                      </button>
                      <Link
                        className="btn-tiny"
                        to={evidenceHref({
                          url: page.url,
                          profileId: page.profileId ?? activeProfileId,
                        })}
                      >
                        {insightsT('openExplorer')}
                      </Link>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyState
                description={insightsT('referencePagesEmptyDescription')}
                eyebrow={insightsT('referencePages')}
                title={insightsT('referencePagesEmptyTitle')}
              />
            )}
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">
              {insightsT('sourceEffectiveness')}
            </span>
            <span className="panel-action">
              {insights.sourceEffectiveness.length}
            </span>
          </div>
          <div className="panel-body intelligence-stack">
            {insights.sourceEffectiveness.length > 0 ? (
              <div className="intelligence-result-list">
                {insights.sourceEffectiveness.map((source) => (
                  <div key={source.sourceId} className="result-row">
                    <div className="result-row__header">
                      <strong>{source.domain}</strong>
                      <span className="mono-support">{source.sourceRole}</span>
                    </div>
                    <p>
                      {insightsT('sourceEffectivenessBody', {
                        groups: source.queryGroupCount,
                        references: source.referencePageCount,
                        landings: source.stableLandingCount,
                      })}
                    </p>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyState
                description={insightsT('sourceEffectivenessEmptyDescription')}
                eyebrow={insightsT('sourceEffectiveness')}
                title={insightsT('sourceEffectivenessEmptyTitle')}
              />
            )}
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">
              {insightsT('deterministicModules')}
            </span>
            <span className="panel-action">{runtime?.modules.length ?? 0}</span>
          </div>
          <div className="panel-body intelligence-stack">
            {runtime?.modules.length ? (
              <div className="intelligence-result-list">
                {runtime.modules.map((module) => (
                  <div key={module.moduleId} className="result-row">
                    <div className="result-row__header">
                      <strong>
                        {deterministicModuleLabel(module.moduleId, settingsT)}
                      </strong>
                      <span className="mono-support">
                        {deterministicModuleStatusLabel(
                          module.status,
                          settingsT,
                        )}
                      </span>
                    </div>
                    <p>
                      {module.notes[0] ??
                        insightsT('deterministicModulesDescription')}
                    </p>
                    <div className="result-row__meta">
                      <span className="mono-support">
                        {module.derivedTables.join(', ')}
                      </span>
                      {module.lastBuiltAt ? (
                        <span className="mono-support">
                          {formatDateTime(module.lastBuiltAt, language) ??
                            module.lastBuiltAt}
                        </span>
                      ) : null}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyState
                description={insightsT('deterministicModulesEmptyDescription')}
                eyebrow={insightsT('deterministicModules')}
                title={insightsT('deterministicModulesEmptyTitle')}
              />
            )}
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('insightCards')}</span>
            <span className="panel-action">{insightsT('explainable')}</span>
          </div>
          <div className="panel-body intelligence-result-list">
            {insights.cards.map((card) => (
              <div key={card.cardId} className="result-row">
                <div className="result-row__header">
                  <strong>{compactInsightText(card.title, 88)}</strong>
                  <span className="mono-support">
                    {card.kind === 'open-loop'
                      ? insightsT('openLoopSignal')
                      : card.kind === 'revisit'
                        ? insightsT('revisitSignal')
                        : card.kind === 'focus-balance'
                          ? insightsT('focusBalanceSignal')
                          : insightsT('genericCard')}
                  </span>
                </div>
                <p>{card.summary}</p>
                <div className="result-row__meta">
                  <span className="mono-support">
                    {insightsT('evidenceItems', {
                      count: card.evidence.length,
                      days: card.windowDays,
                    })}
                  </span>
                  <span className="mono-support">
                    {card.chromiumEnhanced
                      ? insightsT('chromiumEnhanced')
                      : insightsT('crossBrowserSafe')}
                  </span>
                </div>
                <div className="intelligence-actions">
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() =>
                      void handleExplain({
                        id: card.cardId,
                        kind: card.kind,
                        title: compactInsightText(card.title, 88),
                        profileId: card.profileId,
                        windowDays: card.windowDays,
                      })
                    }
                    disabled={Boolean(action)}
                  >
                    {insightsT('explain')}
                  </button>
                  <Link
                    className="btn-tiny"
                    to={assistantHref(
                      insightsT('explainCardPrompt', { title: card.title }),
                    )}
                  >
                    {insightsT('askAssistant')}
                  </Link>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {selectedInsight && explanation ? (
        <div className="panel intelligence-panel">
          <div className="panel-header">
            <span className="panel-title">{insightsT('explainability')}</span>
            <span className="panel-action">
              {compactInsightText(selectedInsight.title, 72)}
            </span>
          </div>
          <div className="panel-body intelligence-stack">
            <p className="summary-text">{explanation.explanation}</p>
            {explanation.notes.length > 0 && (
              <div className="intelligence-note-list">
                {explanation.notes.map((note) => (
                  <p key={note} className="mono-support">
                    {note}
                  </p>
                ))}
              </div>
            )}
            <div className="intelligence-result-list">
              {explanation.citations.map((item: InsightEvidenceItem) => (
                <Link
                  key={`${item.historyId}-${item.url}`}
                  className="result-row"
                  to={evidenceHref({
                    ...item,
                    profileId: item.profileId ?? activeProfileId,
                  })}
                >
                  <div className="result-row__header">
                    <strong>
                      {compactInsightText(item.title ?? item.url, 88)}
                    </strong>
                    <span className="mono-support">
                      {formatDateTime(item.visitedAt, language) ??
                        item.visitedAt}
                    </span>
                  </div>
                  <p>{item.note ?? insightsT('usedToExplain')}</p>
                </Link>
              ))}
            </div>
          </div>
        </div>
      ) : null}

      {action ? <p className="mono-support">{action}…</p> : null}
    </section>
  )
}
