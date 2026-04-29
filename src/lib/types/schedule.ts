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
  manualStepDetails?: ScheduleManualStep[]
  applyCommands: string[][]
  rollbackCommands: string[][]
  applySupported: boolean
}

/**
 * Describes one manual scheduler operation the route can render or verify in isolation.
 *
 * Manual steps are first-class UI data so users can recover without relying on
 * the one-click native action path.
 */
export interface ScheduleManualStep {
  id: string
  titleKey: string
  summaryKey: string
  whyKey: string
  command?: string[] | null
  filePath?: string | null
  fileContents?: string | null
  directoryPath?: string | null
  canAutoRun: boolean
  canVerify: boolean
}

/**
 * Names a scheduler problem with localized copy keys and concrete host evidence.
 *
 * The route renders these instead of raw backend warning strings so the user
 * sees the problem, consequence, and available repair path in their language.
 */
export interface ScheduleIssue {
  code: string
  severity: 'warning' | 'error' | 'info'
  titleKey: string
  detailKey: string
  consequenceKey: string
  evidence: string[]
  repairAction?: string | null
  dismissible: boolean
}

/**
 * Captures a single verification row for install checks and per-step validation.
 *
 * These checks let the UI show partial success/failure without collapsing a
 * multi-step scheduler operation into one opaque message.
 */
export interface ScheduleVerificationCheck {
  key: string
  status: 'ok' | 'warning' | 'error' | 'pending'
  labelKey: string
  detailKey: string
  evidence: string[]
}

/**
 * Records the last scheduler action that produced an auditable outcome.
 *
 * This gives the settings route a durable result line after install, repair, or
 * removal without reusing transient button state as source of truth.
 */
export interface ScheduleLastAction {
  action: string
  status: string
  message: string
  at: string
  auditPath?: string | null
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
  stepResults?: ScheduleVerificationCheck[]
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
  manualStepDetails?: ScheduleManualStep[]
  auditPath?: string | null
  lastSuccessfulBackupAt?: string | null
  warnings: string[]
  issues?: ScheduleIssue[]
  verificationChecks?: ScheduleVerificationCheck[]
  checkedAt?: string | null
  lastAction?: ScheduleLastAction | null
}
