/**
 * @file zero-state.tsx
 * @description Renders the dashboard zero-state layout after archive initialization or first-run setup has not yet produced useful history.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Compose the dashboard's zero-state stats, archive boundary, and setup checklist.
 * - Keep zero-state navigation and copy aligned with the main dashboard shell.
 *
 * ## Not responsible for
 * - Fetching dashboard data or choosing whether zero-state should be shown
 * - Rendering the fully populated dashboard route
 *
 * ## Dependencies
 * - Depends on split dashboard panel owners plus shared dashboard helper view models.
 *
 * ## Performance notes
 * - Pure composition only; avoid local effects so the zero-state branch stays cheap on cold boot.
 */

import { Link } from 'react-router-dom'
import { StatusCallout } from '../../components/primitives/status-callout'
import type { BrowserProfile, DashboardSnapshot } from '../../lib/types'
import type { DashboardStatItem } from './helpers'
import {
  DashboardArchiveBoundaryPanel,
  DashboardStatsRow,
  DashboardZeroStateChecklistPanel,
} from './panels'

type Translate = (key: string, vars?: Record<string, string | number>) => string

interface DashboardZeroStateProps {
  commonT: Translate
  dashboard: DashboardSnapshot
  selectedProfiles: BrowserProfile[]
  snapshotInitialized: boolean
  stats: DashboardStatItem[]
  t: Translate
}

/**
 * Keeps the dashboard zero-state branch out of the route shell so the main render path stays readable.
 */
export function DashboardZeroState({
  commonT,
  dashboard,
  selectedProfiles,
  snapshotInitialized,
  stats,
  t,
}: DashboardZeroStateProps) {
  return (
    <>
      <DashboardStatsRow stats={stats} />
      <StatusCallout
        tone="info"
        eyebrow={t('dashboard.zeroStateEyebrow')}
        title={t('dashboard.zeroStateTitle')}
        body={dashboard.nextAction ?? t('dashboard.zeroStateBody')}
        actions={
          <Link className="btn-primary" to="/onboarding">
            {t('dashboard.openOnboardingFlow')}
          </Link>
        }
      />
      <div className="dashboard-grid">
        <div className="dashboard-left">
          <DashboardArchiveBoundaryPanel
            commonT={commonT}
            selectedProfiles={selectedProfiles}
            t={t}
          />
        </div>
        <div className="dashboard-right">
          <DashboardZeroStateChecklistPanel
            dashboard={dashboard}
            snapshotInitialized={snapshotInitialized}
            t={t}
          />
        </div>
      </div>
    </>
  )
}
