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
  ArchiveUpgradeAssessment,
  BackupReport,
  ExportRequest,
  ExportProgress,
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
  /** With the old archive's password, the conversion's table counts too. */
  assessArchiveUpgrade: (databaseKey?: string | null) =>
    call<ArchiveUpgradeAssessment>('assess_archive_upgrade', { databaseKey }),
  runBackupNow: (dueOnly = false, kindSpecific = false) =>
    call<BackupReport>('run_backup_now', { dueOnly, kindSpecific }),
  previewSnapshotRestore: (request: SnapshotRestoreRequest) =>
    call<SnapshotRestorePreview>('preview_snapshot_restore', { request }),
  runSnapshotRestore: (request: SnapshotRestoreRequest) =>
    call<BackupReport>('run_snapshot_restore', { request }),
  previewRetentionPrune: () =>
    call<RetentionPreview>('preview_retention_prune'),
  runRetentionPrune: (request: RetentionPruneRequest) =>
    call<RetentionPruneResult>('run_retention_prune', { request }),
  /** Asks where to save first; `null` when the person closes the dialog. */
  exportHistory: (request: ExportRequest) =>
    call<ExportResult | null>('export_history', { request }),
  getExportProgress: (exportId: string) =>
    call<ExportProgress | null>('get_export_progress', { exportId }),
  cancelExport: (exportId: string) =>
    call<boolean>('cancel_export', { exportId }),
  listRecoverySnapshots: () =>
    call<RecoverySnapshot[]>('list_recovery_snapshots'),
  runFullArchiveRestore: (
    request: SnapshotRestoreRequest,
    options: FullArchiveRestoreOptions = {},
  ) =>
    call<FullArchiveRestoreReport>('run_full_archive_restore', {
      request,
      key: options.key ?? null,
      currentPassword: options.currentPassword ?? null,
      undoConversations: options.undoConversations ?? false,
      useCopyEncryption: options.useCopyEncryption ?? false,
    }),
}

/**
 * The two passwords a restore can need are different things: `key` opens
 * the copy (the password the archive had when the copy was made);
 * `currentPassword` opens the archive itself when it could not be opened,
 * so the restored browsing history keeps the archive's password.
 */
export interface FullArchiveRestoreOptions {
  key?: string | null
  currentPassword?: string | null
  /**
   * With the copy made before an import: also move the conversations that
   * import added to the trash, after keeping a copy of them.
   */
  undoConversations?: boolean
  /**
   * Only for an encrypted archive whose key can't be read at all: restore
   * under the copy's own encryption. The current files are kept aside as a
   * copy, and Agent conversations start empty.
   */
  useCopyEncryption?: boolean
}
