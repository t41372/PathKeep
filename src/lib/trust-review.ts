/**
 * This module turns archive, import, security, and run status into the review-friendly labels and tones used by the UI.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `AuditSeverity`
 * - `archiveModeKey`
 * - `securityModeKey`
 * - `importBatchStatusKey`
 * - `importBatchStatusTone`
 * - `healthCheckStatusKey`
 * - `healthCheckStatusTone`
 * - `runStatusKey`
 * - `runTypeKey`
 * - `runTriggerKey`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type { AuditRunDetail, ScheduleStatus } from './types'
import type { TranslationKey } from './i18n'

/**
 * Defines the type-level contract for audit severity.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export type AuditSeverity = 'clear' | 'warning' | 'blocked'

/**
 * Explains how archive mode key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function archiveModeKey(mode: string): TranslationKey {
  return mode === 'Plaintext' || mode === 'plaintext'
    ? 'common.modePlaintext'
    : 'common.modeEncrypted'
}

/**
 * Explains how security mode key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function securityModeKey(mode: string): TranslationKey {
  if (mode === 'uninitialized') return 'common.modeUninitialized'
  if (mode === 'locked') return 'common.modeLocked'
  if (mode === 'plaintext') return 'common.modePlaintext'
  return 'common.modeEncrypted'
}

/**
 * Explains how import batch status key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function importBatchStatusKey(status: string): TranslationKey {
  if (status === 'reverted') return 'common.statusReverted'
  if (status === 'preview') return 'common.statusPreview'
  if (status === 'quarantined') return 'common.statusQuarantined'
  if (status === 'warning') return 'common.statusNeedsAttention'
  return 'common.statusImported'
}

/**
 * Explains how import batch status tone works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function importBatchStatusTone(status: string) {
  if (status === 'reverted') return 'danger' as const
  if (status === 'preview') return 'neutral' as const
  if (status === 'quarantined') return 'danger' as const
  return 'success' as const
}

/**
 * Explains how health check status key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function healthCheckStatusKey(ok: boolean): TranslationKey {
  return ok ? 'common.statusSuccess' : 'common.statusFailed'
}

/**
 * Explains how health check status tone works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function healthCheckStatusTone(ok: boolean) {
  return ok ? ('success' as const) : ('blocked' as const)
}

/**
 * Explains how run status key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function runStatusKey(status: string): TranslationKey {
  if (status === 'failed') return 'common.statusFailed'
  if (status === 'running') return 'common.statusRunning'
  if (status === 'pending') return 'common.statusPending'
  return 'common.statusSuccess'
}

/**
 * Explains how run type key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function runTypeKey(runType: string): TranslationKey {
  if (runType === 'import') return 'audit.runTypeImport'
  if (runType === 'rollback') return 'audit.runTypeRollback'
  if (runType === 'restore') return 'audit.runTypeRestore'
  if (runType === 'rekey') return 'audit.runTypeRekey'
  if (runType === 'doctor') return 'audit.runTypeDoctor'
  if (runType === 'snapshot_restore') return 'audit.runTypeSnapshotRestore'
  if (runType === 'retention_prune') return 'audit.runTypeRetentionPrune'
  if (runType === 'ai_index') return 'audit.runTypeAiIndex'
  if (runType === 'assistant') return 'audit.runTypeAssistant'
  if (runType === 'mcp_query') return 'audit.runTypeMcpQuery'
  return 'audit.runTypeBackup'
}

/**
 * Explains how run trigger key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function runTriggerKey(trigger: string): TranslationKey {
  if (trigger === 'schedule') return 'audit.scheduledBackup'
  if (trigger === 'repair') return 'audit.automaticRepair'
  return 'audit.manualBackup'
}

/**
 * Explains how source kind from profile scope works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function sourceKindFromProfileScope(profileScope: string[]): string[] {
  if (profileScope.length === 0) {
    return ['archive-wide']
  }

  return Array.from(
    new Set(
      profileScope.map((profileId) => {
        const [sourceKind] = profileId.split(':')
        return sourceKind || 'archive-wide'
      }),
    ),
  ).sort()
}

/**
 * Explains how audit severity works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function auditSeverity(
  detail: Pick<AuditRunDetail, 'warnings' | 'errorMessage'>,
): AuditSeverity {
  if (detail.errorMessage) {
    return 'blocked'
  }
  if (detail.warnings.length > 0) {
    return 'warning'
  }
  return 'clear'
}

/**
 * Explains how audit severity key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function auditSeverityKey(severity: AuditSeverity): TranslationKey {
  if (severity === 'blocked') return 'common.statusBlocked'
  if (severity === 'warning') return 'common.statusNeedsAttention'
  return 'common.statusClear'
}

/**
 * Explains how audit severity tone works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function auditSeverityTone(severity: AuditSeverity) {
  if (severity === 'blocked') return 'blocked' as const
  if (severity === 'warning') return 'warning' as const
  return 'success' as const
}

/**
 * Explains how schedule install tone works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function scheduleInstallTone(status: ScheduleStatus['installState']) {
  if (status === 'installed') return 'success' as const
  if (status === 'not-installed') return 'info' as const
  if (status === 'manual-review' || status === 'mismatch') {
    return 'warning' as const
  }
  return 'blocked' as const
}
