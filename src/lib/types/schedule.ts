/**
 * This module defines typed front-end contracts for schedule preview, apply results, and install-state review.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - `ScheduleGeneratedFile`
 * - `SchedulePlan`
 * - `ApplyResult`
 * - `ScheduleStatus`
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

/**
 * Defines the typed shape for schedule generated file.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ScheduleGeneratedFile {
  relativePath: string
  absolutePath?: string | null
  purpose: string
  contents: string
}

/**
 * Defines the typed shape for schedule plan.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface SchedulePlan {
  platform: string
  label: string
  executablePath: string
  generatedFiles: ScheduleGeneratedFile[]
  manualSteps: string[]
  applyCommands: string[][]
  rollbackCommands: string[][]
  applySupported: boolean
}

/**
 * Defines the typed shape for apply result.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ApplyResult {
  applied: boolean
  platform: string
  files: string[]
  auditPath?: string | null
  message: string
}

/**
 * Represents a read model or status snapshot for schedule.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ScheduleStatus {
  platform: string
  label: string
  dueAfterHours: number
  checkIntervalHours: number
  applySupported: boolean
  installState: string
  detectedFiles: string[]
  manualSteps: string[]
  auditPath?: string | null
  lastSuccessfulBackupAt?: string | null
  warnings: string[]
}
