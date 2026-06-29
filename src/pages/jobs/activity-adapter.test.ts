/**
 * @file activity-adapter.test.ts
 * @description Pure unit tests for the Activity center adapter functions.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Verify buildActivities correctly maps all data sources to Activity[].
 * - Verify filtering, progress computation, and derived views work correctly.
 *
 * ## Not responsible for
 * - Rendering or UI behavior (use index.test.tsx for that).
 */

import { describe, expect, test } from 'vitest'
import {
  buildActivities,
  buildNeedsAttention,
  buildRunningNow,
  buildRecent,
  type ActivityAdapterInput,
} from './activity-adapter'
import type { Activity } from './activity-types'
import type {
  AiQueueStatus,
  AiQueueJob,
  BackupRunOverview,
  IntelligenceRuntimeSnapshot,
  IntelligenceJobOverview,
} from '../../lib/types'
import type { ShellTask } from '../../app/shell-tasks'

// ── Fixture helpers ───────────────────────────────────────────────────────────

function makeAiQueueJob(overrides: Partial<AiQueueJob> = {}): AiQueueJob {
  return {
    id: 1,
    jobType: 'index-build',
    state: 'running',
    priority: 10,
    attempt: 1,
    maxAttempts: 3,
    runId: null,
    summary: null,
    queuedAt: '2026-04-07T10:00:00Z',
    availableAt: '2026-04-07T10:00:00Z',
    startedAt: '2026-04-07T10:01:00Z',
    finishedAt: null,
    heartbeatAt: null,
    errorCode: null,
    errorMessage: null,
    ...overrides,
  }
}

function makeAiQueueStatus(
  overrides: Partial<AiQueueStatus> = {},
): AiQueueStatus {
  return {
    paused: false,
    concurrency: 1,
    queued: 0,
    running: 0,
    failed: 0,
    indexQueued: 0,
    indexRunning: 0,
    recentJobs: [],
    ...overrides,
  }
}

function makeShellTask(overrides: Partial<ShellTask> = {}): ShellTask {
  return {
    id: 'task-import-1',
    kind: 'import',
    state: 'running',
    title: 'Import browser history',
    detail: 'Writing archive records',
    startedAt: '2026-04-07T10:00:00Z',
    updatedAt: '2026-04-07T10:01:00Z',
    finishedAt: null,
    logEntries: [],
    ...overrides,
  }
}

function makeBackupRunOverview(
  overrides: Partial<BackupRunOverview> = {},
): BackupRunOverview {
  return {
    id: 42,
    startedAt: '2026-04-07T09:00:00Z',
    finishedAt: null,
    status: 'running',
    runType: 'backup',
    profilesProcessed: 1,
    newVisits: 100,
    newUrls: 50,
    newDownloads: 0,
    ...overrides,
  }
}

function makeRuntimeJob(
  overrides: Partial<IntelligenceJobOverview> = {},
): IntelligenceJobOverview {
  return {
    id: 10,
    jobType: 'content-fetch',
    state: 'running',
    attempt: 1,
    createdAt: '2026-04-07T10:00:00Z',
    startedAt: '2026-04-07T10:01:00Z',
    updatedAt: '2026-04-07T10:01:00Z',
    retryable: true,
    cancellable: true,
    ...overrides,
  }
}

function makeRuntime(
  jobs: IntelligenceJobOverview[],
): IntelligenceRuntimeSnapshot {
  return {
    queue: {
      queued: 0,
      running: jobs.filter((j) => j.state === 'running').length,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      lastActivityAt: null,
    },
    plugins: [],
    modules: [],
    recentJobs: jobs,
    notes: [],
  }
}

function emptyInput(): ActivityAdapterInput {
  return {
    aiQueue: null,
    runtime: null,
    archiveTasks: [],
    recentRuns: [],
  }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('buildActivities', () => {
  test('maps index-build running job → kind=index-build, state=running, progress from scanned/target', () => {
    const job = makeAiQueueJob({
      jobType: 'index-build',
      state: 'running',
      progressScanned: 5000,
      progressScanTarget: 10000,
      progressEmbedded: 4800,
    })
    const input: ActivityAdapterInput = {
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    }
    const activities = buildActivities(input)
    expect(activities).toHaveLength(1)
    const act = activities[0]
    expect(act.kind).toBe('index-build')
    expect(act.state).toBe('running')
    expect(act.progress.value).toBeCloseTo(0.5)
    expect(act.aiJobId).toBe(1)
  })

  test('progressScanTarget=0 → value=null (indeterminate)', () => {
    const job = makeAiQueueJob({
      progressScanned: 100,
      progressScanTarget: 0,
    })
    const input: ActivityAdapterInput = {
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    }
    const activities = buildActivities(input)
    expect(activities[0].progress.value).toBeNull()
  })

  test('progress is clamped to [0,1] even if scanned > target', () => {
    const job = makeAiQueueJob({
      progressScanned: 12000,
      progressScanTarget: 10000,
    })
    const input: ActivityAdapterInput = {
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    }
    const activities = buildActivities(input)
    expect(activities[0].progress.value).toBe(1)
  })

  test('filters out assistant/chat jobs', () => {
    const assistantJob = makeAiQueueJob({
      jobType: 'assistant-chat',
      state: 'running',
    })
    const chatJob = makeAiQueueJob({ id: 2, jobType: 'chat', state: 'queued' })
    const indexJob = makeAiQueueJob({
      id: 3,
      jobType: 'index-build',
      state: 'running',
    })
    const input: ActivityAdapterInput = {
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({
        recentJobs: [assistantJob, chatJob, indexJob],
      }),
    }
    const activities = buildActivities(input)
    expect(activities).toHaveLength(1)
    expect(activities[0].aiJobId).toBe(3)
  })

  test('stale backup run from recentRuns → state=stale, kind=backup', () => {
    const run = makeBackupRunOverview({
      runType: 'backup',
      status: 'running',
    })
    const input: ActivityAdapterInput = {
      ...emptyInput(),
      recentRuns: [run],
    }
    const activities = buildActivities(input)
    expect(activities).toHaveLength(1)
    expect(activities[0].state).toBe('stale')
    expect(activities[0].kind).toBe('backup')
    expect(activities[0].id).toBe('stale-run-42')
  })

  test('running import ShellTask → state=running, resumability=safe', () => {
    const task = makeShellTask({
      kind: 'import',
      state: 'running',
    })
    const input: ActivityAdapterInput = {
      ...emptyInput(),
      archiveTasks: [task],
    }
    const activities = buildActivities(input)
    expect(activities).toHaveLength(1)
    expect(activities[0].state).toBe('running')
    expect(activities[0].kind).toBe('import')
    expect(activities[0].resumability).toBe('safe')
  })

  test('stale ShellTask → state=stale, resumability=cannot-resume', () => {
    const task = makeShellTask({
      kind: 'backup',
      state: 'stale',
    })
    const input: ActivityAdapterInput = {
      ...emptyInput(),
      archiveTasks: [task],
    }
    const activities = buildActivities(input)
    expect(activities).toHaveLength(1)
    expect(activities[0].state).toBe('stale')
    expect(activities[0].resumability).toBe('cannot-resume')
  })

  test('recentRuns stale import → kind=import', () => {
    const run = makeBackupRunOverview({
      runType: 'import',
      status: 'running',
    })
    const input: ActivityAdapterInput = {
      ...emptyInput(),
      recentRuns: [run],
    }
    const activities = buildActivities(input)
    expect(activities[0].kind).toBe('import')
  })

  test('recentRuns with non-running status are excluded', () => {
    const run = makeBackupRunOverview({ status: 'succeeded' })
    const input: ActivityAdapterInput = {
      ...emptyInput(),
      recentRuns: [run],
    }
    const activities = buildActivities(input)
    expect(activities).toHaveLength(0)
  })
})

describe('buildNeedsAttention', () => {
  test('returns only failed and stale activities', () => {
    const activities: Activity[] = [
      {
        id: '1',
        kind: 'index-build',
        state: 'running',
        taskNameKey: 'taskIndexBuild',
        timestamp: '2026-04-07T10:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'safe',
      },
      {
        id: '2',
        kind: 'backup',
        state: 'failed',
        taskNameKey: 'taskBackupRunning',
        timestamp: '2026-04-07T09:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'safe',
        outcomeKey: 'outcomeFailed',
      },
      {
        id: '3',
        kind: 'import',
        state: 'stale',
        taskNameKey: 'taskImportStale',
        timestamp: '2026-04-07T08:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'cannot-resume',
        outcomeKey: 'outcomeInterrupted',
      },
      {
        id: '4',
        kind: 'index-build',
        state: 'succeeded',
        taskNameKey: 'taskIndexBuild',
        timestamp: '2026-04-06T10:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'safe',
        outcomeKey: 'outcomeSuccess',
      },
    ]
    const result = buildNeedsAttention(activities)
    expect(result).toHaveLength(2)
    expect(result.map((a) => a.id)).toContain('2')
    expect(result.map((a) => a.id)).toContain('3')
  })
})

describe('buildRunningNow', () => {
  test('returns only running and queued activities', () => {
    const activities: Activity[] = [
      {
        id: '1',
        kind: 'index-build',
        state: 'running',
        taskNameKey: 'taskIndexBuild',
        timestamp: '2026-04-07T10:00:00Z',
        progress: { value: 0.5, label: null, labelKind: null },
        resumability: 'safe',
      },
      {
        id: '2',
        kind: 'content-fetch',
        state: 'queued',
        taskNameKey: 'taskContentFetch',
        timestamp: '2026-04-07T09:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'safe',
      },
      {
        id: '3',
        kind: 'backup',
        state: 'failed',
        taskNameKey: 'taskBackupRunning',
        timestamp: '2026-04-07T08:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'safe',
      },
      {
        id: '4',
        kind: 'import',
        state: 'succeeded',
        taskNameKey: 'taskImportRunning',
        timestamp: '2026-04-06T10:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'safe',
      },
    ]
    const result = buildRunningNow(activities)
    expect(result).toHaveLength(2)
    expect(result.map((a) => a.id)).toContain('1')
    expect(result.map((a) => a.id)).toContain('2')
  })
})

describe('buildRecent', () => {
  test('caps at 15 items when given 20 terminal activities', () => {
    const activities: Activity[] = Array.from({ length: 20 }, (_, i) => ({
      id: `item-${i}`,
      kind: 'index-build' as const,
      state: 'succeeded' as const,
      taskNameKey: 'taskIndexBuild',
      timestamp: `2026-04-${String(i + 1).padStart(2, '0')}T10:00:00Z`,
      progress: { value: null, label: null, labelKind: null },
      resumability: 'safe' as const,
      outcomeKey: 'outcomeSuccess',
    }))
    const result = buildRecent(activities, 15)
    expect(result).toHaveLength(15)
  })

  test('includes failed and stale in recent', () => {
    const activities: Activity[] = [
      {
        id: '1',
        kind: 'index-build',
        state: 'failed',
        taskNameKey: 'taskIndexBuild',
        timestamp: '2026-04-07T10:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'safe',
        outcomeKey: 'outcomeFailed',
      },
      {
        id: '2',
        kind: 'import',
        state: 'stale',
        taskNameKey: 'taskImportStale',
        timestamp: '2026-04-07T09:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'cannot-resume',
        outcomeKey: 'outcomeInterrupted',
      },
      {
        id: '3',
        kind: 'backup',
        state: 'succeeded',
        taskNameKey: 'taskBackupRunning',
        timestamp: '2026-04-07T08:00:00Z',
        progress: { value: null, label: null, labelKind: null },
        resumability: 'safe',
        outcomeKey: 'outcomeSuccess',
      },
      {
        id: '4',
        kind: 'index-build',
        state: 'running',
        taskNameKey: 'taskIndexBuild',
        timestamp: '2026-04-07T11:00:00Z',
        progress: { value: 0.3, label: null, labelKind: null },
        resumability: 'safe',
      },
    ]
    const result = buildRecent(activities)
    // Should include failed, stale, succeeded but NOT running
    expect(result).toHaveLength(3)
    expect(result.some((a) => a.id === '4')).toBe(false)
  })
})

// ── mapAiJobState branches ────────────────────────────────────────────────────
// mapAiJobState is exercised via buildActivities → aiJobToActivity; we use
// terminal-state jobs to hit all branches including the outcomeKey path.

describe('mapAiJobState — all terminal states', () => {
  test('succeeded AI job → state=succeeded, outcomeKey=outcomeSuccess', () => {
    const job = makeAiQueueJob({
      jobType: 'index-build',
      state: 'succeeded',
      finishedAt: '2026-04-07T11:00:00Z',
    })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].state).toBe('succeeded')
    expect(activities[0].outcomeKey).toBe('outcomeSuccess')
  })

  test('cancelled AI job → state=cancelled, outcomeKey=outcomeCancelled', () => {
    const job = makeAiQueueJob({
      jobType: 'index-build',
      state: 'cancelled',
      finishedAt: '2026-04-07T11:00:00Z',
    })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].state).toBe('cancelled')
    expect(activities[0].outcomeKey).toBe('outcomeCancelled')
  })

  test('stale AI job → state=stale, outcomeKey=outcomeInterrupted', () => {
    const job = makeAiQueueJob({ jobType: 'index-build', state: 'stale' })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].state).toBe('stale')
    expect(activities[0].outcomeKey).toBe('outcomeInterrupted')
  })

  test('paused AI job → state=queued (paused maps to queued)', () => {
    const job = makeAiQueueJob({ jobType: 'index-build', state: 'paused' })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].state).toBe('queued')
  })

  test('queued AI job → state=queued', () => {
    const job = makeAiQueueJob({ jobType: 'index-build', state: 'queued' })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].state).toBe('queued')
  })

  test('unknown AI job state → state=queued (default fallthrough)', () => {
    const job = makeAiQueueJob({
      jobType: 'index-build',
      state: 'pending-unknown-state',
    })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].state).toBe('queued')
  })

  test('failed AI job → state=failed, outcomeKey=outcomeFailed', () => {
    const job = makeAiQueueJob({
      jobType: 'index-build',
      state: 'failed',
      finishedAt: '2026-04-07T11:00:00Z',
    })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].state).toBe('failed')
    expect(activities[0].outcomeKey).toBe('outcomeFailed')
  })

  test('re-embed job type → kind=re-embed, taskNameKey=taskReEmbed', () => {
    const job = makeAiQueueJob({ jobType: 're-embed', state: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].kind).toBe('re-embed')
    expect(activities[0].taskNameKey).toBe('taskReEmbed')
  })

  test('reembed (no hyphen) job type → kind=re-embed', () => {
    const job = makeAiQueueJob({ jobType: 'reembed-all', state: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].kind).toBe('re-embed')
  })

  test('AI job uses startedAt when set, falls back to queuedAt', () => {
    const job = makeAiQueueJob({
      jobType: 'index-build',
      state: 'running',
      startedAt: '2026-04-07T10:01:00Z',
      queuedAt: '2026-04-07T10:00:00Z',
    })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].timestamp).toBe('2026-04-07T10:01:00Z')
  })

  test('AI job uses queuedAt when startedAt is null', () => {
    const job = makeAiQueueJob({
      jobType: 'index-build',
      state: 'queued',
      startedAt: null,
      queuedAt: '2026-04-07T10:00:00Z',
    })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].timestamp).toBe('2026-04-07T10:00:00Z')
  })

  test('progressEmbedded=undefined in computeIndexBuildProgress → label=null (no phantom zero)', () => {
    const job = makeAiQueueJob({
      jobType: 'index-build',
      state: 'running',
      progressScanned: 5000,
      progressScanTarget: 10000,
      progressEmbedded: undefined,
    })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    // When progressEmbedded is absent, no count label is emitted (honest: no phantom zero).
    expect(activities[0].progress.label).toBeNull()
    expect(activities[0].progress.labelKind).toBeNull()
  })

  test('progressScanned=undefined → value=null (indeterminate)', () => {
    const job = makeAiQueueJob({
      jobType: 'index-build',
      state: 'running',
      progressScanned: undefined,
      progressScanTarget: 10000,
    })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [job] }),
    })
    expect(activities[0].progress.value).toBeNull()
  })
})

// ── runtimeJobToActivity ─────────────────────────────────────────────────────

describe('runtimeJobToActivity — runtime job mapping', () => {
  test('content-fetch via pluginId → kind=content-fetch', () => {
    const job = makeRuntimeJob({
      jobType: 'process',
      pluginId: 'readable-content-refetch',
      state: 'running',
    })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities).toHaveLength(1)
    expect(activities[0].kind).toBe('content-fetch')
    expect(activities[0].taskNameKey).toBe('taskContentFetch')
    expect(activities[0].runtimeJobId).toBe(10)
  })

  test('deterministic-rebuild job type → kind=deterministic-rebuild', () => {
    const job = makeRuntimeJob({
      jobType: 'rebuild-derived-tables',
      state: 'running',
    })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].kind).toBe('deterministic-rebuild')
    expect(activities[0].taskNameKey).toBe('taskDeterministicRebuild')
  })

  test('re-embed runtime job type → kind=re-embed', () => {
    const job = makeRuntimeJob({ jobType: 're-embed-stale', state: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].kind).toBe('re-embed')
    expect(activities[0].taskNameKey).toBe('taskReEmbed')
  })

  test('generic runtime job with no matching type → kind=content-fetch (default)', () => {
    const job = makeRuntimeJob({ jobType: 'unknown-job', state: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].kind).toBe('content-fetch')
  })

  test('runtime job with progressPercent → value = percent/100 clamped', () => {
    const job = makeRuntimeJob({ progressPercent: 75, state: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].progress.value).toBeCloseTo(0.75)
  })

  test('runtime job with progressCurrent + progressTotal → value = current/total', () => {
    const job = makeRuntimeJob({
      progressCurrent: 30,
      progressTotal: 100,
      state: 'running',
    })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].progress.value).toBeCloseTo(0.3)
  })

  test('runtime job with progressTotal=0 → value=null (indeterminate)', () => {
    const job = makeRuntimeJob({
      progressCurrent: 30,
      progressTotal: 0,
      state: 'running',
    })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].progress.value).toBeNull()
  })

  test('runtime job with progressLabel → label forwarded', () => {
    const job = makeRuntimeJob({
      progressLabel: 'Processing…',
      state: 'running',
    })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].progress.label).toBe('Processing…')
  })

  test('runtime job with no progress fields → value=null, label=null', () => {
    const job = makeRuntimeJob({ state: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].progress.value).toBeNull()
    expect(activities[0].progress.label).toBeNull()
  })

  test('runtime job uses startedAt when set, createdAt as fallback', () => {
    const job = makeRuntimeJob({
      startedAt: '2026-04-07T10:01:00Z',
      createdAt: '2026-04-07T10:00:00Z',
    })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].timestamp).toBe('2026-04-07T10:01:00Z')
  })

  test('runtime job uses createdAt when startedAt is null', () => {
    const job = makeRuntimeJob({
      startedAt: null,
      createdAt: '2026-04-07T10:00:00Z',
    })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].timestamp).toBe('2026-04-07T10:00:00Z')
  })

  test('failed runtime job → state=failed, outcomeKey=outcomeFailed', () => {
    const job = makeRuntimeJob({
      jobType: 'content-fetch',
      state: 'failed',
      finishedAt: '2026-04-07T11:00:00Z',
    })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].state).toBe('failed')
    expect(activities[0].outcomeKey).toBe('outcomeFailed')
  })

  test('runtime job → resumability is always safe', () => {
    const job = makeRuntimeJob({ state: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      runtime: makeRuntime([job]),
    })
    expect(activities[0].resumability).toBe('safe')
  })
})

// ── shellTaskToActivity progress ─────────────────────────────────────────────

describe('shellTaskToActivity — progress fields', () => {
  test('ShellTask with processedRecords + totalRecords → progress.value calculated', () => {
    const task = makeShellTask({
      kind: 'import',
      state: 'running',
      processedRecords: 300,
      totalRecords: 1000,
    })
    const activities = buildActivities({
      ...emptyInput(),
      archiveTasks: [task],
    })
    expect(activities[0].progress.value).toBeCloseTo(0.3)
  })

  test('ShellTask with totalRecords=0 → progress.value=null (no division by zero)', () => {
    const task = makeShellTask({
      kind: 'import',
      state: 'running',
      processedRecords: 0,
      totalRecords: 0,
    })
    const activities = buildActivities({
      ...emptyInput(),
      archiveTasks: [task],
    })
    expect(activities[0].progress.value).toBeNull()
  })

  test('backup running ShellTask → taskNameKey=taskBackupRunning', () => {
    const task = makeShellTask({ kind: 'backup', state: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      archiveTasks: [task],
    })
    expect(activities[0].taskNameKey).toBe('taskBackupRunning')
    expect(activities[0].kind).toBe('backup')
  })

  test('import stale ShellTask → taskNameKey=taskImportStale', () => {
    const task = makeShellTask({ kind: 'import', state: 'stale' })
    const activities = buildActivities({
      ...emptyInput(),
      archiveTasks: [task],
    })
    expect(activities[0].taskNameKey).toBe('taskImportStale')
  })

  test('ShellTask with resultLink → resultLink forwarded', () => {
    const task = makeShellTask({
      kind: 'import',
      state: 'succeeded',
      resultLink: '/audit?run=99',
    })
    const activities = buildActivities({
      ...emptyInput(),
      archiveTasks: [task],
    })
    expect(activities[0].resultLink).toBe('/audit?run=99')
  })

  test('ShellTask without resultLink → resultLink=null', () => {
    const task = makeShellTask({ kind: 'import', state: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      archiveTasks: [task],
    })
    expect(activities[0].resultLink).toBeNull()
  })
})

// ── buildActivities sorting ──────────────────────────────────────────────────

describe('buildActivities — sorting', () => {
  test('sorts activities by most recent first across data sources', () => {
    const olderJob = makeAiQueueJob({
      id: 1,
      jobType: 'index-build',
      state: 'running',
      startedAt: '2026-04-07T09:00:00Z',
    })
    const newerTask = makeShellTask({
      kind: 'import',
      state: 'running',
      startedAt: '2026-04-07T10:00:00Z',
    })
    const activities = buildActivities({
      ...emptyInput(),
      aiQueue: makeAiQueueStatus({ recentJobs: [olderJob] }),
      archiveTasks: [newerTask],
    })
    // newerTask should come first
    expect(activities[0].kind).toBe('import')
    expect(activities[1].kind).toBe('index-build')
  })

  test('recentRuns with non-backup/import runType are excluded', () => {
    const run = makeBackupRunOverview({ runType: 'other', status: 'running' })
    const activities = buildActivities({
      ...emptyInput(),
      recentRuns: [run],
    })
    expect(activities).toHaveLength(0)
  })

  test('recentRuns with null runType are excluded via ?? fallback', () => {
    // runType: null exercises the `run.runType ?? ''` null/undefined branch
    const run = makeBackupRunOverview({
      runType: null as unknown as string,
      status: 'running',
    })
    const activities = buildActivities({
      ...emptyInput(),
      recentRuns: [run],
    })
    expect(activities).toHaveLength(0)
  })

  test('shellTaskToActivity maps task.error to activity.cause', () => {
    const task = makeShellTask({ state: 'failed', error: 'disk full' })
    const activities = buildActivities({
      ...emptyInput(),
      archiveTasks: [task],
    })
    expect(activities).toHaveLength(1)
    expect(activities[0].cause).toBe('disk full')
  })

  test('shellTaskToActivity sets cause to undefined when task.error is null', () => {
    const task = makeShellTask({ state: 'failed', error: null })
    const activities = buildActivities({
      ...emptyInput(),
      archiveTasks: [task],
    })
    expect(activities).toHaveLength(1)
    expect(activities[0].cause).toBeUndefined()
  })
})
