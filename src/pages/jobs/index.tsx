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
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { TaskProgressCard } from '../../components/progress'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  runtimeJobMutationNeedsRefresh,
  summarizeRuntimeJob,
  summarizeRuntimeJobError,
} from '../../lib/intelligence-presentation'
import { readableContentFetchAvailable } from '../../lib/release-capabilities'
import type { AppConfig, BackupRunOverview } from '../../lib/types'
import type { ShellTask } from '../../app/shell-tasks'
import { JobPanel, RuntimeJobPanel } from './job-panels'
import { JobsRuntimeHealthSection } from './runtime-health-section'

function nextPausedConfig(config: AppConfig, paused: boolean): AppConfig {
  return {
    ...config,
    ai: {
      ...config.ai,
      jobQueuePaused: paused,
    },
  }
}

function isStaleArchiveRun(run: BackupRunOverview) {
  const kind = run.runType ?? ''
  return (kind === 'backup' || kind === 'import') && run.status === 'running'
}

function staleArchiveRunTask(
  run: BackupRunOverview,
  jobsT: (key: string, vars?: Record<string, string | number>) => string,
): ShellTask {
  const kind = run.runType === 'import' ? 'import' : 'backup'
  return {
    id: `stale-run-${run.id}`,
    kind,
    state: 'stale',
    title: jobsT('archiveTaskStaleTitle'),
    detail: jobsT('archiveTaskStaleBody'),
    startedAt: run.startedAt,
    updatedAt: run.finishedAt ?? run.startedAt,
    finishedAt: run.finishedAt ?? null,
    sourceLabel: run.profileScope?.join(', ') ?? null,
    profileLabel: null,
    progressLabel: null,
    progressValue: null,
    current: run.profilesProcessed,
    total: null,
    processedRecords: run.newVisits,
    totalRecords: null,
    importedRecords: run.newVisits,
    duplicateRecords: null,
    skippedRecords: null,
    logEntries: [
      {
        id: `stale-run-${run.id}:stale`,
        timestamp: run.finishedAt ?? run.startedAt,
        level: 'warning',
        code: 'archive.stale',
        message: jobsT('archiveTaskStaleBody'),
      },
    ],
    resultLink: `/audit?run=${run.id}`,
    error: jobsT('archiveTaskStaleBody'),
  }
}

export function JobsPage() {
  const {
    loading,
    archiveTasks = [],
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
    const archiveRunning = archiveTasks.filter(
      (task) => task.state === 'running' || task.state === 'queued',
    ).length
    const queued = (aiQueue?.queued ?? 0) + (runtime?.queue.queued ?? 0)
    const running =
      (aiQueue?.running ?? 0) + (runtime?.queue.running ?? 0) + archiveRunning
    const failed = (aiQueue?.failed ?? 0) + (runtime?.queue.failed ?? 0)
    return { failed, queued, running }
  }, [aiQueue, archiveTasks, runtime])

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
  const staleArchiveTasks = useMemo(
    () =>
      (snapshot?.recentRuns ?? [])
        .filter(isStaleArchiveRun)
        .map((run) => staleArchiveRunTask(run, jobsT)),
    [jobsT, snapshot?.recentRuns],
  )
  const visibleArchiveTasks = useMemo(
    () => [...archiveTasks, ...staleArchiveTasks],
    [archiveTasks, staleArchiveTasks],
  )

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
    setAction(paused ? jobsT('pauseQueue') : jobsT('resumeQueue'))
    try {
      await saveConfig(nextPausedConfig(snapshot!.config, paused))
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
      const message = describeError(error, 'retry_intelligence_job')
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
      const message = describeError(error, 'cancel_intelligence_job')
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
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="jobs-page"
      >
        <LoadingState label={jobsT('loadingPage')} />
      </div>
    )
  }

  if (!snapshot?.config.initialized) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="jobs-page"
      >
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
      </div>
    )
  }

  if (!snapshot.archiveStatus.unlocked) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="jobs-page"
      >
        <PermissionGate
          detail={jobsT('lockedDetail')}
          eyebrow={jobsT('lockedEyebrow')}
          title={jobsT('lockedTitle')}
        >
          <Link className="btn-primary" to="/security">
            {commonT('reviewSecurity')}
          </Link>
        </PermissionGate>
      </div>
    )
  }

  if (runtimeLoading) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="jobs-page"
      >
        <LoadingState label={jobsT('loadingPage')} />
      </div>
    )
  }

  if (runtimeStatus.error && !aiQueue && !runtime) {
    return (
      <div
        className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
        data-testid="jobs-page"
      >
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
      </div>
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
  const visibleFailedJobsCount =
    (aiQueue?.recentJobs.filter((job) => job.state === 'failed').length ?? 0) +
    (runtime?.recentJobs.filter((job) => job.state === 'failed').length ?? 0)
  const visibleReadableContentRows = readableContentFetchAvailable
    ? (contentPlugin?.storedRecords ?? 0)
    : 0
  let contentQueueMessage = jobsT('contentFetchDeferredBody')
  if (readableContentFetchAvailable) {
    if (!contentPlugin) {
      contentQueueMessage = jobsT('contentFetchFallbackBody')
    } else if (contentPlugin.queuedJobs > 0) {
      contentQueueMessage = jobsT('contentFetchBacklogBody', {
        queued: contentPlugin.queuedJobs,
        stored: contentPlugin.storedRecords,
      })
    } else if (contentPlugin.runningJobs > 0) {
      contentQueueMessage = jobsT('contentFetchRunningBody', {
        stored: contentPlugin.storedRecords,
      })
    } else {
      contentQueueMessage = jobsT('contentFetchReadyBody', {
        stored: contentPlugin.storedRecords,
      })
    }
  }
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
  const showQueueToggle =
    snapshot.config.ai.jobQueuePaused ||
    queueCounts.queued > 0 ||
    queueCounts.running > 0
  const heroHeadline =
    queueCounts.failed > 0
      ? jobsT('overviewHeadlineFailures', { count: queueCounts.failed })
      : snapshot.config.ai.jobQueuePaused && queueCounts.queued > 0
        ? jobsT('overviewHeadlinePaused', { count: queueCounts.queued })
        : queueCounts.running > 0
          ? jobsT('overviewHeadlineRunning', { count: queueCounts.running })
          : queueCounts.queued > 0
            ? jobsT('overviewHeadlineQueued', { count: queueCounts.queued })
            : jobsT('overviewHeadlineIdle')
  return (
    <div
      className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
      data-testid="jobs-page"
    >
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
              {visibleFailedJobsCount > 0 ? (
                <a
                  className="btn-secondary"
                  href="#jobs-recent-activity"
                  onClick={(event) => {
                    const target = document.getElementById(
                      'jobs-recent-activity',
                    )
                    if (!target) return
                    event.preventDefault()
                    const reduceMotion =
                      typeof window.matchMedia === 'function' &&
                      window.matchMedia('(prefers-reduced-motion: reduce)')
                        .matches
                    target.scrollIntoView({
                      behavior: reduceMotion ? 'auto' : 'smooth',
                      block: 'start',
                    })
                    if (!target.hasAttribute('tabindex')) {
                      target.setAttribute('tabindex', '-1')
                    }
                    target.focus({ preventScroll: true })
                  }}
                >
                  {jobsT('jumpToFailures', { count: visibleFailedJobsCount })}
                </a>
              ) : null}
              {showQueueToggle ? (
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
              ) : null}
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
          <PaperCard
            className="jobs-hero-card jobs-hero-card--wide"
            testId="jobs-overview-card"
          >
            <PaperCardHeader
              title={jobsT('overviewTitle')}
              right={
                <PaperCardBadge>
                  {lastActivityAt
                    ? formatRelativeTime(lastActivityAt, language)
                    : jobsT('sidebarIdleDetail')}
                </PaperCardBadge>
              }
            />
            <PaperCardBody>
              <div className="jobs-state-board">
                <div className="jobs-hero-copy">
                  <h2>{heroHeadline}</h2>
                  <p>{jobsT('overviewBody')}</p>
                  <p className="mono-support">{contentQueueMessage}</p>
                </div>
                <div className="jobs-hero-stats">
                  <div
                    className={`jobs-hero-stat ${queueCounts.running > 0 ? 'jobs-hero-stat--active' : ''}`}
                  >
                    <span className="dim">{jobsT('runningCount')}</span>
                    <strong className="mono">
                      {queueCounts.running.toLocaleString(language)}
                    </strong>
                  </div>
                  <div
                    className={`jobs-hero-stat ${
                      snapshot.config.ai.jobQueuePaused &&
                      queueCounts.queued > 0
                        ? 'jobs-hero-stat--warning'
                        : ''
                    }`}
                  >
                    <span className="dim">{jobsT('queuedCount')}</span>
                    <strong className="mono">
                      {queueCounts.queued.toLocaleString(language)}
                    </strong>
                  </div>
                  <div
                    className={`jobs-hero-stat ${queueCounts.failed > 0 ? 'jobs-hero-stat--danger' : ''}`}
                  >
                    <span className="dim">{jobsT('failedCount')}</span>
                    <strong className="mono">
                      {queueCounts.failed.toLocaleString(language)}
                    </strong>
                  </div>
                  <div className="jobs-hero-stat">
                    <span className="dim">{jobsT('savedReadableContent')}</span>
                    <strong className="mono">
                      {visibleReadableContentRows.toLocaleString(language)}
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
            </PaperCardBody>
          </PaperCard>

          <PaperCard
            className="jobs-overview-card"
            testId="jobs-queue-summary-card"
          >
            <PaperCardHeader
              title={jobsT('queueSummaryTitle')}
              right={
                <PaperCardBadge>
                  {snapshot.config.ai.jobQueuePaused
                    ? jobsT('queueStatePaused')
                    : jobsT('queueStateLive')}
                </PaperCardBadge>
              }
            />
            <PaperCardBody className="jobs-panel-stack">
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
            </PaperCardBody>
          </PaperCard>

          <PaperCard
            className="jobs-overview-card"
            testId="jobs-runtime-summary-card"
          >
            <PaperCardHeader
              title={jobsT('runtimeSummaryTitle')}
              right={
                <PaperCardBadge>
                  {(runtime?.queue.queued ?? 0).toLocaleString(language)}{' '}
                  {jobsT('queuedCount').toLowerCase()}
                </PaperCardBadge>
              }
            />
            <PaperCardBody className="jobs-panel-stack">
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
            </PaperCardBody>
          </PaperCard>
        </div>
        <JobsRuntimeHealthSection
          commonT={commonT}
          jobsT={jobsT}
          language={language}
          runtime={runtime}
          settingsT={settingsT}
        />

        <div className="jobs-section-heading">
          <span className="panel-title">{jobsT('archiveTasksTitle')}</span>
          <p>{jobsT('archiveTasksBody')}</p>
        </div>

        <div className="jobs-archive-task-list">
          {visibleArchiveTasks.length > 0 ? (
            visibleArchiveTasks.map((task) => (
              <TaskProgressCard
                key={task.id}
                task={task}
                language={language}
                labels={{
                  started: jobsT('archiveTaskStarted'),
                  updated: jobsT('archiveTaskUpdated'),
                  records: jobsT('archiveTaskRecords'),
                  console: jobsT('archiveTaskConsole'),
                  noLogs: jobsT('archiveTaskNoLogs'),
                }}
                actions={
                  task.resultLink ? (
                    <Link className="btn-secondary" to={task.resultLink}>
                      {jobsT('archiveTaskOpenResult')}
                    </Link>
                  ) : null
                }
              />
            ))
          ) : (
            <StatusCallout
              tone="success"
              title={jobsT('archiveTasksTitle')}
              body={jobsT('archiveTasksBody')}
            />
          )}
        </div>

        <div className="jobs-section-heading" id="jobs-recent-activity">
          <span className="panel-title">{jobsT('recentActivityTitle')}</span>
          <p>{jobsT('recentActivityBody')}</p>
        </div>

        <div className="jobs-summary-grid">
          <JobPanel
            action={action}
            emptyLabel={jobsT('recentJobsEmpty')}
            jobs={aiQueue?.recentJobs ?? []}
            jobsT={jobsT}
            language={language}
            noDetailsLabel={jobsT('noErrorDetails')}
            onCancel={handleCancelAiJob}
            onRetry={handleReplayAiJob}
            title={jobsT('recentAiJobs')}
          />
          <RuntimeJobPanel
            action={action}
            emptyLabel={jobsT('recentJobsEmpty')}
            jobs={runtime?.recentJobs ?? []}
            jobsT={jobsT}
            language={language}
            onCancel={handleCancelRuntimeJob}
            onRetry={handleRetryRuntimeJob}
            settingsT={settingsT}
            title={jobsT('recentRuntimeJobs')}
          />
        </div>
      </div>
    </div>
  )
}
