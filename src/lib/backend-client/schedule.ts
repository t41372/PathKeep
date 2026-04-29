/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `scheduleClient`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

import type { ApplyResult, SchedulePlan, ScheduleStatus } from '../types'
import { call } from './shared'

/**
 * Exposes the focused client surface for schedule commands.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
export const scheduleClient = {
  previewInstall: (platform?: string) =>
    call<SchedulePlan>('preview_schedule', { platform }),
  getStatus: (platform?: string) =>
    call<ScheduleStatus>('schedule_status', { platform }),
  applyInstall: (plan: SchedulePlan) =>
    call<ApplyResult>('apply_schedule', { plan }),
  removeInstall: (plan: SchedulePlan) =>
    call<ApplyResult>('remove_schedule', { plan }),
  repairInstall: (plan: SchedulePlan) =>
    call<ApplyResult>('repair_schedule', { plan }),
}
