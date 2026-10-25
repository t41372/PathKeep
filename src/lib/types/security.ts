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
  /**
   * The archive's current password. The backend refuses to change the
   * password of, or decrypt, an encrypted archive without it
   * (`archive-password-required` / `archive-password-wrong`); a plaintext
   * archive has none, so encrypting one leaves it out.
   */
  currentKey?: string | null
  /**
   * Whether the keychain should hold the archive's key once the change is
   * done, and the setting with it. The backend applies both under the same
   * hold as the rewrite, so quitting as soon as the rewrite ends cannot leave
   * them behind. Left out, both stay as they are (previews).
   */
  rememberInKeychain?: boolean | null
}

/**
 * Reports the outcome of a proactive archive-encryption reconcile pass.
 *
 * The reconcile command self-heals a drifted encryption state right after unlock —
 * so a user's next backup does not fail on a stale mode mismatch.
 */
export interface ReconcileReport {
  repaired: boolean
  fromMode: 'Plaintext' | 'Encrypted' | null
  toMode: 'Plaintext' | 'Encrypted'
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
  /** Diagnostic English prose; only rendered when a code has no catalog entry. */
  warnings: string[]
  /**
   * Stable warning codes aligned index-for-index with `warnings`. An entry is
   * `''` when the backend is passing an opaque diagnostic through.
   */
  warningCodes?: string[]
}
