/**
 * Typed front-end client for the whole-app Export / Import commands.
 *
 * Why this file exists:
 * - The Settings → Data Migration section calls `export_app_data`,
 *   `preview_app_data_import`, and `apply_app_data_import` against the
 *   desktop façade. Routes should not type raw command names so the
 *   transport can rename or version safely.
 *
 * Main declarations:
 * - `migrationClient`
 * - `ExportedBundle`
 * - `ImportPreview`
 * - `ImportResult`
 *
 * Source-of-truth notes:
 * - Bundle format and apply semantics: `vault_core::migration` rustdoc.
 * - Transport contract mirrors the dev-IPC dispatch arms in
 *   `src-tauri/src/dev_ipc_bridge/dispatch.rs`.
 */

import type { KindState } from './data-kinds'
import { call } from './shared'

export interface ExportManifestFile {
  path: string
  sha256: string
  sizeBytes: number
}

export interface ExportManifest {
  formatVersion: number
  appVersion: string
  archiveSchemaVersion: number
  /** `"encrypted"` or `"plaintext"`. */
  archiveMode: string
  exportedAt: string
  exporterHostname: string | null
  files: ExportManifestFile[]
}

export interface ExportedBundle {
  bundlePath: string
  manifest: ExportManifest
  bytesWritten: number
}

export interface ImportExclusionNote {
  path: string
  reason: string
}

/**
 * What opening the file does to Agent conversations here, so the check can
 * say it before anything changes. The count stays unknown until the
 * password opens the file.
 */
export interface ImportAgentConversations {
  /** The file has the conversations' storage (PathKeep 0.6 and later), even an empty one. */
  carried: boolean
  /**
   * This computer's conversations stay; the file's are added beside them.
   * Browsing history is still replaced by the file's.
   */
  keepsLocal: boolean
  /**
   * Restoring the copy made before the import brings browsing back and
   * leaves the added conversations, unless the restore is asked to move
   * them to the trash too.
   */
  restoreKeepsAdditions: boolean
  /**
   * How they show here afterwards: `paused` (readable, nothing backed up)
   * on a computer that never chose; otherwise this computer's own choice.
   * `null` when the file carries none.
   */
  stateIfPresent: KindState | null
}

export interface ImportPreview {
  manifest: ExportManifest
  schemaUpToDate: boolean
  migrationsToApply: number[]
  bytesToExtract: number
  exclusionNotes: ImportExclusionNote[]
  /** True when the live project already has an initialized archive. */
  willOverwriteExisting: boolean
  /**
   * False for files PathKeep 0.5 and earlier made: they all record 0.1.0 as
   * their version, so the version they were made with is unknown.
   */
  versionKnown: boolean
  agentConversations: ImportAgentConversations
}

export interface ApplyImportOptions {
  /** Must be true when the preview reports `willOverwriteExisting`. */
  confirmOverwrite: boolean
  /**
   * The password the bundle was exported with. Every bundle is encrypted;
   * the imported archive keeps this password. A wrong one comes back as
   * `archive-password-wrong` and leaves this machine's archive unchanged.
   * Sent as `sourceArchiveKey` to match the Rust serde rename.
   */
  sourceArchiveKey?: string
}

export interface ImportResult {
  manifest: ExportManifest
  migrationsApplied: number[]
  finalSchemaVersion: number
  /** The archive the import replaced was kept as a safety copy ("before an import"). */
  keptPreviousAsCopy: boolean
  /**
   * Why that copy could not be checked, when it could not. It is listed as not
   * checked yet, and a restore from it checks it first.
   */
  previousCopyUnchecked: string | null
}

/**
 * One import that added conversations this computer didn't have. The list
 * lives in the archive, so it stays after the copy made before the import
 * is freed.
 */
export interface ConversationImport {
  id: string
  importedAt: string
  /** Conversations the import added. */
  added: number
  /** Of those, how many an undo would still move to the trash. */
  eligible: number
  /** The folder name of the copy made before the import, while it exists. */
  previousCopy: string | null
}

export interface ConversationImportUndo {
  /** Conversations moved to the trash. */
  moved: number
  /** The copy of them kept first; null when there was nothing to move. */
  copyPath: string | null
}

export const migrationClient = {
  /** The bundle is always encrypted with `exportPassword`; on import it becomes the archive's password. */
  exportAppData: (targetPath: string, exportPassword: string) =>
    call<ExportedBundle>('export_app_data', { targetPath, exportPassword }),
  previewAppDataImport: (bundlePath: string) =>
    call<ImportPreview>('preview_app_data_import', { bundlePath }),
  /**
   * `currentPassword` is this archive's own password, asked for only when
   * the import answers `agent-history-key-required`: the conversations here
   * must be read to be kept.
   */
  applyAppDataImport: (
    bundlePath: string,
    options: ApplyImportOptions,
    currentPassword: string | null = null,
  ) =>
    call<ImportResult>('apply_app_data_import', {
      bundlePath,
      options,
      currentPassword,
    }),
  listConversationImports: () =>
    call<ConversationImport[]>('list_conversation_imports', {}),
  /** Keeps a copy of what the import added, then moves it to the trash. */
  undoConversationImport: (importId: string) =>
    call<ConversationImportUndo>('undo_conversation_import', { importId }),
}
