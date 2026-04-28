/**
 * @file shell-tasks.test.ts
 * @description Unit coverage for shell-owned archive task and notification helpers.
 * @module app/shell-tasks
 */

import { describe, expect, test } from 'vitest'
import type {
  BackupReport,
  ImportProgressEvent,
  TakeoutInspection,
} from '../lib/types'
import {
  addShellNotification,
  appendShellTaskLogs,
  applyBackupProgressToTask,
  applyImportProgressToTask,
  completeBackupTask,
  completeImportTask,
  createShellTask,
  dismissShellNotification,
  failShellTask,
  findActiveArchiveTask,
  markShellNotificationsRead,
  normalizeProgressValue,
  shellNotificationLimit,
  shellTaskLogLimit,
  upsertShellTask,
  type ShellNotification,
  type ShellTask,
} from './shell-tasks'

describe('shell task helpers', () => {
  test('creates, finds, appends, and sorts archive tasks without mutating callers', () => {
    const task = createShellTask({
      id: 'task-import',
      kind: 'import',
      title: 'Import Chrome',
      detail: 'Queued import',
      sourceLabel: 'Chrome',
      profileLabel: 'Default',
      timestamp: '2026-04-27T10:00:00.000Z',
    })
    const completed = { ...task, id: 'done', state: 'succeeded' } as ShellTask
    const backup = createShellTask({
      id: 'task-backup',
      kind: 'backup',
      title: 'Backup',
      detail: 'Queued backup',
      timestamp: '2026-04-27T10:01:00.000Z',
    })

    expect(task).toMatchObject({
      state: 'running',
      sourceLabel: 'Chrome',
      profileLabel: 'Default',
      progressValue: null,
    })
    expect(findActiveArchiveTask([completed, task])).toBe(task)
    expect(findActiveArchiveTask([completed])).toBeUndefined()
    expect(upsertShellTask([task], backup).map((item) => item.id)).toEqual([
      'task-backup',
      'task-import',
    ])
    expect(
      appendShellTaskLogs(task, []).logEntries.map((entry) => entry.id),
    ).toEqual(['task-import:start'])

    const manyLogs = Array.from(
      { length: shellTaskLogLimit + 2 },
      (_, index) => ({
        id: `line-${index}`,
        timestamp: '2026-04-27T10:02:00.000Z',
        level: 'info' as const,
        code: 'test.line',
        message: `Line ${index}`,
      }),
    )
    const capped = appendShellTaskLogs(task, manyLogs)
    expect(capped.logEntries).toHaveLength(shellTaskLogLimit)
    expect(capped.logEntries[0].id).toBe('line-2')
  })

  test('normalizes import progress from structured events before falling back to raw log lines', () => {
    const task = createShellTask({
      id: 'task-import',
      kind: 'import',
      title: 'Import',
      detail: 'Queued import',
      timestamp: '2026-04-27T10:00:00.000Z',
    })
    const structuredProgress: ImportProgressEvent = {
      phase: 'import-file',
      label: 'Importing',
      detail: 'Writing BrowserHistory.json',
      current: 1,
      total: 4,
      progressPercent: undefined,
      logLines: ['raw one'],
      sourcePath: '/tmp/BrowserHistory.json',
      sourceLabel: 'Chrome Default',
      processedRecords: 3,
      totalRecords: 10,
      importedRecords: 2,
      duplicateRecords: 1,
      skippedRecords: 0,
      logEvents: [
        {
          level: 'warning',
          code: 'import.duplicates',
          message: 'Skipped duplicate records.',
          sourceLabel: 'Chrome Default',
          diagnostic: 'source_visit_id already exists',
          processedRecords: 3,
          totalRecords: 10,
        },
      ],
    }

    const updated = applyImportProgressToTask(
      task,
      structuredProgress,
      '2026-04-27T10:00:01.000Z',
    )

    expect(updated).toMatchObject({
      state: 'running',
      progressLabel: '3 / 10',
      progressValue: 30,
      processedRecords: 3,
      totalRecords: 10,
      importedRecords: 2,
      duplicateRecords: 1,
      skippedRecords: 0,
      sourceLabel: 'Chrome Default',
    })
    expect(updated.logEntries.at(-1)).toMatchObject({
      level: 'warning',
      code: 'import.duplicates',
      message: 'Skipped duplicate records.',
      diagnostic: 'source_visit_id already exists',
      current: 3,
      total: 10,
    })

    const rawFallback = applyImportProgressToTask(
      updated,
      {
        ...structuredProgress,
        phase: 'complete',
        progressPercent: 125,
        processedRecords: undefined,
        totalRecords: undefined,
        importedRecords: undefined,
        duplicateRecords: undefined,
        skippedRecords: undefined,
        sourceLabel: null,
        logEvents: [
          {
            level: 'unexpected',
            code: '',
            message: '',
            processedRecords: null,
            totalRecords: null,
          },
        ],
      },
      '2026-04-27T10:00:02.000Z',
    )

    expect(rawFallback.state).toBe('succeeded')
    expect(rawFallback.progressValue).toBe(100)
    expect(rawFallback.finishedAt).toBe('2026-04-27T10:00:02.000Z')
    expect(rawFallback.logEntries.at(-1)).toMatchObject({
      level: 'info',
      code: 'import.complete',
      message: 'Writing BrowserHistory.json',
    })
  })

  test('falls back structured log source and counters without inventing totals', () => {
    const task = createShellTask({
      id: 'task-import',
      kind: 'import',
      title: 'Import',
      detail: 'Queued import',
      sourceLabel: 'Chrome Default',
      timestamp: '2026-04-27T10:00:00.000Z',
    })

    const sourceFallback = applyImportProgressToTask(
      task,
      {
        phase: 'import-file',
        label: 'Importing',
        detail: 'Writing records',
        current: 7,
        total: 12,
        progressPercent: null,
        logLines: [],
        sourceLabel: 'Chrome Default',
        logEvents: [
          {
            level: 'info',
            code: 'import.batch',
            message: 'Batch written.',
          },
        ],
      },
      '2026-04-27T10:00:01.000Z',
    )

    expect(sourceFallback.logEntries.at(-1)).toMatchObject({
      sourceLabel: 'Chrome Default',
      current: 7,
      total: 12,
    })

    const nullFallbackProgress = {
      phase: 'import-file',
      label: 'Importing',
      detail: 'Streaming records',
      current: null,
      total: null,
      progressPercent: null,
      logLines: [],
      sourceLabel: null,
      processedRecords: null,
      totalRecords: null,
      logEvents: [
        {
          level: 'info',
          code: 'import.stream',
          message: 'Streaming without total.',
          processedRecords: null,
          totalRecords: null,
        },
      ],
    } as unknown as ImportProgressEvent
    const nullFallback = applyImportProgressToTask(
      task,
      nullFallbackProgress,
      '2026-04-27T10:00:02.000Z',
    )

    expect(nullFallback.logEntries.at(-1)).toMatchObject({
      sourceLabel: null,
      current: null,
      total: null,
    })
  })

  test('falls back import source labels through source path and existing task labels', () => {
    const task = createShellTask({
      id: 'task-import',
      kind: 'import',
      title: 'Import',
      detail: 'Queued import',
      sourceLabel: 'Existing source',
      timestamp: '2026-04-27T10:00:00.000Z',
    })
    const sourcePathFallback = applyImportProgressToTask(
      task,
      {
        phase: 'import-file',
        label: 'Importing',
        detail: 'Writing records',
        current: 1,
        total: 1,
        progressPercent: null,
        logLines: [],
        sourcePath: '/tmp/BrowserHistory.json',
        sourceLabel: null,
      },
      '2026-04-27T10:00:01.000Z',
    )

    expect(sourcePathFallback.sourceLabel).toBe('/tmp/BrowserHistory.json')

    const taskSourceFallback = applyImportProgressToTask(
      task,
      {
        phase: 'import-file',
        label: 'Importing',
        detail: 'Writing records',
        current: 1,
        total: 1,
        progressPercent: null,
        logLines: [],
        sourcePath: null,
        sourceLabel: null,
      },
      '2026-04-27T10:00:02.000Z',
    )

    expect(taskSourceFallback.sourceLabel).toBe('Existing source')

    const nullSourceFallback = applyImportProgressToTask(
      { ...task, sourceLabel: null },
      {
        phase: 'import-file',
        label: 'Importing',
        detail: 'Writing records',
        current: 1,
        total: 1,
        progressPercent: null,
        logLines: [],
        sourcePath: null,
        sourceLabel: null,
      },
      '2026-04-27T10:00:03.000Z',
    )

    expect(nullSourceFallback.sourceLabel).toBeNull()
  })

  test('normalizes backup progress with record counters, phase counters, and bounded fallbacks', () => {
    const task = createShellTask({
      id: 'task-backup',
      kind: 'backup',
      title: 'Backup',
      detail: 'Queued backup',
      timestamp: '2026-04-27T10:00:00.000Z',
    })

    const recordProgress = applyBackupProgressToTask(
      task,
      {
        phase: 'ingest-profile',
        label: 'Writing archive',
        detail: 'Writing canonical facts',
        step: 1,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 1,
        profileId: 'chrome:Default',
        progressCurrent: 1,
        progressTotal: 1,
        progressPercent: undefined,
        logLines: ['one', 'two', 'three', 'four', 'five'],
        sourceLabel: 'Chrome / Default',
        processedRecords: 12,
        totalRecords: null,
        importedRecords: 10,
        duplicateRecords: 2,
        skippedRecords: Number.NaN,
      },
      '2026-04-27T10:01:00.000Z',
    )

    expect(recordProgress).toMatchObject({
      profileLabel: 'chrome:Default',
      progressLabel: '12',
      progressValue: null,
      processedRecords: 12,
      skippedRecords: null,
    })
    expect(
      recordProgress.logEntries.slice(-4).map((entry) => entry.message),
    ).toEqual(['two', 'three', 'four', 'five'])

    const phaseProgress = applyBackupProgressToTask(
      recordProgress,
      {
        phase: 'stage-profile',
        label: 'Staging',
        detail: 'Copying profile',
        step: 1,
        totalSteps: 3,
        completedProfiles: 1,
        totalProfiles: 2,
        progressCurrent: 1,
        progressTotal: 2,
        progressPercent: -5,
        logLines: [],
      },
      '2026-04-27T10:02:00.000Z',
    )

    expect(phaseProgress.progressLabel).toBe('1 / 2')
    expect(phaseProgress.progressValue).toBe(0)
    expect(phaseProgress.logEntries.at(-1)).toMatchObject({
      code: 'backup.stage-profile',
      message: 'Copying profile',
    })
  })

  test('completes and fails tasks while preserving result links and severity', () => {
    const importTask = createShellTask({
      id: 'import',
      kind: 'import',
      title: 'Import',
      detail: 'Queued import',
      timestamp: '2026-04-27T10:00:00.000Z',
    })
    const backupTask = createShellTask({
      id: 'backup',
      kind: 'backup',
      title: 'Backup',
      detail: 'Queued backup',
      timestamp: '2026-04-27T10:00:00.000Z',
    })

    const imported = completeImportTask(
      importTask,
      takeoutInspection({ batchId: 17 }),
      '2026-04-27T10:03:00.000Z',
      'Import complete',
    )
    expect(imported).toMatchObject({
      state: 'succeeded',
      progressValue: 100,
      resultLink: '/import?batch=17',
      importedRecords: 8,
      duplicateRecords: 2,
    })

    expect(
      completeImportTask(
        importTask,
        takeoutInspection({ batchId: null }),
        '2026-04-27T10:03:00.000Z',
        'Import complete',
      ).resultLink,
    ).toBe('/import')

    const backupReport = backupReportFixture({ warnings: ['Full Disk Access'] })
    const completedBackup = completeBackupTask(
      backupTask,
      backupReport,
      '2026-04-27T10:04:00.000Z',
      'Backup complete',
    )
    expect(completedBackup.resultLink).toBe('/audit?run=42')
    expect(completedBackup.logEntries.at(-1)?.level).toBe('warning')
    expect(
      completeBackupTask(
        backupTask,
        backupReportFixture({ run: null, warnings: [] }),
        '2026-04-27T10:04:00.000Z',
        'Backup complete',
      ).resultLink,
    ).toBe('/jobs')

    const failed = failShellTask(
      backupTask,
      '2026-04-27T10:05:00.000Z',
      'Backup failed',
    )
    expect(failed).toMatchObject({
      state: 'failed',
      error: 'Backup failed',
      finishedAt: '2026-04-27T10:05:00.000Z',
    })
    expect(failed.logEntries.at(-1)).toMatchObject({
      level: 'error',
      code: 'backup.failed',
    })
  })

  test('clamps progress and keeps notification queues bounded and dismissible', () => {
    expect(normalizeProgressValue(null)).toBeNull()
    expect(normalizeProgressValue(Number.NaN)).toBeNull()
    expect(normalizeProgressValue(-10)).toBe(0)
    expect(normalizeProgressValue(120)).toBe(100)
    expect(normalizeProgressValue(48.4)).toBe(48.4)

    const current = Array.from(
      { length: shellNotificationLimit },
      (_, index): ShellNotification => ({
        id: `old-${index}`,
        timestamp: '2026-04-27T10:00:00.000Z',
        title: `Old ${index}`,
        body: 'Older notification',
        tone: 'info',
        read: false,
      }),
    )
    const next = addShellNotification(current, {
      id: 'new',
      timestamp: '2026-04-27T10:01:00.000Z',
      title: 'New',
      body: 'Newest notification',
      tone: 'success',
      taskId: 'task',
      href: '/jobs',
    })

    expect(next).toHaveLength(shellNotificationLimit)
    expect(next[0]).toMatchObject({
      id: 'new',
      read: false,
      taskId: 'task',
      href: '/jobs',
    })
    expect(next.at(-1)?.id).toBe('old-48')
    expect(markShellNotificationsRead(next).every((item) => item.read)).toBe(
      true,
    )
    expect(dismissShellNotification(next, 'new')[0].id).toBe('old-0')
    expect(
      addShellNotification([], {
        id: 'unlinked',
        timestamp: '2026-04-27T10:02:00.000Z',
        title: 'Unlinked',
        body: 'No task metadata',
        tone: 'info',
      }).at(0),
    ).toMatchObject({
      taskId: null,
      href: null,
    })
  })
})

function takeoutInspection({
  batchId,
}: {
  batchId: number | null
}): TakeoutInspection {
  return {
    sourcePath: '/tmp/Takeout',
    dryRun: false,
    recognizedFiles: [],
    quarantinedFiles: [],
    candidateItems: 10,
    importedItems: 8,
    duplicateItems: 2,
    previewEntries: [],
    importBatch:
      batchId === null
        ? null
        : {
            id: batchId,
            sourceKind: 'google-takeout',
            sourcePath: '/tmp/Takeout',
            profileId: 'takeout::browser-history',
            createdAt: '2026-04-27T10:00:00.000Z',
            importedAt: '2026-04-27T10:03:00.000Z',
            revertedAt: null,
            status: 'imported',
            candidateItems: 10,
            importedItems: 8,
            duplicateItems: 2,
            visibleItems: 8,
            auditPath: null,
            gitCommit: null,
          },
    notes: [],
    detectedLocale: null,
    previewRangeStart: null,
    previewRangeEnd: null,
  }
}

function backupReportFixture({
  run = {
    id: 42,
    startedAt: '2026-04-27T10:00:00.000Z',
    finishedAt: '2026-04-27T10:04:00.000Z',
    status: 'success',
    runType: 'backup',
    trigger: 'manual',
    profileScope: ['chrome:Default'],
    manifestHash: null,
    profilesProcessed: 1,
    newVisits: 8,
    newUrls: 5,
    newDownloads: 0,
  },
  warnings,
}: {
  run?: BackupReport['run']
  warnings: string[]
}): BackupReport {
  return {
    dueSkipped: false,
    reason: null,
    run,
    profiles: [],
    manifestPath: null,
    gitCommit: null,
    warnings,
    remoteBackup: null,
  }
}
