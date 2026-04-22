/**
 * @file job-panel-helpers.ts
 * @description Pure helper contracts for Jobs recent-activity panels.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Hold AI queue state labeling outside the TSX render owner so fast-refresh rules stay clean.
 * - Define the lightweight translator contract shared by extracted Jobs panels.
 *
 * ## Not responsible for
 * - Rendering any Jobs UI.
 * - Owning queue mutations or runtime reads.
 *
 * ## Dependencies
 * - Depends only on the Jobs translator callback contract.
 *
 * ## Performance notes
 * - Pure synchronous helpers used during render only.
 */

export type JobsTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Maps raw AI queue states to shipped Jobs copy so queue rows never leak raw
 * backend states directly into the UI.
 */
export function aiJobStateLabel(state: string, jobsT: JobsTranslator) {
  switch (state) {
    case 'queued':
      return jobsT('jobStateQueued')
    case 'running':
      return jobsT('jobStateRunning')
    case 'succeeded':
      return jobsT('jobStateSucceeded')
    case 'failed':
      return jobsT('jobStateFailed')
    case 'cancelled':
      return jobsT('jobStateCancelled')
    case 'paused':
      return jobsT('jobStatePaused')
    case 'stale':
      return jobsT('jobStateStale')
    default:
      return state
  }
}
