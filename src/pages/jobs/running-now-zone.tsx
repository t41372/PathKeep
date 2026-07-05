/**
 * @file running-now-zone.tsx
 * @description Renders currently running or queued activities including model download progress.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Display a region of rows for running and queued activities.
 * - Show progress bars with correct aria roles (determinate vs indeterminate).
 * - Show interruption resumability badges.
 * - Show model download progress when active.
 *
 * ## Not responsible for
 * - Computing which activities are running (see activity-adapter.ts).
 * - Starting or cancelling the model download.
 */

import { Button } from '@/components/ui/button'
import type { ModelDownloadProgress } from '../../lib/ipc/model-download'
import type { ResolvedLanguage } from '../../lib/i18n'
import type { Activity } from './activity-types'

interface RunningNowZoneProps {
  activities: Activity[]
  modelDownload: ModelDownloadProgress
  showModelDownload: boolean
  onPauseChange: (paused: boolean) => void
  onCancel: (jobId: number) => void
  onCancelRuntime: (jobId: number) => void
  action: string | null
  jobsT: (key: string, vars?: Record<string, string | number>) => string
  language: ResolvedLanguage
}

/**
 * Renders the "Running now" region for active and queued activities.
 *
 * Looping animations are gated on prefers-reduced-motion: no-preference so
 * users who request reduced motion do not see pulsing dots.
 */
export function RunningNowZone({
  activities,
  modelDownload,
  showModelDownload,
  onCancel,
  onCancelRuntime,
  action,
  jobsT,
  language,
}: RunningNowZoneProps) {
  const hasContent =
    activities.length > 0 ||
    (showModelDownload && modelDownload.phase === 'downloading')
  /* v8 ignore next 1 -- parent always renders this zone when content exists */
  if (!hasContent) return null

  return (
    <section
      className="activity-zone activity-zone--running"
      role="region"
      aria-label={jobsT('runningNowTitle')}
    >
      <h2 className="activity-zone__heading">{jobsT('runningNowTitle')}</h2>
      <div className="activity-zone__list">
        {activities.map((activity) => (
          <ActivityRunningRow
            key={activity.id}
            activity={activity}
            onCancel={onCancel}
            onCancelRuntime={onCancelRuntime}
            action={action}
            jobsT={jobsT}
            language={language}
          />
        ))}

        {showModelDownload && modelDownload.phase === 'downloading' && (
          <ModelDownloadRow modelDownload={modelDownload} jobsT={jobsT} />
        )}
      </div>
    </section>
  )
}

interface ActivityRunningRowProps {
  activity: Activity
  onCancel: (jobId: number) => void
  onCancelRuntime: (jobId: number) => void
  action: string | null
  jobsT: (key: string, vars?: Record<string, string | number>) => string
  language: ResolvedLanguage
}

function ActivityRunningRow({
  activity,
  onCancel,
  onCancelRuntime,
  action,
  jobsT,
  language,
}: ActivityRunningRowProps) {
  const { progress, resumability } = activity
  const isDeterminate = progress.value != null

  // A running row carries either 'safe' (index/content/re-embed/deterministic — durable cursor) or
  // 'restart-whole' (a running import/backup — data safe, but the task restarts on quit). 'per-file'
  // (the model download has its own dedicated row) and 'cannot-resume' (stale → Needs attention)
  // never reach a running ActivityRunningRow, so anything-not-'safe' is the restart-whole signal.
  const isResumeSafe = resumability === 'safe'
  const resumabilityKey = isResumeSafe
    ? 'badgeSafeToClose'
    : 'badgeRestartWhole'
  const badgeToneClass = isResumeSafe
    ? 'activity-row__badge--safe'
    : 'activity-row__badge--warning'

  return (
    <div className="activity-row">
      <div className="activity-row__lead">
        <span
          className="activity-dot activity-dot--running"
          aria-hidden="true"
        />
      </div>
      <div className="activity-row__body">
        <span className="activity-row__name">
          {jobsT(activity.taskNameKey)}
        </span>

        {isDeterminate ? (
          <div
            className="activity-progress"
            role="progressbar"
            aria-valuenow={progress.value!}
            aria-valuemin={0}
            aria-valuemax={1}
          >
            <div
              className="activity-progress__bar"
              style={{ width: `${(progress.value! * 100).toFixed(1)}%` }}
            />
          </div>
        ) : (
          <div
            className="activity-progress activity-progress--indeterminate"
            aria-busy="true"
          >
            <div className="activity-progress__bar activity-progress__bar--indeterminate" />
          </div>
        )}

        {progress.labelKind === 'embedded' && progress.label !== null && (
          <span className="activity-row__count">
            {jobsT('progressEmbeddedLabel', { count: progress.label })}
          </span>
        )}
        {progress.labelKind === 'embeddedOfTotal' &&
          progress.processedCount != null &&
          progress.totalCount != null && (
            <span className="activity-row__count">
              {jobsT('progressEmbeddedOfTotalLabel', {
                processed: progress.processedCount.toLocaleString(language),
                total: progress.totalCount.toLocaleString(language),
              })}
            </span>
          )}
        {progress.labelKind === 'verbatim' && progress.label !== null && (
          <span className="activity-row__count">{progress.label}</span>
        )}
        {progress.labelKind === 'records' &&
          progress.processedCount != null &&
          progress.totalCount != null && (
            <span className="activity-row__count">
              {jobsT('progressRecordsLabel', {
                processed: progress.processedCount.toLocaleString(language),
                total: progress.totalCount.toLocaleString(language),
              })}
            </span>
          )}

        <span className={`activity-row__badge ${badgeToneClass}`} role="note">
          {jobsT(resumabilityKey)}
        </span>
      </div>

      {activity.aiJobId != null && (
        <div className="activity-row__actions">
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={() => onCancel(activity.aiJobId!)}
            disabled={Boolean(action)}
            aria-label={`${jobsT('actionCancel')} ${jobsT(activity.taskNameKey)}`}
          >
            {jobsT('actionCancel')}
          </Button>
        </div>
      )}

      {activity.runtimeJobId != null && activity.cancellable && (
        <div className="activity-row__actions">
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={() => onCancelRuntime(activity.runtimeJobId!)}
            disabled={Boolean(action)}
            aria-label={`${jobsT('actionCancel')} ${jobsT(activity.taskNameKey)}`}
          >
            {jobsT('actionCancel')}
          </Button>
        </div>
      )}
    </div>
  )
}

interface ModelDownloadRowProps {
  modelDownload: ModelDownloadProgress
  jobsT: (key: string, vars?: Record<string, string | number>) => string
}

function ModelDownloadRow({ modelDownload, jobsT }: ModelDownloadRowProps) {
  const { downloadedBytes, totalBytes } = modelDownload
  const isDeterminate = totalBytes > 0
  const progressValue = isDeterminate
    ? Math.min(downloadedBytes / totalBytes, 1)
    : null

  return (
    <div className="activity-row">
      <div className="activity-row__lead">
        <span
          className="activity-dot activity-dot--running"
          aria-hidden="true"
        />
      </div>
      <div className="activity-row__body">
        <span className="activity-row__name">{jobsT('taskModelDownload')}</span>

        {isDeterminate && progressValue != null ? (
          <div
            className="activity-progress"
            role="progressbar"
            aria-valuenow={progressValue}
            aria-valuemin={0}
            aria-valuemax={1}
          >
            <div
              className="activity-progress__bar"
              style={{ width: `${(progressValue * 100).toFixed(1)}%` }}
            />
          </div>
        ) : (
          <div
            className="activity-progress activity-progress--indeterminate"
            aria-busy="true"
          >
            <div className="activity-progress__bar activity-progress__bar--indeterminate" />
          </div>
        )}

        {isDeterminate && (
          <span className="activity-row__count">
            {jobsT('progressDownloadLabel', {
              downloaded: formatBytes(downloadedBytes),
              total: formatBytes(totalBytes),
            })}
          </span>
        )}

        <span
          className="activity-row__badge activity-row__badge--warning"
          role="note"
        >
          {jobsT('badgePerFileResume')}
        </span>
      </div>
    </div>
  )
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}
