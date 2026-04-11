import type {
  AppSnapshot,
  AppConfig,
  BackupReport,
  ExportRequest,
  ExportResult,
  RetentionPreview,
  RetentionPruneRequest,
  RetentionPruneResult,
  SnapshotRestorePreview,
  SnapshotRestoreRequest,
} from '../types'
import { call } from './shared'

export const archiveClient = {
  initializeArchive: (config: AppConfig, databaseKey?: string | null) =>
    call<AppSnapshot>('initialize_archive', {
      config,
      databaseKey,
    }),
  runBackupNow: (dueOnly = false) =>
    call<BackupReport>('run_backup_now', { dueOnly }),
  previewSnapshotRestore: (request: SnapshotRestoreRequest) =>
    call<SnapshotRestorePreview>('preview_snapshot_restore', { request }),
  runSnapshotRestore: (request: SnapshotRestoreRequest) =>
    call<BackupReport>('run_snapshot_restore', { request }),
  previewRetentionPrune: () =>
    call<RetentionPreview>('preview_retention_prune'),
  runRetentionPrune: (request: RetentionPruneRequest) =>
    call<RetentionPruneResult>('run_retention_prune', { request }),
  exportHistory: (request: ExportRequest) =>
    call<ExportResult>('export_history', { request }),
}
