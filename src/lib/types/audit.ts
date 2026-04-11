/**
 * This module defines typed front-end contracts for audit-ledger review, doctor findings, and repair reports.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - `AuditArtifact`
 * - `AuditRunDetail`
 * - `HealthCheck`
 * - `HealthReport`
 * - `HealthRepairReport`
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

import type { BackupRunOverview } from './archive'

/**
 * Defines the typed shape for audit artifact.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AuditArtifact {
  kind: string
  path: string
  checksum?: string | null
  sizeBytes?: number | null
  createdAt: string
  reason?: string | null
}

/**
 * Represents the detailed view model for audit run.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AuditRunDetail {
  run: BackupRunOverview
  trigger: string
  timezone?: string | null
  dueOnly: boolean
  profileScope: string[]
  warnings: string[]
  errorMessage?: string | null
  stats: Record<string, unknown>
  manifestPath?: string | null
  manifestHash?: string | null
  artifacts: AuditArtifact[]
}
/**
 * Defines the typed shape for health check.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface HealthCheck {
  name: string
  status: string
  message: string
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface HealthReport {
  generatedAt: string
  checks: HealthCheck[]
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface HealthRepairReport {
  runId?: number | null
  repairedImportAudits: number
  repairedVisibilityRows: number
  clearedDerivedRows: number
  notes: string[]
}
