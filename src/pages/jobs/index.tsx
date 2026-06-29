/**
 * @file index.tsx
 * @description Activity center — the redesigned background-tasks page for PathKeep.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Route entry for /jobs: render the Activity center with needs-attention, running-now,
 *   background-features, and recent-activity zones.
 * - Manage queue pause/resume, AI job retry/cancel, and runtime job retry/cancel mutations.
 * - Pass pre-computed Activity[] and queue counts to child zones; no business logic in zones.
 *
 * ## Not responsible for
 * - Polling or fetching runtime data (shell data context owns that lifecycle).
 * - Rendering job details or panel expansions.
 *
 * ## Source-of-truth notes
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose and navigation.
 * - Stay aligned with `docs/features/intelligence.md` for queue semantics.
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { PermissionGate } from '../../components/primitives/permission-gate'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  aiJobMutationNeedsRefresh,
  runtimeJobMutationNeedsRefresh,
} from '../../lib/intelligence-presentation'
import { useModelDownloadProgress } from '../../lib/ipc/model-download'
import type { AppConfig, BackupRunOverview } from '../../lib/types'
import type { ShellTask } from '../../app/shell-tasks'
import {
  buildActivities,
  buildNeedsAttention,
  buildRunningNow,
  buildRecent,
} from './activity-adapter'
import { ActivityHeader } from './activity-header'
import { NeedsAttentionZone } from './needs-attention-zone'
import { RunningNowZone } from './running-now-zone'
import { BackgroundFeaturesZone } from './background-features-zone'
import { RecentZone } from './recent-zone'

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
  const commonT = ns('common')
  const [pageError, setPageError] = useState<string | null>(null)
  const [action, setAction] = useState<string | null>(null)
  const [announcement, setAnnouncement] = useState('')

  const aiQueue = runtimeStatus.aiQueue
  const runtime = runtimeStatus.intelligence
  const runtimeLoading =
    runtimeStatus.loading ||
    (!runtimeStatus.error && aiQueue === null && runtime === null)

  // Stale archive detection (reuse from old code)
  const staleArchiveTasks = useMemo(
    () =>
      (snapshot?.recentRuns ?? [])
        .filter(isStaleArchiveRun)
        .map((run) => staleArchiveRunTask(run, jobsT)),
    [jobsT, snapshot?.recentRuns],
  )
  const allArchiveTasks = useMemo(
    () => [...archiveTasks, ...staleArchiveTasks],
    [archiveTasks, staleArchiveTasks],
  )

  // Model download hook
  const modelDownload = useModelDownloadProgress(
    snapshot?.aiStatus?.staticEmbedding?.modelDownloaded ?? false,
  )
  const showModelDownload = modelDownload.phase === 'downloading'

  // Build activities
  // Note: allArchiveTasks already includes stale runs converted to ShellTasks via staleArchiveTasks,
  // so we pass an empty recentRuns to avoid double-counting stale runs.
  const activities = useMemo(
    () =>
      buildActivities({
        aiQueue,
        runtime,
        archiveTasks: allArchiveTasks,
        recentRuns: [],
      }),
    [aiQueue, runtime, allArchiveTasks],
  )
  const needsAttention = useMemo(
    () => buildNeedsAttention(activities),
    [activities],
  )
  const runningNow = useMemo(() => buildRunningNow(activities), [activities])
  const recent = useMemo(() => buildRecent(activities, 15), [activities])

  // ── aria-live announcer ────────────────────────────────────────────────────
  // Announces task completion/error and 25/50/75% progress milestones to
  // assistive technology without announcing every polling tick.
  const prevRunningCountRef = useRef<number | null>(null)
  const prevAttentionCountRef = useRef<number>(0)
  const milestoneTrackerRef = useRef<Record<string, Set<number>>>({})

  useEffect(() => {
    const currentRunning = runningNow.length + (showModelDownload ? 1 : 0)
    const currentAttention = needsAttention.length

    // Skip initial mount — no previous state to compare against.
    if (prevRunningCountRef.current === null) {
      prevRunningCountRef.current = currentRunning
      prevAttentionCountRef.current = currentAttention
      return
    }

    let msg = ''

    // Tasks completed (running count dropped)
    if (currentRunning < prevRunningCountRef.current && currentRunning === 0) {
      msg = jobsT('headerSummaryNoActivity')
    }

    // New failures surfaced
    if (currentAttention > prevAttentionCountRef.current) {
      msg = jobsT('headerSummaryFailedIdle', { failed: currentAttention })
    }

    // Progress milestones (25 / 50 / 75%) per running activity
    const milestones = [0.25, 0.5, 0.75] as const
    for (const activity of runningNow) {
      if (activity.progress.value != null) {
        if (!milestoneTrackerRef.current[activity.id]) {
          milestoneTrackerRef.current[activity.id] = new Set()
        }
        for (const m of milestones) {
          if (
            activity.progress.value >= m &&
            !milestoneTrackerRef.current[activity.id].has(m)
          ) {
            milestoneTrackerRef.current[activity.id].add(m)
            msg = `${jobsT(activity.taskNameKey)}: ${Math.round(m * 100)}%`
          }
        }
      }
    }

    if (msg) setAnnouncement(msg)
    prevRunningCountRef.current = currentRunning
    prevAttentionCountRef.current = currentAttention
  }, [runningNow, needsAttention, showModelDownload, jobsT])

  // Queue state
  const queuePaused = snapshot?.config.ai.jobQueuePaused ?? false
  const queueCounts = useMemo(() => {
    const archiveRunning = archiveTasks.filter(
      (task) => task.state === 'running' || task.state === 'queued',
    ).length
    const queued = (aiQueue?.queued ?? 0) + (runtime?.queue.queued ?? 0)
    // Fold the model download (per-file, NOT resumable) into the running count so the
    // header summary reflects all interruptible background work, including the download.
    const running =
      (aiQueue?.running ?? 0) +
      (runtime?.queue.running ?? 0) +
      archiveRunning +
      (showModelDownload ? 1 : 0)
    const failed = (aiQueue?.failed ?? 0) + (runtime?.queue.failed ?? 0)
    return { failed, queued, running }
  }, [aiQueue, archiveTasks, runtime, showModelDownload])

  // Header summary
  const headerSummary = useMemo(() => {
    if (queueCounts.failed > 0 && queueCounts.running > 0)
      return jobsT('headerSummaryFailed', {
        failed: queueCounts.failed,
        running: queueCounts.running,
      })
    if (queueCounts.failed > 0)
      return jobsT('headerSummaryFailedIdle', { failed: queueCounts.failed })
    if (queuePaused && queueCounts.queued > 0)
      return jobsT('headerSummaryPausedQueued', { queued: queueCounts.queued })
    if (queueCounts.running > 0 && queueCounts.queued > 0)
      return jobsT('headerSummaryRunningWaiting', {
        running: queueCounts.running,
        queued: queueCounts.queued,
      })
    if (queueCounts.running > 0) {
      // Only append "safe to close" when every running task is fully resumable.
      // The model download is per-file: finished files survive a quit but the
      // in-progress file restarts, so we must not tell the user it's safe to close.
      return showModelDownload
        ? jobsT('headerSummaryRunningNotSafe', { running: queueCounts.running })
        : jobsT('headerSummaryRunning', { running: queueCounts.running })
    }
    // All caught up
    const lastActivityAt =
      runtime?.queue.lastActivityAt ??
      aiQueue?.recentJobs[0]?.finishedAt ??
      null
    if (lastActivityAt)
      return jobsT('headerSummaryAllCaughtUp', {
        time: formatRelativeTime(lastActivityAt, language),
      })
    return jobsT('headerSummaryNoActivity')
  }, [
    aiQueue,
    jobsT,
    language,
    queueCounts,
    queuePaused,
    runtime,
    showModelDownload,
  ])

  const showQueueToggle =
    queuePaused || queueCounts.queued > 0 || queueCounts.running > 0

  // ── Handlers ───────────────────────────────────────────────────────────────

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
      await Promise.all([refreshAppData(), refreshRuntimeStatus()])
      setPageError(null)
    } catch (error) {
      const message = describeError(error, 'replay_ai_job')
      if (aiJobMutationNeedsRefresh(message)) {
        await Promise.all([refreshAppData(), refreshRuntimeStatus()])
        return
      }
      setPageError(message)
    } finally {
      setAction(null)
    }
  }

  async function handleCancelAiJob(jobId: number) {
    setAction(jobsT('cancelJob'))
    try {
      await backend.cancelAiJob(jobId)
      await Promise.all([refreshAppData(), refreshRuntimeStatus()])
      setPageError(null)
    } catch (error) {
      const message = describeError(error, 'cancel_ai_job')
      if (aiJobMutationNeedsRefresh(message)) {
        await Promise.all([refreshAppData(), refreshRuntimeStatus()])
        return
      }
      setPageError(message)
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

  async function handleRetryBackup() {
    setAction(jobsT('actionRetryBackup'))
    try {
      await backend.runBackupNow()
      await Promise.all([refreshAppData(), refreshRuntimeStatus()])
      setPageError(null)
    } catch (error) {
      setPageError(describeError(error, 'retry_backup'))
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

  // ── Gates ──────────────────────────────────────────────────────────────────

  if (loading && !snapshot) {
    return <ActivitySkeleton />
  }

  if (!snapshot?.config.initialized) {
    return (
      <div data-testid="jobs-page">
        <EmptyState
          title={jobsT('setupTitle')}
          description={jobsT('setupDescription')}
          eyebrow={jobsT('statusEyebrow')}
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
      <div data-testid="jobs-page">
        <PermissionGate
          title={jobsT('lockedTitle')}
          eyebrow={jobsT('lockedEyebrow')}
          detail={jobsT('lockedDetail')}
        >
          <Link className="btn-primary" to="/security">
            {commonT('reviewSecurity')}
          </Link>
        </PermissionGate>
      </div>
    )
  }

  if (runtimeLoading) {
    return <ActivitySkeleton />
  }

  // ── Main render ────────────────────────────────────────────────────────────

  return (
    <main
      aria-labelledby="activity-page-heading"
      className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
      data-testid="jobs-page"
    >
      {/* Visually-hidden live region: announces task completion/error and
          25/50/75% progress milestones to screen readers, not every tick. */}
      <div
        className="sr-only"
        aria-live="polite"
        aria-atomic="true"
        data-testid="activity-live-announcer"
      >
        {announcement}
      </div>

      <ActivityHeader
        summary={headerSummary}
        queuePaused={queuePaused}
        showToggle={showQueueToggle}
        onPauseChange={(paused) => void handlePauseChange(paused)}
        action={action}
      />

      {queuePaused && queueCounts.queued > 0 && (
        <StatusCallout
          tone="warning"
          title={jobsT('pausedQueueCallout', { count: queueCounts.queued })}
          body={jobsT('pausedQueueBody')}
        />
      )}

      {pageError && (
        <StatusCallout
          tone="warning"
          title={jobsT('pageUnavailableTitle')}
          body={pageError}
        />
      )}

      {runtimeStatus.error && !pageError && (
        <StatusCallout
          tone="warning"
          title={jobsT('pageUnavailableTitle')}
          body={runtimeStatus.error}
        />
      )}

      {needsAttention.length > 0 && (
        <NeedsAttentionZone
          activities={needsAttention}
          onRetry={(jobId) => void handleReplayAiJob(jobId)}
          onRetryRuntimeJob={(jobId) => void handleRetryRuntimeJob(jobId)}
          onRetryBackup={() => void handleRetryBackup()}
          action={action}
          jobsT={jobsT}
          language={language}
        />
      )}

      {(runningNow.length > 0 || showModelDownload) && (
        <RunningNowZone
          activities={runningNow}
          modelDownload={modelDownload}
          showModelDownload={showModelDownload}
          onPauseChange={handlePauseChange}
          onCancel={(jobId) => void handleCancelAiJob(jobId)}
          onCancelRuntime={(jobId) => void handleCancelRuntimeJob(jobId)}
          action={action}
          jobsT={jobsT}
          language={language}
        />
      )}

      <BackgroundFeaturesZone
        aiStatus={snapshot.aiStatus}
        runtime={runtime}
        jobsT={jobsT}
        language={language}
      />

      <RecentZone activities={recent} jobsT={jobsT} language={language} />
    </main>
  )
}

/**
 * Loading skeleton shown while the archive snapshot and runtime status are
 * loading. Matches the first visible section structure so the page does not
 * produce a jarring flash when data arrives.
 *
 * Animation is gated on prefers-reduced-motion: no-preference.
 */
function ActivitySkeleton() {
  return (
    <div
      data-testid="activity-page-skeleton"
      className="activity-skeleton mx-auto w-full max-w-[1080px] pt-7"
      aria-hidden="true"
    >
      <div className="activity-skeleton__header">
        <div className="activity-skeleton__bar activity-skeleton__bar--title" />
        <div className="activity-skeleton__bar activity-skeleton__bar--summary" />
      </div>
      <div className="activity-skeleton__section">
        <div className="activity-skeleton__row" />
      </div>
      <div className="activity-skeleton__chips">
        <div className="activity-skeleton__chip" />
        <div className="activity-skeleton__chip" />
        <div className="activity-skeleton__chip" />
      </div>
    </div>
  )
}
