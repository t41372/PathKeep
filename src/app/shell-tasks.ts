/**
 * @file shell-tasks.ts
 * @description Shell-owned archive task and notification helpers for long-running import and backup work.
 * @module app/shell-tasks
 *
 * ## Responsibilities
 * - Define the task, log, and notification contracts shared by shell chrome, Import, Jobs, and sidebar surfaces.
 * - Provide pure helpers for active-task detection, bounded log retention, and notification queue trimming.
 * - Keep archive-write progress state route-independent so navigation cannot hide a running import or backup.
 *
 * ## Not responsible for
 * - Subscribing to Tauri progress events or invoking backend commands.
 * - Rendering task cards, progress bars, or notification popovers.
 * - Persisting completed archive task history beyond the current app session.
 *
 * ## Dependencies
 * - Depends only on the front-end transport types consumed by shell actions.
 *
 * ## Performance notes
 * - Log and notification lists are capped before entering React state to avoid unbounded churn during large imports.
 */

import type {
  BackupProgressEvent,
  BackupReport,
  ImportProgressEvent,
  TakeoutInspection,
} from '../lib/types'

export const shellTaskLogLimit = 160
export const shellNotificationLimit = 50

export type ShellTaskKind = 'import' | 'backup' | 'runtime'
export type ShellTaskState =
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'stale'
export type ShellTaskLogLevel = 'info' | 'success' | 'warning' | 'error'
export type ShellNotificationTone = 'info' | 'success' | 'warning' | 'danger'

export interface ShellTaskLogEntry {
  id: string
  timestamp: string
  level: ShellTaskLogLevel
  code: string
  message: string
  sourceLabel?: string | null
  diagnostic?: string | null
  current?: number | null
  total?: number | null
}

export interface ShellTask {
  id: string
  kind: ShellTaskKind
  state: ShellTaskState
  title: string
  detail: string
  startedAt: string
  updatedAt: string
  finishedAt?: string | null
  sourceLabel?: string | null
  profileLabel?: string | null
  progressLabel?: string | null
  progressValue?: number | null
  current?: number | null
  total?: number | null
  processedRecords?: number | null
  totalRecords?: number | null
  importedRecords?: number | null
  duplicateRecords?: number | null
  skippedRecords?: number | null
  logEntries: ShellTaskLogEntry[]
  resultLink?: string | null
  error?: string | null
}

export interface ShellNotification {
  id: string
  timestamp: string
  title: string
  body: string
  tone: ShellNotificationTone
  read: boolean
  taskId?: string | null
  href?: string | null
}

export interface NewShellTaskInput {
  id: string
  kind: ShellTaskKind
  title: string
  detail: string
  sourceLabel?: string | null
  profileLabel?: string | null
  timestamp: string
}

export interface NewShellNotificationInput {
  id: string
  timestamp: string
  title: string
  body: string
  tone: ShellNotificationTone
  taskId?: string | null
  href?: string | null
}

/**
 * Creates the initial task record used before the first backend progress event arrives.
 *
 * @param input Stable task identity, labels, and timestamp supplied by the shell action owner.
 * @returns A running task with a first bounded log entry.
 */
export function createShellTask(input: NewShellTaskInput): ShellTask {
  return {
    id: input.id,
    kind: input.kind,
    state: 'running',
    title: input.title,
    detail: input.detail,
    startedAt: input.timestamp,
    updatedAt: input.timestamp,
    finishedAt: null,
    sourceLabel: input.sourceLabel ?? null,
    profileLabel: input.profileLabel ?? null,
    progressLabel: null,
    progressValue: null,
    current: null,
    total: null,
    processedRecords: null,
    totalRecords: null,
    importedRecords: null,
    duplicateRecords: null,
    skippedRecords: null,
    logEntries: [
      {
        id: `${input.id}:start`,
        timestamp: input.timestamp,
        level: 'info',
        code: `${input.kind}.started`,
        message: input.detail,
        sourceLabel: input.sourceLabel ?? null,
      },
    ],
    resultLink: null,
    error: null,
  }
}

/**
 * Finds the active archive-write task, if any.
 *
 * @param tasks Current shell task list.
 * @returns The first running or queued import/backup task.
 */
export function findActiveArchiveTask(tasks: readonly ShellTask[]) {
  return tasks.find(
    (task) =>
      (task.kind === 'import' || task.kind === 'backup') &&
      (task.state === 'running' || task.state === 'queued'),
  )
}

/**
 * Inserts or replaces one task while keeping the most recently updated task first.
 *
 * @param tasks Existing task list.
 * @param nextTask Task record to insert or replace.
 * @returns A new task list with stable ordering.
 */
export function upsertShellTask(
  tasks: readonly ShellTask[],
  nextTask: ShellTask,
) {
  const withoutTask = tasks.filter((task) => task.id !== nextTask.id)
  return [nextTask, ...withoutTask].sort((left, right) =>
    right.updatedAt.localeCompare(left.updatedAt),
  )
}

/**
 * Appends task logs while enforcing the shell's bounded retention window.
 *
 * @param task Current task record.
 * @param entries New entries derived from progress or completion.
 * @returns A task copy with recent logs capped.
 */
export function appendShellTaskLogs(
  task: ShellTask,
  entries: readonly ShellTaskLogEntry[],
) {
  if (entries.length === 0) {
    return task
  }

  return {
    ...task,
    logEntries: [...task.logEntries, ...entries].slice(-shellTaskLogLimit),
  }
}

/**
 * Clamps a backend percent value to the UI progress contract.
 *
 * @param value Nullable percent supplied by backend progress.
 * @returns A bounded percent, or null when the task is indeterminate.
 */
export function normalizeProgressValue(value: number | null | undefined) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return null
  }

  return Math.max(0, Math.min(100, value))
}

/**
 * Converts backend import progress into a route-independent task update.
 *
 * @param task Existing import task.
 * @param progress Backend progress payload.
 * @param timestamp Current ISO timestamp.
 * @returns A task with refreshed counters and bounded logs.
 */
export function applyImportProgressToTask(
  task: ShellTask,
  progress: ImportProgressEvent,
  timestamp: string,
) {
  const processedRecords = optionalCount(progress.processedRecords)
  const totalRecords = optionalCount(progress.totalRecords)
  const progressValue =
    normalizeProgressValue(progress.progressPercent) ??
    progressValueFromRecords(processedRecords, totalRecords)
  const progressLabel = recordProgressLabel(
    processedRecords,
    totalRecords,
    progress.current,
    progress.total,
  )
  const nextTask = appendShellTaskLogs(
    {
      ...task,
      state: progress.phase === 'complete' ? 'succeeded' : 'running',
      detail: progress.detail,
      updatedAt: timestamp,
      finishedAt: progress.phase === 'complete' ? timestamp : task.finishedAt,
      sourceLabel:
        progress.sourceLabel ?? progress.sourcePath ?? task.sourceLabel ?? null,
      progressLabel,
      progressValue,
      current: progress.current,
      total: progress.total,
      processedRecords,
      totalRecords,
      importedRecords: optionalCount(progress.importedRecords),
      duplicateRecords: optionalCount(progress.duplicateRecords),
      skippedRecords: optionalCount(progress.skippedRecords),
    },
    progressLogEntries({
      taskId: task.id,
      timestamp,
      fallbackMessage: progress.detail,
      fallbackCode: `import.${progress.phase}`,
      sourceLabel: progress.sourceLabel ?? progress.sourcePath ?? null,
      logEvents: progress.logEvents,
      rawLines: progress.logLines,
      current: processedRecords ?? progress.current,
      total: totalRecords ?? progress.total,
    }),
  )

  return nextTask
}

/**
 * Converts backend backup progress into a route-independent task update.
 *
 * @param task Existing backup task.
 * @param progress Backend progress payload.
 * @param timestamp Current ISO timestamp.
 * @returns A task with refreshed counters and bounded logs.
 */
export function applyBackupProgressToTask(
  task: ShellTask,
  progress: BackupProgressEvent,
  timestamp: string,
) {
  const processedRecords = optionalCount(progress.processedRecords)
  const totalRecords = optionalCount(progress.totalRecords)
  const progressValue =
    normalizeProgressValue(progress.progressPercent) ??
    progressValueFromRecords(processedRecords, totalRecords)
  const progressLabel = recordProgressLabel(
    processedRecords,
    totalRecords,
    progress.progressCurrent ?? progress.completedProfiles,
    progress.progressTotal ?? progress.totalProfiles,
  )

  return appendShellTaskLogs(
    {
      ...task,
      state: 'running',
      detail: progress.detail,
      updatedAt: timestamp,
      sourceLabel:
        progress.sourceLabel ?? progress.profileId ?? task.sourceLabel ?? null,
      profileLabel: progress.profileId ?? task.profileLabel ?? null,
      progressLabel,
      progressValue,
      current: progress.progressCurrent ?? progress.completedProfiles,
      total: progress.progressTotal ?? progress.totalProfiles,
      processedRecords,
      totalRecords,
      importedRecords: optionalCount(progress.importedRecords),
      duplicateRecords: optionalCount(progress.duplicateRecords),
      skippedRecords: optionalCount(progress.skippedRecords),
    },
    progressLogEntries({
      taskId: task.id,
      timestamp,
      fallbackMessage: progress.detail,
      fallbackCode: `backup.${progress.phase}`,
      sourceLabel: progress.sourceLabel ?? progress.profileId ?? null,
      logEvents: progress.logEvents,
      rawLines: progress.logLines,
      current: processedRecords ?? progress.progressCurrent ?? null,
      total: totalRecords ?? progress.progressTotal ?? null,
    }),
  )
}

/**
 * Marks an import task as complete and attaches its review link when the backend returns a batch.
 *
 * @param task Current import task.
 * @param result Backend import result.
 * @param timestamp Completion timestamp.
 * @param message Localized completion message.
 * @returns A succeeded task.
 */
export function completeImportTask(
  task: ShellTask,
  result: TakeoutInspection,
  timestamp: string,
  message: string,
) {
  return appendShellTaskLogs(
    {
      ...task,
      state: 'succeeded',
      detail: message,
      updatedAt: timestamp,
      finishedAt: timestamp,
      progressValue: 100,
      importedRecords: result.importedItems,
      duplicateRecords: result.duplicateItems,
      resultLink: result.importBatch
        ? `/import?batch=${result.importBatch.id}`
        : '/import',
    },
    [
      {
        id: `${task.id}:complete:${timestamp}`,
        timestamp,
        level: 'success',
        code: 'import.complete',
        message,
        current: result.importedItems,
      },
    ],
  )
}

/**
 * Marks a backup task as complete and keeps its run identity available for review.
 *
 * @param task Current backup task.
 * @param report Backend backup report.
 * @param timestamp Completion timestamp.
 * @param message Localized completion message.
 * @returns A succeeded task.
 */
export function completeBackupTask(
  task: ShellTask,
  report: BackupReport,
  timestamp: string,
  message: string,
) {
  return appendShellTaskLogs(
    {
      ...task,
      state: 'succeeded',
      detail: message,
      updatedAt: timestamp,
      finishedAt: timestamp,
      progressValue: 100,
      resultLink: report.run ? `/audit?run=${report.run.id}` : '/jobs',
    },
    [
      {
        id: `${task.id}:complete:${timestamp}`,
        timestamp,
        level: report.warnings.length > 0 ? 'warning' : 'success',
        code: 'backup.complete',
        message,
      },
    ],
  )
}

/**
 * Marks a task as failed while retaining the latest visible progress and logs.
 *
 * @param task Current task.
 * @param timestamp Failure timestamp.
 * @param message User-visible failure message.
 * @returns A failed task.
 */
export function failShellTask(
  task: ShellTask,
  timestamp: string,
  message: string,
) {
  return appendShellTaskLogs(
    {
      ...task,
      state: 'failed',
      detail: message,
      updatedAt: timestamp,
      finishedAt: timestamp,
      error: message,
    },
    [
      {
        id: `${task.id}:failed:${timestamp}`,
        timestamp,
        level: 'error',
        code: `${task.kind}.failed`,
        message,
      },
    ],
  )
}

/**
 * Adds one notification to the persistent queue and caps retention.
 *
 * @param notifications Existing queue.
 * @param input New unread notification.
 * @returns A queue capped to the persistent notification limit.
 */
export function addShellNotification(
  notifications: readonly ShellNotification[],
  input: NewShellNotificationInput,
) {
  const next: ShellNotification = {
    id: input.id,
    timestamp: input.timestamp,
    title: input.title,
    body: input.body,
    tone: input.tone,
    read: false,
    taskId: input.taskId ?? null,
    href: input.href ?? null,
  }

  return [next, ...notifications].slice(0, shellNotificationLimit)
}

/**
 * Marks every queued notification as read after the notification panel opens.
 *
 * @param notifications Existing queue.
 * @returns A new queue with read flags set.
 */
export function markShellNotificationsRead(
  notifications: readonly ShellNotification[],
) {
  return notifications.map((notification) => ({
    ...notification,
    read: true,
  }))
}

/**
 * Removes one notification from the persistent queue.
 *
 * @param notifications Existing queue.
 * @param id Notification id to remove.
 * @returns A new queue without the requested notification.
 */
export function dismissShellNotification(
  notifications: readonly ShellNotification[],
  id: string,
) {
  return notifications.filter((notification) => notification.id !== id)
}

function progressLogEntries(input: {
  taskId: string
  timestamp: string
  fallbackMessage: string
  fallbackCode: string
  sourceLabel?: string | null
  logEvents?: readonly ProgressLogEventLike[] | null
  rawLines?: readonly string[] | null
  current?: number | null
  total?: number | null
}) {
  const structured = input.logEvents?.length
    ? input.logEvents.map((event, index) => ({
        id: `${input.taskId}:${input.timestamp}:structured:${index}`,
        timestamp: input.timestamp,
        level: normalizeLogLevel(event.level),
        code: event.code || input.fallbackCode,
        message: event.message || input.fallbackMessage,
        sourceLabel: event.sourceLabel ?? input.sourceLabel ?? null,
        diagnostic: event.diagnostic ?? null,
        current: event.processedRecords ?? input.current ?? null,
        total: event.totalRecords ?? input.total ?? null,
      }))
    : null

  if (structured) {
    return structured
  }

  const raw = input.rawLines?.length ? input.rawLines : [input.fallbackMessage]
  return raw.slice(-4).map((line, index) => ({
    id: `${input.taskId}:${input.timestamp}:raw:${index}`,
    timestamp: input.timestamp,
    level: 'info' as const,
    code: input.fallbackCode,
    message: line,
    sourceLabel: input.sourceLabel ?? null,
    current: input.current ?? null,
    total: input.total ?? null,
  }))
}

interface ProgressLogEventLike {
  level?: string | null
  code?: string | null
  message?: string | null
  sourceLabel?: string | null
  diagnostic?: string | null
  processedRecords?: number | null
  totalRecords?: number | null
}

function normalizeLogLevel(
  level: string | null | undefined,
): ShellTaskLogLevel {
  switch (level) {
    case 'success':
    case 'warning':
    case 'error':
      return level
    default:
      return 'info'
  }
}

function optionalCount(value: number | null | undefined) {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
    ? value
    : null
}

function progressValueFromRecords(
  processedRecords: number | null,
  totalRecords: number | null,
) {
  if (processedRecords === null || totalRecords === null || totalRecords <= 0) {
    return null
  }

  return normalizeProgressValue((processedRecords / totalRecords) * 100)
}

function recordProgressLabel(
  processedRecords: number | null,
  totalRecords: number | null,
  current: number | null | undefined,
  total: number | null | undefined,
) {
  if (processedRecords !== null && totalRecords !== null && totalRecords > 0) {
    return `${processedRecords.toLocaleString()} / ${totalRecords.toLocaleString()}`
  }

  if (processedRecords !== null) {
    return processedRecords.toLocaleString()
  }

  if (typeof current === 'number' && typeof total === 'number' && total > 0) {
    return `${current.toLocaleString()} / ${total.toLocaleString()}`
  }

  return null
}
