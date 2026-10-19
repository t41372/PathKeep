/**
 * Typed client for the progress and cancellation of long whole-archive
 * operations (export, import, conversion, mode switch, restore).
 *
 * Why this file exists:
 * - Those commands block until they finish, often for minutes on a large
 *   archive. Progress and Stop come from these two separate, cheap commands,
 *   which never wait behind the operation itself.
 *
 * Source-of-truth notes:
 * - Shapes mirror `vault_worker::maintenance_jobs` and `vault_core::maintenance::Progress`.
 */
import { call } from './shared'

/**
 * `phase` is the operation (`exporting`, …) or its current step (`copying`,
 * `verifying`, …), then `completed`, `cancelled` or `failed` once it ends.
 * `total` is 0 when the step has no countable unit.
 */
export interface MaintenanceProgress {
  phase: string
  database: string | null
  table: string | null
  processed: number
  total: number
}

export const maintenanceClient = {
  progress: () => call<MaintenanceProgress | null>('get_maintenance_progress'),
  /** True when an operation was running and has been asked to stop. */
  cancel: () => call<boolean>('cancel_maintenance'),
}
