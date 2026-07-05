/**
 * Unit coverage for the ambient-task selector.
 *
 * Why this file exists:
 * - `selectAmbientTasks` is the single projection that folds the archive task store and the runtime
 *   AI queue into the shell's ambient bottom bar. Its filtering (running/queued only), clamping,
 *   rounding, and archive-first ordering are the contract the bar depends on, so they are pinned
 *   here independently of the shell wiring.
 */

import { describe, expect, test } from 'vitest'
import {
  ambientModelFromBusyOverlay,
  selectAmbientTasks,
} from './shell-ambient-tasks'
import type { ShellTask } from './shell-tasks'
import type { BusyOverlayState, ShellRuntimeStatus } from './shell-data-context'
import type { AiQueueJob } from '../lib/types'

function archiveTask(overrides: Partial<ShellTask> = {}): ShellTask {
  return {
    id: 'task-1',
    kind: 'import',
    state: 'running',
    title: 'Importing history',
    detail: '',
    startedAt: '2026-06-28T00:00:00Z',
    updatedAt: '2026-06-28T00:00:00Z',
    finishedAt: null,
    sourceLabel: null,
    profileLabel: null,
    progressLabel: null,
    progressValue: null,
    current: null,
    total: null,
    processedRecords: null,
    totalRecords: null,
    importedRecords: null,
    duplicateRecords: null,
    skippedRecords: null,
    logEntries: [],
    resultLink: null,
    error: null,
    ...overrides,
  }
}

function indexJob(overrides: Partial<AiQueueJob> = {}): AiQueueJob {
  return {
    id: 55,
    jobType: 'index-build',
    state: 'running',
    priority: 10,
    attempt: 1,
    maxAttempts: 3,
    runId: null,
    summary: null,
    queuedAt: '2026-06-28T00:00:00Z',
    availableAt: '2026-06-28T00:00:00Z',
    startedAt: '2026-06-28T00:01:00Z',
    finishedAt: null,
    heartbeatAt: null,
    errorCode: null,
    errorMessage: null,
    ...overrides,
  }
}

function runtimeStatus(recentJobs: AiQueueJob[]): ShellRuntimeStatus {
  return {
    aiQueue: {
      paused: false,
      concurrency: 1,
      queued: 0,
      running: recentJobs.length,
      failed: 0,
      indexQueued: 0,
      indexRunning: recentJobs.length,
      recentJobs,
    },
    intelligence: null,
    loading: false,
    error: null,
  }
}

const label = (key: string) => `label:${key}`

describe('selectAmbientTasks', () => {
  test('empty input yields no tasks', () => {
    const model = selectAmbientTasks({
      archiveTasks: [],
      runtimeStatus: null,
      runtimeTaskLabel: label,
    })
    expect(model).toEqual({ count: 0, primary: null, tasks: [] })
  })

  test('a single running archive task passes its label + progress through', () => {
    const model = selectAmbientTasks({
      archiveTasks: [
        archiveTask({ progressValue: 45, progressLabel: '450 / 1000' }),
      ],
      runtimeStatus: null,
      runtimeTaskLabel: label,
    })
    expect(model.count).toBe(1)
    expect(model.primary).toEqual({
      id: 'task-1',
      label: 'Importing history',
      progressValue: 45,
      progressLabel: '450 / 1000',
    })
  })

  test('an out-of-range progress value is clamped to [0, 100]', () => {
    const over = selectAmbientTasks({
      archiveTasks: [archiveTask({ id: 'a', progressValue: 150 })],
      runtimeStatus: null,
      runtimeTaskLabel: label,
    })
    expect(over.primary?.progressValue).toBe(100)

    const under = selectAmbientTasks({
      archiveTasks: [archiveTask({ id: 'b', progressValue: -20 })],
      runtimeStatus: null,
      runtimeTaskLabel: label,
    })
    expect(under.primary?.progressValue).toBe(0)
  })

  test('a null progress value stays indeterminate', () => {
    const model = selectAmbientTasks({
      archiveTasks: [archiveTask({ progressValue: null, progressLabel: null })],
      runtimeStatus: null,
      runtimeTaskLabel: label,
    })
    expect(model.primary?.progressValue).toBeNull()
    expect(model.primary?.progressLabel).toBeNull()
  })

  test('a queued task is included; succeeded/failed tasks are excluded', () => {
    const model = selectAmbientTasks({
      archiveTasks: [
        archiveTask({ id: 'queued', state: 'queued' }),
        archiveTask({ id: 'done', state: 'succeeded' }),
        archiveTask({ id: 'failed', state: 'failed' }),
      ],
      runtimeStatus: null,
      runtimeTaskLabel: label,
    })
    expect(model.tasks.map((task) => task.id)).toEqual(['queued'])
  })

  test('a runtime index-build job maps its label and rounds its progress', () => {
    const model = selectAmbientTasks({
      archiveTasks: [],
      runtimeStatus: runtimeStatus([
        indexJob({ progressEmbedded: 3, progressEmbedTarget: 7 }),
      ]),
      runtimeTaskLabel: label,
    })
    expect(model.count).toBe(1)
    // 3 / 7 = 0.4285… → Math.round(42.857) = 43
    expect(model.primary).toEqual({
      id: 'ai-job-55',
      label: 'label:taskIndexBuild',
      progressValue: 43,
      progressLabel: null,
    })
  })

  test('a runtime job with no progress is indeterminate (null value)', () => {
    const model = selectAmbientTasks({
      archiveTasks: [],
      runtimeStatus: runtimeStatus([indexJob()]),
      runtimeTaskLabel: label,
    })
    expect(model.primary?.progressValue).toBeNull()
  })

  test('archive and runtime tasks combine, archive first, with the right count', () => {
    const model = selectAmbientTasks({
      archiveTasks: [archiveTask({ id: 'archive-1' })],
      runtimeStatus: runtimeStatus([indexJob()]),
      runtimeTaskLabel: label,
    })
    expect(model.count).toBe(2)
    expect(model.tasks.map((task) => task.id)).toEqual([
      'archive-1',
      'ai-job-55',
    ])
    expect(model.primary?.id).toBe('archive-1')
  })
})

describe('ambientModelFromBusyOverlay', () => {
  function overlay(
    overrides: Partial<BusyOverlayState> = {},
  ): BusyOverlayState {
    return { label: 'Backing up', background: true, ...overrides }
  }

  test('projects a background overlay into a single-task model', () => {
    const model = ambientModelFromBusyOverlay(
      overlay({ progressValue: 30, progressLabel: '3 / 10' }),
    )
    expect(model).toEqual({
      count: 1,
      primary: {
        id: 'busy-overlay',
        label: 'Backing up',
        progressValue: 30,
        progressLabel: '3 / 10',
      },
      tasks: [
        {
          id: 'busy-overlay',
          label: 'Backing up',
          progressValue: 30,
          progressLabel: '3 / 10',
        },
      ],
    })
  })

  test('clamps the progress value and defaults a missing progress label to null', () => {
    const model = ambientModelFromBusyOverlay(overlay({ progressValue: 150 }))
    expect(model.primary?.progressValue).toBe(100)
    expect(model.primary?.progressLabel).toBeNull()
  })

  test('keeps an indeterminate (null) progress value indeterminate', () => {
    const model = ambientModelFromBusyOverlay(overlay({ progressValue: null }))
    expect(model.primary?.progressValue).toBeNull()
  })
})
