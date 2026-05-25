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
}

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
