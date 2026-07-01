/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - If a route needs desktop data, start here before reaching for legacy preview helpers.
 *
 * Main declarations:
 * - `archiveClient`
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

import type {
  AppSnapshot,
  AppConfig,
  BackupReport,
  ExportRequest,
  ExportResult,
  FullArchiveRestoreReport,
  RecoverySnapshot,
  RetentionPreview,
  RetentionPruneRequest,
  RetentionPruneResult,
  SnapshotRestorePreview,
  SnapshotRestoreRequest,
} from '../types'
import { call } from './shared'

/**
 * Exposes the focused client surface for archive commands.
 *
 * The backend-client layer exists to keep command names, transport, and route code decoupled, so focused declarations here are intentional.
 */
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
  listRecoverySnapshots: () =>
    call<RecoverySnapshot[]>('list_recovery_snapshots'),
  runFullArchiveRestore: (request: SnapshotRestoreRequest) =>
    call<FullArchiveRestoreReport>('run_full_archive_restore', { request }),
}
