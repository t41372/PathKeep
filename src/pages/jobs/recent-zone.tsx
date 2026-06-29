/**
 * @file recent-zone.tsx
 * @description Collapsible list of recently finished activities.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Render a collapsible list of up to 15 recent terminal activities.
 * - Show outcome pills and relative timestamps.
 * - Provide "View result →" links for activities that have a resultLink.
 *
 * ## Not responsible for
 * - Computing which activities are recent (see activity-adapter.ts).
 * - Fetching or polling data.
 */

import { useState } from 'react'
import { Link } from 'react-router-dom'
import { formatRelativeTime } from '../../lib/format'
import type { ResolvedLanguage } from '../../lib/i18n'
import type { Activity } from './activity-types'

interface RecentZoneProps {
  activities: Activity[]
  jobsT: (key: string, vars?: Record<string, string | number>) => string
  language: ResolvedLanguage
}

/**
 * Renders the "Recent activity" collapsible zone.
 *
 * Collapsed by default. The toggle shows item count when collapsed.
 */
export function RecentZone({ activities, jobsT, language }: RecentZoneProps) {
  const [expanded, setExpanded] = useState(false)

  if (activities.length === 0) return null

  return (
    <section
      className="activity-zone activity-zone--recent"
      role="region"
      aria-label={jobsT('recentTitle')}
    >
      <div className="activity-zone__header">
        <button
          className="activity-recent-toggle"
          type="button"
          onClick={() => setExpanded((prev) => !prev)}
          aria-expanded={expanded}
        >
          {expanded
            ? jobsT('hideRecentToggle')
            : jobsT('showRecentToggle', { count: activities.length })}
        </button>
      </div>

      {expanded && (
        <div className="activity-zone__list">
          {activities.slice(0, 15).map((activity) => (
            <RecentActivityRow
              key={activity.id}
              activity={activity}
              jobsT={jobsT}
              language={language}
            />
          ))}
        </div>
      )}
    </section>
  )
}

interface RecentActivityRowProps {
  activity: Activity
  jobsT: (key: string, vars?: Record<string, string | number>) => string
  language: ResolvedLanguage
}

function RecentActivityRow({
  activity,
  jobsT,
  language,
}: RecentActivityRowProps) {
  const outcomeClass =
    activity.state === 'succeeded'
      ? 'outcome-pill--success'
      : activity.state === 'failed'
        ? 'outcome-pill--failed'
        : activity.state === 'cancelled'
          ? 'outcome-pill--cancelled'
          : 'outcome-pill--interrupted'

  const dotClass =
    activity.state === 'succeeded'
      ? 'state-dot--success'
      : activity.state === 'failed'
        ? 'state-dot--failed'
        : 'state-dot--neutral'

  /* v8 ignore next 1 -- outcomeKey is always set for terminal-state activities in buildRecent */
  const outcomeLabel = activity.outcomeKey ?? 'outcomeSuccess'

  return (
    <div className="recent-row">
      <span className={`state-dot ${dotClass}`} aria-hidden="true" />
      <span className="recent-row__name">{jobsT(activity.taskNameKey)}</span>
      <span className={`outcome-pill ${outcomeClass}`}>
        {jobsT(outcomeLabel)}
      </span>
      <span className="recent-row__time">
        {formatRelativeTime(activity.timestamp, language)}
      </span>
      {activity.resultLink && (
        <Link className="recent-row__result-link" to={activity.resultLink}>
          View result →
        </Link>
      )}
    </div>
  )
}
