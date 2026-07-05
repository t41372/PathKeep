/**
 * @file needs-attention-zone.tsx
 * @description Renders failed and stale activities that require user action.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Display a region of cards for failed and stale activities.
 * - Provide one primary action per card based on kind and state.
 *
 * ## Not responsible for
 * - Fetching or polling data.
 * - Computing which activities need attention (see activity-adapter.ts).
 */

import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { formatRelativeTime } from '../../lib/format'
import type { ResolvedLanguage } from '../../lib/i18n'
import type { Activity } from './activity-types'

interface NeedsAttentionZoneProps {
  activities: Activity[]
  onRetry: (jobId: number) => void
  onRetryRuntimeJob: (jobId: number) => void
  onRetryBackup: () => void
  action: string | null
  jobsT: (key: string, vars?: Record<string, string | number>) => string
  language: ResolvedLanguage
}

/**
 * Renders the "Needs attention" region for failed and stale activities.
 *
 * Each card has a 3px left rail colored by tone and a single primary action.
 */
export function NeedsAttentionZone({
  activities,
  onRetry,
  onRetryRuntimeJob,
  onRetryBackup,
  action,
  jobsT,
  language,
}: NeedsAttentionZoneProps) {
  /* v8 ignore next 1 -- parent always renders this zone with non-empty activities */
  if (activities.length === 0) return null

  return (
    <section
      className="activity-zone activity-zone--attention"
      role="region"
      aria-label={jobsT('needsAttentionTitle')}
    >
      <h2 className="activity-zone__heading">{jobsT('needsAttentionTitle')}</h2>
      <div className="activity-zone__list">
        {activities.map((activity) => {
          const isFailed = activity.state === 'failed'
          const railClass = isFailed
            ? 'activity-card--danger-rail'
            : 'activity-card--warning-rail'

          return (
            <div key={activity.id} className={`activity-card ${railClass}`}>
              <div className="activity-card__body">
                <span className="activity-card__name">
                  {jobsT(activity.taskNameKey)}
                </span>
                {activity.cause && (
                  <span className="activity-card__cause">{activity.cause}</span>
                )}
                <span className="activity-card__time">
                  {formatRelativeTime(activity.timestamp, language)}
                </span>
              </div>
              <div className="activity-card__actions">
                {renderPrimaryAction(
                  activity,
                  jobsT,
                  onRetry,
                  onRetryRuntimeJob,
                  onRetryBackup,
                  action,
                )}
              </div>
            </div>
          )
        })}
      </div>
    </section>
  )
}

function renderPrimaryAction(
  activity: Activity,
  jobsT: (key: string, vars?: Record<string, string | number>) => string,
  onRetry: (jobId: number) => void,
  onRetryRuntimeJob: (jobId: number) => void,
  onRetryBackup: () => void,
  action: string | null,
) {
  const { kind, state } = activity

  if (
    kind === 'index-build' &&
    state === 'failed' &&
    activity.aiJobId != null
  ) {
    return (
      <Button
        variant="outline"
        type="button"
        onClick={() => onRetry(activity.aiJobId!)}
        disabled={Boolean(action)}
        aria-label={`${jobsT('actionRetry')} ${jobsT(activity.taskNameKey)}`}
      >
        {jobsT('actionRetry')}
      </Button>
    )
  }

  if (
    (kind === 'content-fetch' ||
      kind === 're-embed' ||
      kind === 'deterministic-rebuild') &&
    state === 'failed' &&
    activity.runtimeJobId != null
  ) {
    return (
      <Button
        variant="outline"
        type="button"
        onClick={() => onRetryRuntimeJob(activity.runtimeJobId!)}
        disabled={Boolean(action)}
        aria-label={`${jobsT('actionRetry')} ${jobsT(activity.taskNameKey)}`}
      >
        {jobsT('actionRetry')}
      </Button>
    )
  }

  if (kind === 'import' && (state === 'stale' || state === 'failed')) {
    return (
      <Button variant="outline" asChild>
        <Link
          to="/import"
          aria-label={`${jobsT('actionOpenImport')} ${jobsT(activity.taskNameKey)}`}
        >
          {jobsT('actionOpenImport')}
        </Link>
      </Button>
    )
  }

  if (kind === 'backup' && (state === 'stale' || state === 'failed')) {
    return (
      <Button
        variant="outline"
        type="button"
        onClick={onRetryBackup}
        disabled={Boolean(action)}
        aria-label={`${jobsT('actionRetryBackup')} ${jobsT(activity.taskNameKey)}`}
      >
        {jobsT('actionRetryBackup')}
      </Button>
    )
  }

  return null
}
