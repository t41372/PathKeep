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

export interface ImportPreview {
  manifest: ExportManifest
  schemaUpToDate: boolean
  migrationsToApply: number[]
  bytesToExtract: number
  exclusionNotes: ImportExclusionNote[]
  /** True when the live project already has an initialized archive. */
  willOverwriteExisting: boolean
}

export interface ApplyImportOptions {
  /** Must be true when the preview reports `willOverwriteExisting`. */
  confirmOverwrite: boolean
  /**
   * Required when `preview.manifest.archiveMode === 'encrypted'`. The
   * value is the archive cipher key that was used on the source
   * machine. Plaintext bundles ignore this field. Sent as
   * `sourceArchiveKey` over the wire to match the Rust serde rename.
   *
   * Two typed-error responses can come back from the backend when the
   * bundle is encrypted:
   *   - prefix `source_archive_key required` → no key supplied; the UI
   *     should render the source-key prompt.
   *   - prefix `source_archive_key invalid` → wrong key; the UI should
   *     keep the prompt and surface the error inline. In both cases
   *     the live archive on this machine is unchanged.
   */
  sourceArchiveKey?: string
}

/**
 * Error-message prefix the backend uses to signal "this bundle is
 * encrypted but no source key was supplied." The frontend matches on
 * this prefix to swap the apply error into a source-key input prompt
 * instead of a generic error banner.
 *
 * Pinned constant rather than free text so a future copy tweak on the
 * Rust side does not silently break the UI detection.
 */
export const IMPORT_SOURCE_KEY_REQUIRED_PREFIX = 'source_archive_key required'

/**
 * Error-message prefix the backend uses to signal "the supplied source
 * key does not decrypt the imported archive." Distinct from
 * `IMPORT_SOURCE_KEY_REQUIRED_PREFIX` so the UI can swap copy between
 * "please enter" and "wrong key, try again."
 */
export const IMPORT_SOURCE_KEY_INVALID_PREFIX = 'source_archive_key invalid'

export interface ImportResult {
  manifest: ExportManifest
  migrationsApplied: number[]
  finalSchemaVersion: number
  preservedPreviousAsBak: boolean
}

export const migrationClient = {
  exportAppData: (targetPath: string) =>
    call<ExportedBundle>('export_app_data', { targetPath }),
  previewAppDataImport: (bundlePath: string) =>
    call<ImportPreview>('preview_app_data_import', { bundlePath }),
  applyAppDataImport: (bundlePath: string, options: ApplyImportOptions) =>
    call<ImportResult>('apply_app_data_import', { bundlePath, options }),
}
