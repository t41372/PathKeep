/**
 * @file shell-ambient-tasks.ts
 * @description Pure selector that folds the shell archive-task store and the runtime AI/enrichment
 *   queue into one ambient bottom-bar model — the single "something is running" indicator the shell
 *   surfaces on every route.
 * @module app/shell-ambient-tasks
 *
 * ## Responsibilities
 * - Combine the two independent long-task sources (the route-independent `archiveTasks` store and
 *   the runtime queue) into one ordered list so ANY running task is visible after navigation, not
 *   just the backup that used to own the bottom strip.
 * - Keep the archive/runtime → ambient projection side-effect free and route-independent so the
 *   shell can memoize it inside its render path without adding polling or main-thread work.
 *
 * ## Not responsible for
 * - Fetching, polling, or subscribing to any progress source (the shell-data provider owns that).
 * - Localizing labels beyond delegating runtime task-name keys to the caller-supplied translator
 *   (archive tasks already carry a localized `title`).
 * - Rendering the bar (see components/primitives/ambient-task-bar.tsx).
 */

import type { ShellTask } from './shell-tasks'
import type { BusyOverlayState, ShellRuntimeStatus } from './shell-data-context'
import {
  buildActivities,
  buildRunningNow,
} from '../pages/jobs/activity-adapter'

/**
 * One running/queued task, flattened to exactly what the ambient bar renders. `progressValue` is a
 * clamped percent (0–100) or `null` for indeterminate; `progressLabel` is an already-localized
 * detail string or `null`.
 */
export interface AmbientTask {
  id: string
  label: string
  progressValue: number | null
  progressLabel: string | null
}

/**
 * The ambient bar's whole input: how many tasks are active, the primary (first) one it shows, and
 * the full ordered list for callers that want the count-driven summary.
 */
export interface AmbientTasksModel {
  count: number
  primary: AmbientTask | null
  tasks: AmbientTask[]
}

function clamp0to100(value: number | null | undefined): number | null {
  if (value == null) {
    return null
  }
  return Math.max(0, Math.min(100, value))
}

/**
 * Projects the live shell + runtime state into the ambient bar model.
 *
 * Archive tasks come first (they already sort updatedAt-desc in the store, so their order is
 * preserved), followed by running/queued runtime jobs mapped through the shared Activity adapter so
 * the bar and the Activity page agree on which jobs count as "running now".
 */
export function selectAmbientTasks(input: {
  archiveTasks: readonly ShellTask[]
  runtimeStatus?: ShellRuntimeStatus | null
  runtimeTaskLabel: (taskNameKey: string) => string
}): AmbientTasksModel {
  const { archiveTasks, runtimeStatus, runtimeTaskLabel } = input

  const archive: AmbientTask[] = archiveTasks
    .filter((task) => task.state === 'running' || task.state === 'queued')
    .map((task) => ({
      id: task.id,
      label: task.title,
      progressValue: clamp0to100(task.progressValue),
      progressLabel: task.progressLabel ?? null,
    }))

  const runtimeActivities = buildRunningNow(
    buildActivities({
      aiQueue: runtimeStatus?.aiQueue ?? null,
      runtime: runtimeStatus?.intelligence ?? null,
      archiveTasks: [],
      recentRuns: [],
    }),
  )
  const runtime: AmbientTask[] = runtimeActivities.map((activity) => ({
    id: activity.id,
    label: runtimeTaskLabel(activity.taskNameKey),
    progressValue:
      activity.progress.value != null
        ? Math.round(activity.progress.value * 100)
        : null,
    progressLabel: null,
  }))

  const tasks = [...archive, ...runtime]

  return {
    count: tasks.length,
    primary: tasks[0] ?? null,
    tasks,
  }
}

/**
 * Defensive fallback: projects a `background: true` busy overlay directly into a one-task ambient
 * model.
 *
 * Today the only `background: true` producer (manual backup) also registers a running archive task,
 * so `selectAmbientTasks` already drives the bar. But a future `background: true` overlay that
 * forgets to register a task would otherwise route to nothing — no blocking overlay AND no ambient
 * bar — making the work invisible. This keeps the bar honest by surfacing the overlay's own payload
 * so background work can never silently disappear.
 */
export function ambientModelFromBusyOverlay(
  overlay: BusyOverlayState,
): AmbientTasksModel {
  const primary: AmbientTask = {
    id: 'busy-overlay',
    label: overlay.label,
    progressValue: clamp0to100(overlay.progressValue),
    progressLabel: overlay.progressLabel ?? null,
  }
  return { count: 1, primary, tasks: [primary] }
}
