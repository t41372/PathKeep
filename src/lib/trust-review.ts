import type { AuditRunDetail, ScheduleStatus } from './types'
import type { TranslationKey } from './i18n'

export type AuditSeverity = 'clear' | 'warning' | 'blocked'

export function archiveModeKey(mode: string): TranslationKey {
  return mode === 'Plaintext' || mode === 'plaintext'
    ? 'common.modePlaintext'
    : 'common.modeEncrypted'
}

export function securityModeKey(mode: string): TranslationKey {
  if (mode === 'uninitialized') return 'common.modeUninitialized'
  if (mode === 'locked') return 'common.modeLocked'
  if (mode === 'plaintext') return 'common.modePlaintext'
  return 'common.modeEncrypted'
}

export function importBatchStatusKey(status: string): TranslationKey {
  if (status === 'reverted') return 'common.statusReverted'
  if (status === 'preview') return 'common.statusPreview'
  if (status === 'quarantined') return 'common.statusQuarantined'
  if (status === 'warning') return 'common.statusNeedsAttention'
  return 'common.statusImported'
}

export function importBatchStatusTone(status: string) {
  if (status === 'reverted') return 'danger' as const
  if (status === 'preview') return 'neutral' as const
  if (status === 'quarantined') return 'danger' as const
  return 'success' as const
}

export function healthCheckStatusKey(status: string): TranslationKey {
  if (status === 'warning') return 'common.statusNeedsAttention'
  if (status === 'error' || status === 'blocked') return 'common.statusBlocked'
  if (status === 'pending') return 'common.statusPending'
  return status === 'ok' ? 'common.statusSuccess' : 'common.statusInfo'
}

export function healthCheckStatusTone(status: string) {
  if (status === 'warning') return 'warning' as const
  if (status === 'error' || status === 'blocked') return 'blocked' as const
  return status === 'ok' ? ('success' as const) : ('info' as const)
}

export function runStatusKey(status: string): TranslationKey {
  if (status === 'failed') return 'common.statusFailed'
  if (status === 'running') return 'common.statusRunning'
  if (status === 'pending') return 'common.statusPending'
  return 'common.statusSuccess'
}

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

export function runTriggerKey(trigger: string): TranslationKey {
  return trigger === 'schedule' ? 'audit.scheduledBackup' : 'audit.manualBackup'
}

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

export function auditSeverityKey(severity: AuditSeverity): TranslationKey {
  if (severity === 'blocked') return 'common.statusBlocked'
  if (severity === 'warning') return 'common.statusNeedsAttention'
  return 'common.statusClear'
}

export function auditSeverityTone(severity: AuditSeverity) {
  if (severity === 'blocked') return 'blocked' as const
  if (severity === 'warning') return 'warning' as const
  return 'success' as const
}

export function scheduleInstallTone(status: ScheduleStatus['installState']) {
  if (status === 'installed') return 'success' as const
  if (status === 'not-installed') return 'info' as const
  if (status === 'manual-review' || status === 'mismatch') {
    return 'warning' as const
  }
  return 'blocked' as const
}
