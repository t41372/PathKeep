import type { AuditRunDetail, ImportBatchOverview } from '../../lib/types'

export interface AuditDetailState {
  runId: number | null
  detail: AuditRunDetail | null
  error: string | null
}

export interface AuditFilterState {
  runType: string
  severity: 'all' | 'clear' | 'warning' | 'blocked'
  sourceKind: string
  profileId: string
  artifactType: string
}

export type AuditDetailTab = 'summary' | 'artifacts' | 'warnings'

export type Translator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

export function parseAuditTimestamp(value?: string | null) {
  if (!value) return Number.NaN
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : Number.NaN
}

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
