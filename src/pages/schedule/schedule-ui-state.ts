/**
 * @file schedule-ui-state.ts
 * @description Derives the state-machine surface for Scheduled Backup Settings.
 * @module pages/schedule
 *
 * ## Responsibilities
 * - Map backend schedule status into the five product UI states.
 * - Keep severity rules testable outside the route renderer.
 *
 * ## Not responsible for
 * - Fetching schedule data or running native scheduler actions.
 * - Translating labels, issues, or verification rows.
 *
 * ## Dependencies
 * - Schedule DTOs from the shared frontend type contract.
 *
 * ## Performance notes
 * - Pure constant-time logic over tiny issue/check arrays.
 */

import type { ScheduleIssue, ScheduleStatus } from '../../lib/types'

export type ScheduleUiState =
  | 'CHECKING'
  | 'NOT_INSTALLED'
  | 'INSTALLED_OK'
  | 'INSTALLED_WARN'
  | 'INSTALLED_ERROR'

/**
 * Maps the backend install/read-model state into the route's product-level state machine.
 *
 * @param loading true while preview/status detection is in flight.
 * @param status current backend status snapshot, if detection succeeded.
 * @param hasNeverRun true when no successful backup exists yet.
 * @returns the only top-level state the Schedule route should render.
 */
export function deriveScheduleUiState({
  hasNeverRun,
  loading,
  status,
}: {
  hasNeverRun: boolean
  loading: boolean
  status: ScheduleStatus | null
}): ScheduleUiState {
  if (loading || !status) return 'CHECKING'

  const issues = status.issues ?? []
  if (issues.some((issue) => issue.severity === 'error')) {
    return 'INSTALLED_ERROR'
  }

  if (status.installState === 'permission-warning') return 'INSTALLED_ERROR'
  if (status.installState === 'not-installed') return 'NOT_INSTALLED'

  if (
    status.installState === 'legacy-install-detected' ||
    status.installState === 'mismatch' ||
    status.installState === 'manual-review' ||
    hasNeverRun ||
    issues.some((issue) => issue.severity === 'warning')
  ) {
    return 'INSTALLED_WARN'
  }

  return 'INSTALLED_OK'
}

/**
 * Filters user-dismissed non-blocking issues without hiding hard scheduler faults.
 *
 * @param issues backend issues or route-synthesized issues.
 * @param dismissed issue codes dismissed in this route session.
 * @returns visible issues that should still affect the screen.
 */
export function visibleScheduleIssues(
  issues: ScheduleIssue[],
  dismissed: Set<string>,
): ScheduleIssue[] {
  return issues.filter(
    (issue) => !issue.dismissible || !dismissed.has(issue.code),
  )
}
