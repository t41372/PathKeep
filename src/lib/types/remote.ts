/**
 * This module defines typed front-end contracts for remote-backup preview, execution, and verification.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - `RemoteBackupConfig`
 * - `S3CredentialInput`
 * - `RemoteBackupPreview`
 * - `RemoteBackupResult`
 * - `RemoteBackupVerificationCheck`
 * - `RemoteBackupVerificationFile`
 * - `RemoteBackupVerification`
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

/**
 * Represents persisted configuration for remote backup.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RemoteBackupConfig {
  enabled: boolean
  bucket: string
  region: string
  endpoint?: string | null
  prefix: string
  pathStyle: boolean
  uploadAfterBackup: boolean
  credentialsSaved: boolean
  lastUploadedAt?: string | null
  lastUploadedObjectKey?: string | null
  lastError?: string | null
}
/**
 * Defines the typed shape for s3 credential input.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface S3CredentialInput {
  accessKeyId: string
  secretAccessKey: string
}

/**
 * Represents the preview payload shown before a write or high-risk action happens.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RemoteBackupPreview {
  bundlePath: string
  objectKey: string
  uploadUrl: string
  previewCommand: string
  manualSteps: string[]
  warnings: string[]
}

/**
 * Defines the typed shape for remote backup result.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RemoteBackupResult {
  uploaded: boolean
  bundlePath: string
  objectKey: string
  uploadUrl: string
  message: string
}

/**
 * Defines the typed shape for remote backup verification check.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RemoteBackupVerificationCheck {
  name: string
  status: string
  message: string
}

/**
 * Defines the typed shape for remote backup verification file.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RemoteBackupVerificationFile {
  relativePath: string
  sha256: string
  sizeBytes: number
}

/**
 * Defines the typed shape for remote backup verification.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RemoteBackupVerification {
  bundlePath: string
  bundleVersion: string
  appVersion: string
  createdAt: string
  archiveMode: string
  objectKey: string
  restoreReady: boolean
  checks: RemoteBackupVerificationCheck[]
  warnings: string[]
  restoreSteps: string[]
  manifestFiles: RemoteBackupVerificationFile[]
}
