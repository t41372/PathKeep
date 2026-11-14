/**
 * Invented numbers for Settings → 儲存: what can be freed, the safety
 * copies, the search indexes. The archive's own size comes from the per-type
 * sizes the Backup pages use, so the usage card and 存檔大小 agree.
 */
import { DAY, NOW } from '~/data/fixture'
import { archiveBytes } from '../../backup/sources-data'

export type BucketId = 'snapshots' | 'exports' | 'staging' | 'quarantine'

/** What 釋放空間 lists (`preview_retention_prune`), in the backend's order. */
export const initialBuckets: Record<BucketId, number> = {
  snapshots: 2_481_000_000,
  exports: 912_400_000,
  staging: 126_300_000,
  quarantine: 51_200_000,
}

export const bucketCopy: Record<BucketId, { title: string; description: string }> = {
  snapshots: { title: '快照', description: '瀏覽器歷史的副本，以及重大變更前保存的安全副本。' },
  exports: { title: '匯出檔案', description: '你之前匯出的檔案。' },
  staging: { title: '暫存檔案', description: '備份和匯入時留下的檔案。' },
  quarantine: { title: '移到一旁的檔案', description: 'PathKeep 在還原或修復時移到一旁的檔案。' },
}

/** Search index, Insights data, the semantic sidecar and its blobs. */
export const INDEX_BYTES = 2_268_000_000

/** Every type's archive, added up. */
export const archiveTotal = Object.values(archiveBytes).reduce((sum, bytes) => sum + (bytes ?? 0), 0)

/** A safety copy as `list_recovery_snapshots` returns it. */
export interface RecoverySnapshot {
  id: string
  sourceOp: string
  createdAt: number | null
  label: string
  sizeBytes: number
  encrypted: boolean
}

export const recoverySnapshots: RecoverySnapshot[] = [
  { id: 'rekey-1', sourceOp: 'rekey', createdAt: NOW - 3 * DAY - 2 * 3600_000, label: 'archive-before-rekey', sizeBytes: 1_402_000_000, encrypted: true },
  { id: 'periodic-1', sourceOp: 'periodic', createdAt: NOW - 6 * DAY, label: 'periodic', sizeBytes: 1_377_000_000, encrypted: true },
  { id: 'import-1', sourceOp: 'import', createdAt: NOW - 24 * DAY, label: 'before-import', sizeBytes: 1_190_000_000, encrypted: true },
  { id: 'reconcile-1', sourceOp: 'reconcile', createdAt: NOW - 45 * DAY, label: 'before-reconcile', sizeBytes: 1_044_000_000, encrypted: false },
  { id: 'legacy-1', sourceOp: 'legacy', createdAt: null, label: 'history-vault-0.3.sqlite', sizeBytes: 812_000_000, encrypted: false },
]
