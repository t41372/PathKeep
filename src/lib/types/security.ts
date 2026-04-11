/**
 * This module defines typed front-end contracts for keyring review, rekey preview, and security-state surfaces.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - `KeyringStatusReport`
 * - `RekeyRequest`
 * - `SecurityStatus`
 * - `RekeyPreview`
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

import type { ArchiveMode } from './archive'

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface KeyringStatusReport {
  available: boolean
  backend: string
  storedSecret: boolean
  message?: string | null
}
/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RekeyRequest {
  newMode: ArchiveMode
  newKey?: string | null
}

/**
 * Represents a read model or status snapshot for security.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface SecurityStatus {
  initialized: boolean
  mode: string
  encrypted: boolean
  unlocked: boolean
  databasePath: string
  strongholdPath: string
  rememberDatabaseKeyInKeyring: boolean
  lastSuccessfulBackupAt?: string | null
  lastRekeyAt?: string | null
  lastRekeyRunId?: number | null
  lastRekeySnapshotPath?: string | null
  keyringStatus: KeyringStatusReport
  warnings: string[]
}

/**
 * Represents the preview payload shown before a write or high-risk action happens.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RekeyPreview {
  currentMode: ArchiveMode
  nextMode: ArchiveMode
  requiresNewKey: boolean
  snapshotPath: string
  tempDatabasePath: string
  steps: string[]
  warnings: string[]
}
