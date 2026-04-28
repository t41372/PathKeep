/**
 * @file task-progress.tsx
 * @description Shared progress bar, task card, and bounded console renderer for PathKeep long-running work.
 * @module components/progress
 *
 * ## Responsibilities
 * - Render one consistent progress grammar for import, backup, busy overlays, Jobs, and sidebar-adjacent task surfaces.
 * - Keep console-style task logs bounded, timestamped, severity-marked, and auto-scrolled.
 * - Support compact and full task variants without letting route surfaces invent separate progress bars.
 *
 * ## Not responsible for
 * - Owning task state, persistence, or backend progress subscriptions.
 * - Translating backend diagnostic codes into product copy.
 * - Rendering queue-specific retry or cancel controls.
 *
 * ## Dependencies
 * - Depends on shell task types and the shared date formatter.
 * - Consumed by `BusyOverlay`, Import, Jobs, and sidebar chrome.
 *
 * ## Performance notes
 * - Uses only the already-bounded task log list supplied by the shell store and keeps scrolling work inside one ref.
 */

import { useEffect, useMemo, useRef, type ReactNode } from 'react'
import type { ShellTask, ShellTaskLogEntry } from '../../app/shell-tasks'
import { formatRelativeTime } from '../../lib/format'
import type { ResolvedLanguage } from '../../lib/i18n'

export interface ProgressMeterProps {
  label?: string | null
  value?: number | null
  compact?: boolean
}

export interface TaskConsoleProps {
  entries: readonly ShellTaskLogEntry[]
  emptyLabel: string
  language: ResolvedLanguage
  compact?: boolean
}

export interface TaskProgressCardProps {
  task: ShellTask
  language: ResolvedLanguage
  labels: {
    started: string
    updated: string
    records: string
    console: string
    noLogs: string
  }
  compact?: boolean
  actions?: ReactNode
}

/**
 * Renders the shared determinate or indeterminate progress track.
 *
 * @param label Optional inline progress label shown beside the percent.
 * @param value Nullable percent; null means active but indeterminate.
 * @param compact Reduces vertical spacing for sidebar and busy-overlay usage.
 */
export function ProgressMeter({ label, value, compact }: ProgressMeterProps) {
  const normalizedProgress = normalizeProgressValue(value)
  const showProgress = Boolean(label) || normalizedProgress !== null

  if (!showProgress) {
    return null
  }

  return (
    <div
      className={`task-progress-meter ${
        compact ? 'task-progress-meter--compact' : ''
      }`}
    >
      <div className="task-progress-meter__meta">
        {label ? <span>{label}</span> : <span />}
        {normalizedProgress !== null ? (
          <span>{Math.round(normalizedProgress)}%</span>
        ) : null}
      </div>
      <div
        aria-hidden="true"
        className={`task-progress-meter__track ${
          normalizedProgress === null
            ? 'task-progress-meter__track--indeterminate'
            : ''
        }`}
      >
        <span
          className={`task-progress-meter__fill ${
            normalizedProgress === null
              ? 'task-progress-meter__fill--indeterminate'
              : ''
          }`}
          style={
            normalizedProgress === null
              ? undefined
              : { width: `${Math.max(4, normalizedProgress)}%` }
          }
        />
      </div>
    </div>
  )
}

/**
 * Renders recent task log entries as a bounded, timestamped console.
 *
 * @param entries Already-bounded task logs from the shell store.
 * @param emptyLabel Fallback when no entries have arrived.
 * @param language Locale used for timestamp formatting.
 * @param compact Reduces visible height for embedded use.
 */
export function TaskConsole({
  compact,
  emptyLabel,
  entries,
  language,
}: TaskConsoleProps) {
  const consoleRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    const node = consoleRef.current
    /* v8 ignore next -- React attaches this node after mount; the guard protects SSR/test teardown. */
    if (!node) {
      return
    }

    node.scrollTop = node.scrollHeight
  }, [entries])

  const formatter = useMemo(
    () =>
      new Intl.DateTimeFormat(language, {
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
      }),
    [language],
  )

  return (
    <div
      ref={consoleRef}
      className={`task-console ${compact ? 'task-console--compact' : ''}`}
      role="log"
      aria-live="polite"
    >
      {entries.length > 0 ? (
        entries.map((entry) => (
          <div
            key={entry.id}
            className="task-console__line"
            data-level={entry.level}
          >
            <span className="task-console__time">
              {formatter.format(new Date(entry.timestamp))}
            </span>
            <span className="task-console__level">{entry.level}</span>
            <span className="task-console__message">
              {entry.sourceLabel ? `[${entry.sourceLabel}] ` : null}
              {entry.message}
            </span>
          </div>
        ))
      ) : (
        <div className="task-console__line" data-level="info">
          <span className="task-console__message">{emptyLabel}</span>
        </div>
      )}
    </div>
  )
}

/**
 * Renders a full archive task card for Import and Jobs.
 *
 * @param task Shell task to display.
 * @param language Locale used for relative time and console timestamps.
 * @param labels Localized static labels for metadata.
 * @param compact Chooses a denser card layout.
 * @param actions Optional route-specific actions such as Jobs or result links.
 */
export function TaskProgressCard({
  actions,
  compact,
  labels,
  language,
  task,
}: TaskProgressCardProps) {
  const recordSummary =
    task.processedRecords !== null &&
    task.processedRecords !== undefined &&
    task.totalRecords !== null &&
    task.totalRecords !== undefined
      ? `${task.processedRecords.toLocaleString(language)} / ${task.totalRecords.toLocaleString(language)} ${labels.records}`
      : task.processedRecords !== null && task.processedRecords !== undefined
        ? `${task.processedRecords.toLocaleString(language)} ${labels.records}`
        : task.progressLabel

  return (
    <article
      className={`task-progress-card ${
        compact ? 'task-progress-card--compact' : ''
      }`}
      data-state={task.state}
      data-kind={task.kind}
    >
      <div className="task-progress-card__header">
        <div>
          <span className="task-progress-card__eyebrow">
            {task.kind} · {task.state}
          </span>
          <h3>{task.title}</h3>
        </div>
        {actions ? (
          <div className="task-progress-card__actions">{actions}</div>
        ) : null}
      </div>
      <p className="task-progress-card__detail">{task.detail}</p>
      <div className="task-progress-card__meta">
        <span>
          {labels.started}: {formatRelativeTime(task.startedAt, language)}
        </span>
        <span>
          {labels.updated}: {formatRelativeTime(task.updatedAt, language)}
        </span>
        {recordSummary ? <span>{recordSummary}</span> : null}
      </div>
      <ProgressMeter
        compact={compact}
        label={recordSummary ?? task.progressLabel}
        value={task.progressValue}
      />
      <div className="task-progress-card__console-label">{labels.console}</div>
      <TaskConsole
        compact={compact}
        emptyLabel={labels.noLogs}
        entries={task.logEntries}
        language={language}
      />
    </article>
  )
}

function normalizeProgressValue(value: number | null | undefined) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return null
  }

  return Math.max(0, Math.min(100, value))
}
