/**
 * This module renders the Audit Ledger route, where runs, artifacts, warnings, and rollback hints stay reviewable instead of hidden behind success toasts.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `AuditDetailState`
 * - `AuditFilterState`
 * - `AuditDetailTab`
 * - `Translator`
 * - `parseAuditTimestamp`
 * - `resolveBatchEventTime`
 * - `pickRelatedImportBatch`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import type { AuditRunDetail, ImportBatchOverview } from '../../lib/types'

/**
 * Captures the state shape used by `AuditDetail`.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export interface AuditDetailState {
  runId: number | null
  detail: AuditRunDetail | null
  error: string | null
}

/**
 * Captures the state shape used by `AuditFilter`.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export interface AuditFilterState {
  runType: string
  severity: 'all' | 'clear' | 'warning' | 'blocked'
  sourceKind: string
  profileId: string
  artifactType: string
}

/**
 * Enumerates the tabs available on this front-end surface.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export type AuditDetailTab = 'summary' | 'artifacts' | 'warnings'

/**
 * Defines the type-level contract for translator.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Parses audit timestamp into the shape this surface expects.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function parseAuditTimestamp(value?: string | null) {
  if (!value) return Number.NaN
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : Number.NaN
}

/**
 * Resolves batch event time from the available inputs.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function resolveBatchEventTime(
  batch: ImportBatchOverview,
  runType: string,
): number {
  if (runType === 'rollback') {
    return parseAuditTimestamp(
      batch.revertedAt ?? batch.importedAt ?? batch.createdAt,
    )
  }

  return parseAuditTimestamp(
    batch.importedAt ?? batch.revertedAt ?? batch.createdAt,
  )
}

/**
 * Explains how pick related import batch works.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function pickRelatedImportBatch(
  detail: AuditRunDetail | null,
  recentImportBatches: ImportBatchOverview[],
) {
  if (!detail) return null
  const runType = detail.run.runType ?? 'backup'
  if (!['import', 'rollback', 'restore'].includes(runType)) return null

  const runProfileId = detail.profileScope[0] ?? null
  const runTimestamp = parseAuditTimestamp(
    detail.run.finishedAt ?? detail.run.startedAt,
  )
  const sameProfileBatches = recentImportBatches.filter(
    (batch) => !runProfileId || batch.profileId === runProfileId,
  )

  return (
    sameProfileBatches.slice().sort((left, right) => {
      const leftDistance = Math.abs(
        resolveBatchEventTime(left, runType) - runTimestamp,
      )
      const rightDistance = Math.abs(
        resolveBatchEventTime(right, runType) - runTimestamp,
      )
      return leftDistance - rightDistance
    })[0] ?? null
  )
}
