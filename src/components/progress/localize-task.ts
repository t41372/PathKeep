/**
 * @file localize-task.ts
 * @description Localizes shell task narration and console entries from backend progress codes before rendering.
 * @module components/progress
 *
 * ## Responsibilities
 * - Map backend progress `phase` / `logEvents.code` values onto `jobs.*`
 *   catalog copy, per the desktop-command-surface contract ("the UI consumes
 *   `level` / `code` / counters — not transport prose").
 * - Fall back to the backend's raw English message ONLY for codes the catalog
 *   does not know, so new backend codes degrade visibly instead of crashing.
 * - Localize the task kind/state vocabulary shown on task cards.
 *
 * ## Not responsible for
 * - Owning task state (`app/shell-tasks.ts`) or rendering (`task-progress.tsx`).
 * - Localizing shell-authored entries — those arrive already localized and are
 *   marked `origin: 'shell'`.
 */

import type {
  ShellTask,
  ShellTaskKind,
  ShellTaskLogEntry,
  ShellTaskState,
} from '../../app/shell-tasks'

type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/** Backend progress phases with shipped catalog copy, per task kind. */
const LOCALIZED_PHASES: Record<string, readonly string[]> = {
  backup: ['prepare', 'stage-profile', 'ingest-profile', 'finalize'],
  import: [
    'prepare',
    'import-file',
    'finalize',
    'complete',
    'unexpected-phase',
  ],
}

/** Backend structured log codes with shipped catalog copy. */
const LOCALIZED_LOG_CODES = new Set([
  'backup.prepare',
  'backup.stage-profile',
  'backup.stage-profile.fallback',
  'backup.stage-profile.skip',
  'backup.ingest-profile',
  'backup.ingest-profile.records',
  'backup.finalize',
  'import.prepare',
  'import.import-file',
  'import.finalize',
  'import.complete',
  'import.unexpected-phase',
])

/**
 * Returns the localized task narration line: phase-keyed catalog copy for
 * backend-authored details, the already-localized text otherwise.
 */
export function localizedTaskDetail(task: ShellTask, t: Translator): string {
  if (task.detailOrigin !== 'backend') {
    return task.detail
  }
  const phases = LOCALIZED_PHASES[task.kind]
  if (task.phase && phases?.includes(task.phase)) {
    return t(`jobs.phase.${task.kind}.${task.phase}`)
  }
  return task.detail
}

/**
 * Returns the localized console line for one task log entry: code-keyed
 * catalog copy for backend-authored entries, the raw message otherwise.
 */
export function localizedTaskLogMessage(
  entry: ShellTaskLogEntry,
  t: Translator,
): string {
  if (entry.origin !== 'backend' || !LOCALIZED_LOG_CODES.has(entry.code)) {
    return entry.message
  }
  if (entry.code === 'backup.ingest-profile.records') {
    const records = entry.current ?? 0
    // Plural pairs, not "record(s)": this line is read, not parsed.
    return t(
      records === 1
        ? 'jobs.log.backup.ingest-profile.recordsOne'
        : 'jobs.log.backup.ingest-profile.recordsMany',
      { current: records.toLocaleString() },
    )
  }
  return t(`jobs.log.${entry.code}`)
}

/**
 * Returns a task copy whose `detail` and console messages are localized,
 * ready for the translation-free progress renderers.
 */
export function localizeShellTaskForDisplay(
  task: ShellTask,
  t: Translator,
): ShellTask {
  return {
    ...task,
    detail: localizedTaskDetail(task, t),
    logEntries: task.logEntries.map((entry) => ({
      ...entry,
      message: localizedTaskLogMessage(entry, t),
    })),
  }
}

/** Localizes the task-kind vocabulary shown on task cards. */
export function taskKindLabel(kind: ShellTaskKind, t: Translator): string {
  return t(`jobs.taskKind.${kind}`)
}

/** Localizes the task-state vocabulary shown on task cards. */
export function taskStateLabel(state: ShellTaskState, t: Translator): string {
  return t(`jobs.taskState.${state}`)
}
