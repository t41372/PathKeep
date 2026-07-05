/**
 * @file activity-header.tsx
 * @description Activity page header strip with heading, queue summary line, and pause/resume toggle.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Render the page h1 (labeled by aria-labelledby="activity-page-heading").
 * - Display the computed queue summary string.
 * - Show pause/resume queue toggle when relevant.
 *
 * ## Not responsible for
 * - Computing the summary string or queue state (caller's responsibility).
 * - Fetching or polling data.
 */

import { Button } from '@/components/ui/button'
import { useI18n } from '../../lib/i18n'

interface ActivityHeaderProps {
  summary: string
  queuePaused: boolean
  showToggle: boolean
  onPauseChange: (paused: boolean) => void
  action: string | null
}

/**
 * Renders the Activity page heading strip.
 *
 * The h1 carries id="activity-page-heading" so the page's main element can use
 * aria-labelledby to satisfy the landmark labeling contract.
 */
export function ActivityHeader({
  summary,
  queuePaused,
  showToggle,
  onPauseChange,
  action,
}: ActivityHeaderProps) {
  const { ns } = useI18n()
  const jobsT = ns('jobs')

  return (
    <div className="activity-header">
      <div className="activity-header__main">
        <h1 id="activity-page-heading" className="activity-header__title">
          {jobsT('activityPageTitle')}
        </h1>
        <p className="activity-header__summary">{summary}</p>
      </div>
      {showToggle && (
        <div className="activity-header__actions">
          <Button
            variant="outline"
            type="button"
            onClick={() => onPauseChange(!queuePaused)}
            disabled={Boolean(action)}
          >
            {queuePaused ? jobsT('actionResume') : jobsT('actionPause')}
          </Button>
        </div>
      )}
    </div>
  )
}
