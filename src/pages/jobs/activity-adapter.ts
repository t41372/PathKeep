/**
 * @file activity-adapter.ts
 * @description Pure function adapter that maps AI queue, runtime, and archive task data into Activity[].
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Map AiQueueJob, IntelligenceJobOverview, ShellTask, and BackupRunOverview into the unified Activity model.
 * - Filter assistant/chat jobs from the AI queue surface.
 * - Provide derived views: needsAttention, runningNow, and recent.
 *
 * ## Not responsible for
 * - Rendering any UI or deciding copy.
 * - Fetching or polling data from the backend.
 */

import type {
  AiQueueStatus,
  AiQueueJob,
  IntelligenceRuntimeSnapshot,
  IntelligenceJobOverview,
  BackupRunOverview,
} from '../../lib/types'
import type { ShellTask } from '../../app/shell-tasks'
import type {
  Activity,
  ActivityKind,
  ActivityState,
  ActivityProgress,
  InterruptionResumability,
} from './activity-types'

export interface ActivityAdapterInput {
  aiQueue: AiQueueStatus | null
  runtime: IntelligenceRuntimeSnapshot | null
  archiveTasks: ShellTask[]
  recentRuns: BackupRunOverview[]
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value))
}

function mapAiJobState(state: string): ActivityState {
  switch (state) {
    case 'running':
      return 'running'
    case 'queued':
      return 'queued'
    case 'failed':
      return 'failed'
    case 'succeeded':
      return 'succeeded'
    case 'cancelled':
      return 'cancelled'
    case 'stale':
      return 'stale'
    case 'paused':
      return 'queued'
    default:
      return 'queued'
  }
}

function terminalOutcomeKey(state: ActivityState): string | undefined {
  switch (state) {
    case 'succeeded':
      return 'outcomeSuccess'
    case 'failed':
      return 'outcomeFailed'
    case 'cancelled':
      return 'outcomeCancelled'
    case 'stale':
      return 'outcomeInterrupted'
    default:
      return undefined
  }
}

function computeIndexBuildProgress(job: AiQueueJob): ActivityProgress {
  const { progressScanned, progressScanTarget, progressEmbedded } = job
  let value: number | null = null
  if (
    progressScanTarget != null &&
    progressScanTarget > 0 &&
    progressScanned != null
  ) {
    value = clamp(progressScanned / progressScanTarget, 0, 1)
  }
  const label = progressEmbedded != null ? `${progressEmbedded}` : null
  return { value, label, labelKind: label !== null ? 'embedded' : null }
}

function aiJobToActivity(job: AiQueueJob): Activity | null {
  const jobType = job.jobType.toLowerCase()
  if (jobType.includes('assistant') || jobType.includes('chat')) return null

  const kind: ActivityKind =
    jobType.includes('re-embed') || jobType.includes('reembed')
      ? 're-embed'
      : 'index-build'

  const state = mapAiJobState(job.state)
  const progress = computeIndexBuildProgress(job)
  const resumability: InterruptionResumability = 'safe'

  const taskNameKey = kind === 're-embed' ? 'taskReEmbed' : 'taskIndexBuild'

  return {
    id: `ai-job-${job.id}`,
    kind,
    state,
    taskNameKey,
    timestamp: job.startedAt ?? job.queuedAt,
    progress,
    resumability,
    aiJobId: job.id,
    outcomeKey: terminalOutcomeKey(state),
  }
}

function runtimeJobToActivity(job: IntelligenceJobOverview): Activity {
  const jobType = job.jobType.toLowerCase()
  let kind: ActivityKind
  if (job.pluginId === 'readable-content-refetch') {
    kind = 'content-fetch'
  } else if (jobType.includes('rebuild')) {
    kind = 'deterministic-rebuild'
  } else if (jobType.includes('re-embed')) {
    kind = 're-embed'
  } else {
    kind = 'content-fetch'
  }

  const state = mapAiJobState(job.state)

  let value: number | null = null
  if (job.progressPercent != null) {
    value = clamp(job.progressPercent / 100, 0, 1)
  } else if (
    job.progressCurrent != null &&
    job.progressTotal != null &&
    job.progressTotal > 0
  ) {
    value = clamp(job.progressCurrent / job.progressTotal, 0, 1)
  }

  const rawLabel = job.progressLabel ?? null
  const progress: ActivityProgress = {
    value,
    label: rawLabel,
    labelKind: rawLabel !== null ? 'verbatim' : null,
  }

  const resumability: InterruptionResumability = 'safe'

  let taskNameKey: string
  switch (kind) {
    case 're-embed':
      taskNameKey = 'taskReEmbed'
      break
    case 'deterministic-rebuild':
      taskNameKey = 'taskDeterministicRebuild'
      break
    default:
      taskNameKey = 'taskContentFetch'
  }

  return {
    id: `runtime-job-${job.id}`,
    kind,
    state,
    taskNameKey,
    timestamp: job.startedAt ?? job.createdAt,
    progress,
    resumability,
    runtimeJobId: job.id,
    cancellable: job.cancellable,
    outcomeKey: terminalOutcomeKey(state),
  }
}

function shellTaskToActivity(task: ShellTask): Activity {
  const kind: ActivityKind = task.kind === 'import' ? 'import' : 'backup'
  const state = task.state as ActivityState

  let value: number | null = null
  if (
    task.processedRecords != null &&
    task.totalRecords != null &&
    task.totalRecords > 0
  ) {
    value = clamp(task.processedRecords / task.totalRecords, 0, 1)
  }
  const hasRecords = task.processedRecords != null && task.totalRecords != null
  const progress: ActivityProgress = {
    value,
    label: null,
    labelKind: hasRecords ? 'records' : null,
    processedCount: task.processedRecords ?? null,
    totalCount: task.totalRecords ?? null,
  }

  const resumability: InterruptionResumability =
    state === 'stale' ? 'cannot-resume' : 'safe'

  let taskNameKey: string
  if (kind === 'import') {
    taskNameKey = state === 'stale' ? 'taskImportStale' : 'taskImportRunning'
  } else {
    taskNameKey = state === 'stale' ? 'taskBackupStale' : 'taskBackupRunning'
  }

  return {
    id: task.id,
    kind,
    state,
    taskNameKey,
    timestamp: task.startedAt,
    progress,
    resumability,
    cause: task.error ?? undefined,
    resultLink: task.resultLink ?? null,
    outcomeKey: terminalOutcomeKey(state),
  }
}

function staleRunToActivity(run: BackupRunOverview): Activity {
  const kind: ActivityKind = run.runType === 'import' ? 'import' : 'backup'
  const state: ActivityState = 'stale'
  const progress: ActivityProgress = {
    value: null,
    label: null,
    labelKind: null,
  }

  const taskNameKey = kind === 'import' ? 'taskImportStale' : 'taskBackupStale'

  return {
    id: `stale-run-${run.id}`,
    kind,
    state,
    taskNameKey,
    timestamp: run.startedAt,
    progress,
    resumability: 'cannot-resume',
    outcomeKey: 'outcomeInterrupted',
  }
}

export function buildActivities(input: ActivityAdapterInput): Activity[] {
  const activities: Activity[] = []

  // AI queue jobs (filter out assistant/chat)
  if (input.aiQueue) {
    for (const job of input.aiQueue.recentJobs) {
      const activity = aiJobToActivity(job)
      if (activity) activities.push(activity)
    }
  }

  // Runtime jobs
  if (input.runtime) {
    for (const job of input.runtime.recentJobs) {
      activities.push(runtimeJobToActivity(job))
    }
  }

  // Archive tasks (ShellTask)
  for (const task of input.archiveTasks) {
    activities.push(shellTaskToActivity(task))
  }

  // Stale runs from recentRuns where status === 'running'
  for (const run of input.recentRuns) {
    const kind = run.runType ?? ''
    if ((kind === 'backup' || kind === 'import') && run.status === 'running') {
      activities.push(staleRunToActivity(run))
    }
  }

  // Sort by most recent first
  return activities.sort((a, b) => {
    const ta = new Date(a.timestamp).getTime()
    const tb = new Date(b.timestamp).getTime()
    return tb - ta
  })
}

export function buildNeedsAttention(activities: Activity[]): Activity[] {
  return activities.filter((a) => a.state === 'failed' || a.state === 'stale')
}

export function buildRunningNow(activities: Activity[]): Activity[] {
  return activities.filter((a) => a.state === 'running' || a.state === 'queued')
}

export function buildRecent(activities: Activity[], limit = 15): Activity[] {
  return activities
    .filter(
      (a) =>
        a.state === 'succeeded' ||
        a.state === 'failed' ||
        a.state === 'cancelled' ||
        a.state === 'stale',
    )
    .sort(
      (a, b) =>
        new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
    )
    .slice(0, limit)
}
