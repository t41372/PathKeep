/**
 * This module renders the Jobs route, the always-on review surface for background work and recovery.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - Heavy queue-backed work should stay visible and interruptible instead of hiding behind Settings-only diagnostics.
 *
 * Main declarations:
 * - `JobsPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/features/intelligence.md` for queue persistence, pause/resume, and recoverability semantics.
 */

import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { ReviewSection } from '../../components/review'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { formatDateTime, formatRelativeTime } from '../../lib/format'
import { type ResolvedLanguage, useI18n } from '../../lib/i18n'
import {
  runtimeJobMutationNeedsRefresh,
  summarizeRuntimeJob,
  summarizeRuntimeJobError,
} from '../../lib/intelligence-presentation'
import {
  enrichmentPluginLabel,
  intelligenceRuntimeJobStateLabel,
} from '../../lib/intelligence-runtime'
import type {
  AiQueueJob,
  AppConfig,
  IntelligenceJobOverview,
} from '../../lib/types'
import { JobsRuntimeHealthSection } from './runtime-health-section'

type JobsTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

function aiJobStateLabel(state: string, jobsT: JobsTranslator) {
  switch (state) {
    case 'queued':
      return jobsT('jobStateQueued')
    case 'running':
      return jobsT('jobStateRunning')
    case 'succeeded':
      return jobsT('jobStateSucceeded')
    case 'failed':
      return jobsT('jobStateFailed')
    case 'cancelled':
      return jobsT('jobStateCancelled')
    case 'paused':
      return jobsT('jobStatePaused')
    case 'stale':
      return jobsT('jobStateStale')
    default:
      return state
  }
}

function nextPausedConfig(config: AppConfig, paused: boolean): AppConfig {
  return {
    ...config,
    ai: {
      ...config.ai,
      jobQueuePaused: paused,
    },
  }
}

export function JobsPage() {
  const {
    loading,
    refreshAppData,
    refreshRuntimeStatus,
    saveConfig,
    snapshot,
    runtimeStatus = {
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    },
  } = useShellData()
  const { language, ns } = useI18n()
  const jobsT = ns('jobs')
  const settingsT = ns('settings')
  const commonT = ns('common')
  const [pageError, setPageError] = useState<string | null>(null)
  const [action, setAction] = useState<string | null>(null)
  const aiQueue = runtimeStatus.aiQueue
  const runtime = runtimeStatus.intelligence
  const runtimeLoading =
    runtimeStatus.loading ||
    (!runtimeStatus.error && aiQueue === null && runtime === null)

  const queueCounts = useMemo(() => {
    const queued = (aiQueue?.queued ?? 0) + (runtime?.queue.queued ?? 0)
    const running = (aiQueue?.running ?? 0) + (runtime?.queue.running ?? 0)
    const failed = (aiQueue?.failed ?? 0) + (runtime?.queue.failed ?? 0)
    return { failed, queued, running }
  }, [aiQueue, runtime])

  const statusCallout = useMemo(() => {
    if (snapshot?.config.ai.jobQueuePaused && queueCounts.queued > 0) {
      return {
        body: jobsT('pausedBody'),
        title: jobsT('pausedTitle'),
        tone: 'warning' as const,
      }
    }
    if (queueCounts.failed > 0) {
      return {
        body: jobsT('failedBody'),
        title: jobsT('failedTitle'),
        tone: 'danger' as const,
      }
    }
    if (queueCounts.running > 0) {
      return {
        body: jobsT('runningBody'),
        title: jobsT('runningTitle'),
        tone: 'info' as const,
      }
    }
    if (queueCounts.queued > 0) {
      return {
        body: jobsT('queuedBody'),
        title: jobsT('queuedTitle'),
        tone: 'info' as const,
      }
    }
    return {
      body: jobsT('readyBody'),
      title: jobsT('readyTitle'),
      tone: 'success' as const,
    }
  }, [jobsT, queueCounts, snapshot?.config.ai.jobQueuePaused])

  async function handleRefresh() {
    setAction(jobsT('refresh'))
    try {
      setPageError(null)
      await Promise.all([refreshAppData(), refreshRuntimeStatus()])
    } finally {
      setAction(null)
    }
  }

  async function handlePauseChange(paused: boolean) {
    if (!snapshot) return
    setAction(paused ? jobsT('pauseQueue') : jobsT('resumeQueue'))
    try {
      await saveConfig(nextPausedConfig(snapshot.config, paused))
      setPageError(null)
      await Promise.all([refreshAppData(), refreshRuntimeStatus()])
    } finally {
      setAction(null)
    }
  }

  async function handleReplayAiJob(jobId: number) {
    setAction(jobsT('retryJob'))
    try {
      await backend.replayAiJob(jobId)
      setPageError(null)
      await Promise.all([refreshAppData(), refreshRuntimeStatus()])
    } finally {
      setAction(null)
    }
  }

  async function handleCancelAiJob(jobId: number) {
    setAction(jobsT('cancelJob'))
    try {
      await backend.cancelAiJob(jobId)
      setPageError(null)
      await Promise.all([refreshAppData(), refreshRuntimeStatus()])
    } finally {
      setAction(null)
    }
  }

  async function handleRetryRuntimeJob(jobId: number) {
    setAction(jobsT('retryJob'))
    try {
      await backend.retryIntelligenceJob(jobId)
      await Promise.all([refreshAppData(), refreshRuntimeStatus()])
      setPageError(null)
    } catch (error) {
      const message =
        error instanceof Error ? error.message : commonT('notAvailable')
      if (runtimeJobMutationNeedsRefresh(message)) {
        await Promise.all([refreshAppData(), refreshRuntimeStatus()])
        return
      }
      setPageError(message)
    } finally {
      setAction(null)
    }
  }

  async function handleCancelRuntimeJob(jobId: number) {
    setAction(jobsT('cancelJob'))
    try {
      await backend.cancelIntelligenceJob(jobId)
      await Promise.all([refreshAppData(), refreshRuntimeStatus()])
      setPageError(null)
    } catch (error) {
      const message =
        error instanceof Error ? error.message : commonT('notAvailable')
      if (runtimeJobMutationNeedsRefresh(message)) {
        await Promise.all([refreshAppData(), refreshRuntimeStatus()])
        return
      }
      setPageError(message)
    } finally {
      setAction(null)
    }
  }

  if (loading && !snapshot) {
    return (
      <section className="page-shell" data-testid="jobs-page">
        <LoadingState label={jobsT('loadingPage')} />
      </section>
    )
  }

  if (!snapshot?.config.initialized) {
    return (
      <section className="page-shell" data-testid="jobs-page">
        <EmptyState
          description={jobsT('setupDescription')}
          eyebrow={jobsT('statusEyebrow')}
          title={jobsT('setupTitle')}
          action={
            <Link className="btn-primary" to="/onboarding">
              {commonT('initializeFirst')}
            </Link>
          }
        />
      </section>
    )
  }

  if (!snapshot.archiveStatus.unlocked) {
    return (
      <section className="page-shell" data-testid="jobs-page">
        <PermissionGate
          detail={jobsT('lockedDetail')}
          eyebrow={jobsT('lockedEyebrow')}
          title={jobsT('lockedTitle')}
        >
          <Link className="btn-primary" to="/security">
            {commonT('reviewSecurity')}
          </Link>
        </PermissionGate>
      </section>
    )
  }

  if (runtimeLoading) {
    return (
      <section className="page-shell" data-testid="jobs-page">
        <LoadingState label={jobsT('loadingPage')} />
      </section>
    )
  }

  if (runtimeStatus.error && !aiQueue && !runtime) {
    return (
      <section className="page-shell" data-testid="jobs-page">
        <ErrorState
          title={jobsT('pageUnavailableTitle')}
          description={runtimeStatus.error}
          action={
            <button
              className="btn-secondary"
              type="button"
              onClick={() => void handleRefresh()}
            >
              {jobsT('refresh')}
            </button>
          }
        />
      </section>
    )
  }

  const lastActivityAt =
    runtime?.queue.lastActivityAt ??
    aiQueue?.recentJobs.find((job) => job.finishedAt || job.startedAt)
      ?.finishedAt ??
    aiQueue?.recentJobs[0]?.queuedAt ??
    null
  const contentPlugin =
    runtime?.plugins.find(
      (plugin) => plugin.pluginId === 'readable-content-refetch',
    ) ?? null
  const activeRuntimeJob =
    runtime?.recentJobs.find((job) => job.state === 'running') ??
    runtime?.recentJobs.find((job) => job.state === 'queued') ??
    null
  const reviewRuntimeJob =
    runtime?.recentJobs.find((job) => job.state === 'failed') ?? null
  const contentQueueMessage = contentPlugin
    ? contentPlugin.queuedJobs > 0
      ? jobsT('contentFetchBacklogBody', {
          queued: contentPlugin.queuedJobs,
          stored: contentPlugin.storedRecords,
        })
      : contentPlugin.runningJobs > 0
        ? jobsT('contentFetchRunningBody', {
            stored: contentPlugin.storedRecords,
          })
        : jobsT('contentFetchReadyBody', {
            stored: contentPlugin.storedRecords,
          })
    : jobsT('contentFetchFallbackBody')
  const focusNowMessage = activeRuntimeJob
    ? summarizeRuntimeJob(activeRuntimeJob, jobsT)
    : queueCounts.running > 0
      ? jobsT('focusNowBacklog')
      : jobsT('focusNowIdle')
  const needsReviewMessage = reviewRuntimeJob
    ? summarizeRuntimeJobError(reviewRuntimeJob, jobsT)
    : queueCounts.failed > 0
      ? jobsT('needsReviewBacklog', { count: queueCounts.failed })
      : jobsT('needsReviewIdle')

  return (
    <section className="page-shell jobs-page" data-testid="jobs-page">
      <div className="jobs-grid">
        <StatusCallout
          tone={statusCallout.tone}
          eyebrow={jobsT('statusEyebrow')}
          title={statusCallout.title}
          body={statusCallout.body}
          actions={
            <div className="intelligence-actions">
              <button
                className="btn-secondary"
                type="button"
                onClick={() => void handleRefresh()}
                disabled={Boolean(action)}
              >
                {jobsT('refresh')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={() =>
                  void handlePauseChange(!snapshot.config.ai.jobQueuePaused)
                }
                disabled={Boolean(action)}
              >
                {snapshot.config.ai.jobQueuePaused
                  ? jobsT('resumeQueue')
                  : jobsT('pauseQueue')}
              </button>
              <Link className="btn-secondary" to="/settings">
                {jobsT('openSettings')}
              </Link>
            </div>
          }
        />

        {runtimeStatus.error ? (
          <StatusCallout
            tone="warning"
            title={jobsT('pageUnavailableTitle')}
            body={runtimeStatus.error}
          />
        ) : null}

        {pageError ? (
          <StatusCallout
            tone="warning"
            title={jobsT('pageUnavailableTitle')}
            body={pageError}
          />
        ) : null}

        <div className="jobs-overview-grid">
          <div className="panel jobs-hero-card jobs-hero-card--wide">
            <div className="panel-header">
              <span className="panel-title">{jobsT('overviewTitle')}</span>
              <span className="panel-action">
                {lastActivityAt
                  ? formatRelativeTime(lastActivityAt, language)
                  : jobsT('sidebarIdleDetail')}
              </span>
            </div>
            <div className="panel-body jobs-panel-stack">
              <div className="jobs-hero-copy">
                <h2>{jobsT('overviewHeadline')}</h2>
                <p>{jobsT('overviewBody')}</p>
                <p className="mono-support">{contentQueueMessage}</p>
              </div>
              <div className="jobs-hero-stats">
                <div className="jobs-hero-stat">
                  <span className="dim">{jobsT('runningCount')}</span>
                  <strong className="mono">
                    {queueCounts.running.toLocaleString(language)}
                  </strong>
                </div>
                <div className="jobs-hero-stat">
                  <span className="dim">{jobsT('queuedCount')}</span>
                  <strong className="mono">
                    {queueCounts.queued.toLocaleString(language)}
                  </strong>
                </div>
                <div className="jobs-hero-stat">
                  <span className="dim">{jobsT('failedCount')}</span>
                  <strong className="mono">
                    {queueCounts.failed.toLocaleString(language)}
                  </strong>
                </div>
                <div className="jobs-hero-stat">
                  <span className="dim">{jobsT('savedReadableContent')}</span>
                  <strong className="mono">
                    {(contentPlugin?.storedRecords ?? 0).toLocaleString(
                      language,
                    )}
                  </strong>
                </div>
              </div>
              <div className="jobs-callout-strip">
                <div className="jobs-mini-callout">
                  <span className="dim">{jobsT('focusNow')}</span>
                  <p>{focusNowMessage}</p>
                </div>
                <div className="jobs-mini-callout">
                  <span className="dim">{jobsT('needsReviewNow')}</span>
                  <p>{needsReviewMessage}</p>
                </div>
              </div>
            </div>
          </div>

          <div className="panel jobs-overview-card">
            <div className="panel-header">
              <span className="panel-title">{jobsT('queueSummaryTitle')}</span>
              <span className="panel-action">
                {snapshot.config.ai.jobQueuePaused
                  ? jobsT('queueStatePaused')
                  : jobsT('queueStateLive')}
              </span>
            </div>
            <div className="panel-body jobs-panel-stack">
              <p>{jobsT('queueSummaryBody')}</p>
              <div className="intelligence-stat-row">
                <div className="summary-stat">
                  <span className="dim">{jobsT('queuedCount')}</span>
                  <span className="mono">
                    {(aiQueue?.queued ?? 0).toLocaleString(language)}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{jobsT('runningCount')}</span>
                  <span className="mono">
                    {(aiQueue?.running ?? 0).toLocaleString(language)}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{jobsT('failedCount')}</span>
                  <span className="mono">
                    {(aiQueue?.failed ?? 0).toLocaleString(language)}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{jobsT('concurrency')}</span>
                  <span className="mono">
                    {(
                      aiQueue?.concurrency ??
                      snapshot.config.ai.jobQueueConcurrency
                    ).toLocaleString(language)}
                  </span>
                </div>
              </div>
              <p className="mono-support">
                {jobsT('lastActivity')}:{' '}
                {lastActivityAt
                  ? formatRelativeTime(lastActivityAt, language)
                  : jobsT('sidebarIdleDetail')}
              </p>
            </div>
          </div>

          <div className="panel jobs-overview-card">
            <div className="panel-header">
              <span className="panel-title">
                {jobsT('runtimeSummaryTitle')}
              </span>
              <span className="panel-action">
                {(runtime?.queue.queued ?? 0).toLocaleString(language)}{' '}
                {jobsT('queuedCount').toLowerCase()}
              </span>
            </div>
            <div className="panel-body jobs-panel-stack">
              <p>{jobsT('runtimeSummaryBody')}</p>
              <div className="intelligence-stat-row">
                <div className="summary-stat">
                  <span className="dim">{jobsT('queuedCount')}</span>
                  <span className="mono">
                    {(runtime?.queue.queued ?? 0).toLocaleString(language)}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{jobsT('runningCount')}</span>
                  <span className="mono">
                    {(runtime?.queue.running ?? 0).toLocaleString(language)}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{jobsT('failedCount')}</span>
                  <span className="mono">
                    {(runtime?.queue.failed ?? 0).toLocaleString(language)}
                  </span>
                </div>
                <div className="summary-stat">
                  <span className="dim">{jobsT('lastActivity')}</span>
                  <span className="mono">
                    {runtime?.queue.lastActivityAt
                      ? formatRelativeTime(
                          runtime.queue.lastActivityAt,
                          language,
                        )
                      : jobsT('sidebarIdleDetail')}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
        <JobsRuntimeHealthSection
          commonT={commonT}
          jobsT={jobsT}
          language={language}
          runtime={runtime}
          settingsT={settingsT}
        />

        <div className="jobs-section-heading">
          <span className="panel-title">{jobsT('recentActivityTitle')}</span>
          <p>{jobsT('recentActivityBody')}</p>
        </div>

        <div className="jobs-summary-grid">
          <JobPanel
            action={action}
            emptyLabel={jobsT('recentJobsEmpty')}
            jobs={aiQueue?.recentJobs ?? []}
            language={language}
            noDetailsLabel={jobsT('noErrorDetails')}
            onCancel={handleCancelAiJob}
            onRetry={handleReplayAiJob}
            stateLabel={(state) => aiJobStateLabel(state, jobsT)}
            title={jobsT('recentAiJobs')}
            jobsT={jobsT}
          />
          <RuntimeJobPanel
            action={action}
            emptyLabel={jobsT('recentJobsEmpty')}
            jobs={runtime?.recentJobs ?? []}
            language={language}
            onCancel={handleCancelRuntimeJob}
            onRetry={handleRetryRuntimeJob}
            settingsT={settingsT}
            title={jobsT('recentRuntimeJobs')}
            jobsT={jobsT}
          />
        </div>
      </div>
    </section>
  )
}

function JobPanel({
  action,
  emptyLabel,
  jobs,
  language,
  noDetailsLabel,
  onCancel,
  onRetry,
  stateLabel,
  title,
  jobsT,
}: {
  action: string | null
  emptyLabel: string
  jobs: AiQueueJob[]
  language: ResolvedLanguage
  noDetailsLabel: string
  onCancel: (jobId: number) => Promise<void>
  onRetry: (jobId: number) => Promise<void>
  stateLabel: (state: string) => string
  title: string
  jobsT: JobsTranslator
}) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{title}</span>
      </div>
      <div className="panel-body intelligence-job-list">
        {jobs.length === 0 ? (
          <p className="mono-support">{emptyLabel}</p>
        ) : (
          jobs.map((job) => (
            <ReviewSection
              key={job.id}
              headerMeta={
                <span className="mono-support">{stateLabel(job.state)}</span>
              }
              title={
                <>
                  {job.jobType} · #{job.id}
                </>
              }
            >
              <p>{job.summary ?? job.errorMessage ?? noDetailsLabel}</p>
              <div className="jobs-meta-grid mono-support">
                <span>
                  {jobsT('createdAt')}:{' '}
                  {formatDateTime(job.queuedAt, language) ?? job.queuedAt}
                </span>
                <span>
                  {jobsT('startedAt')}:{' '}
                  {job.startedAt
                    ? formatDateTime(job.startedAt, language)
                    : '—'}
                </span>
                <span>
                  {jobsT('finishedAt')}:{' '}
                  {job.finishedAt
                    ? formatDateTime(job.finishedAt, language)
                    : '—'}
                </span>
              </div>
              <div className="intelligence-actions">
                {['failed', 'cancelled', 'paused', 'stale'].includes(
                  job.state,
                ) ? (
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => void onRetry(job.id)}
                    disabled={Boolean(action)}
                  >
                    {jobsT('retryJob')}
                  </button>
                ) : null}
                {['queued', 'paused', 'stale'].includes(job.state) ? (
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => void onCancel(job.id)}
                    disabled={Boolean(action)}
                  >
                    {jobsT('cancelJob')}
                  </button>
                ) : null}
              </div>
            </ReviewSection>
          ))
        )}
      </div>
    </div>
  )
}

function RuntimeJobPanel({
  action,
  emptyLabel,
  jobs,
  language,
  onCancel,
  onRetry,
  settingsT,
  title,
  jobsT,
}: {
  action: string | null
  emptyLabel: string
  jobs: IntelligenceJobOverview[]
  language: ResolvedLanguage
  onCancel: (jobId: number) => Promise<void>
  onRetry: (jobId: number) => Promise<void>
  settingsT: ReturnType<ReturnType<typeof useI18n>['ns']>
  title: string
  jobsT: JobsTranslator
}) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">{title}</span>
      </div>
      <div className="panel-body intelligence-job-list">
        {jobs.length === 0 ? (
          <p className="mono-support">{emptyLabel}</p>
        ) : (
          jobs.map((job) => (
            <ReviewSection
              key={job.id}
              headerMeta={
                <span className="mono-support">
                  {intelligenceRuntimeJobStateLabel(job.state, settingsT)}
                </span>
              }
              title={
                <>
                  {(job.pluginId
                    ? enrichmentPluginLabel(job.pluginId, settingsT)
                    : job.jobType) || job.jobType}{' '}
                  · #{job.id}
                </>
              }
            >
              <p>{summarizeRuntimeJob(job, jobsT)}</p>
              {job.lastError &&
              summarizeRuntimeJob(job, jobsT) !== job.lastError ? (
                <p className="mono-support">{job.lastError}</p>
              ) : null}
              {typeof job.progressPercent === 'number' ? (
                <div className="jobs-progress">
                  <div aria-hidden="true" className="jobs-progress__track">
                    <span
                      className="jobs-progress__fill"
                      style={{
                        width: `${Math.max(
                          4,
                          Math.min(100, job.progressPercent),
                        )}%`,
                      }}
                    />
                  </div>
                  <div className="jobs-progress__meta mono-support">
                    <span>{Math.round(job.progressPercent)}%</span>
                    <span>{job.progressLabel ?? jobsT('runningCount')}</span>
                  </div>
                </div>
              ) : null}
              {job.progressDetail ? (
                <p className="mono-support">{job.progressDetail}</p>
              ) : null}
              <div className="jobs-meta-grid mono-support">
                <span>
                  {jobsT('createdAt')}:{' '}
                  {formatDateTime(job.createdAt, language) ?? job.createdAt}
                </span>
                <span>
                  {jobsT('startedAt')}:{' '}
                  {job.startedAt
                    ? formatDateTime(job.startedAt, language)
                    : '—'}
                </span>
                <span>
                  {jobsT('finishedAt')}:{' '}
                  {job.finishedAt
                    ? formatDateTime(job.finishedAt, language)
                    : '—'}
                </span>
                <span>
                  {jobsT('lastActivity')}:{' '}
                  {job.heartbeatAt
                    ? formatRelativeTime(job.heartbeatAt, language)
                    : formatRelativeTime(job.updatedAt, language)}
                </span>
              </div>
              <div className="intelligence-actions">
                {job.retryable ? (
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => void onRetry(job.id)}
                    disabled={Boolean(action)}
                  >
                    {jobsT('retryJob')}
                  </button>
                ) : null}
                {job.cancellable ? (
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => void onCancel(job.id)}
                    disabled={Boolean(action)}
                  >
                    {jobsT('cancelJob')}
                  </button>
                ) : null}
              </div>
            </ReviewSection>
          ))
        )}
      </div>
    </div>
  )
}
