import { describe, expect, test } from 'vitest'
import {
  archiveModeKey,
  auditSeverity,
  auditSeverityKey,
  auditSeverityTone,
  healthCheckStatusKey,
  healthCheckStatusTone,
  importBatchStatusKey,
  importBatchStatusTone,
  runStatusKey,
  runTypeKey,
  runTriggerKey,
  scheduleInstallTone,
  securityModeKey,
  sourceKindFromProfileScope,
} from './trust-review'

describe('trust review helpers', () => {
  test('maps archive and security modes to translation keys', () => {
    expect(archiveModeKey('Encrypted')).toBe('common.modeEncrypted')
    expect(archiveModeKey('Plaintext')).toBe('common.modePlaintext')
    expect(archiveModeKey('plaintext')).toBe('common.modePlaintext')
    expect(securityModeKey('uninitialized')).toBe('common.modeUninitialized')
    expect(securityModeKey('locked')).toBe('common.modeLocked')
    expect(securityModeKey('plaintext')).toBe('common.modePlaintext')
    expect(securityModeKey('encrypted')).toBe('common.modeEncrypted')
  })

  test('maps import and health statuses to translated labels and tones', () => {
    expect(importBatchStatusKey('imported')).toBe('common.statusImported')
    expect(importBatchStatusKey('reverted')).toBe('common.statusReverted')
    expect(importBatchStatusKey('preview')).toBe('common.statusPreview')
    expect(importBatchStatusKey('quarantined')).toBe('common.statusQuarantined')
    expect(importBatchStatusKey('warning')).toBe('common.statusNeedsAttention')
    expect(importBatchStatusTone('imported')).toBe('success')
    expect(importBatchStatusTone('preview')).toBe('neutral')
    expect(importBatchStatusTone('reverted')).toBe('danger')
    expect(importBatchStatusTone('quarantined')).toBe('danger')

    expect(healthCheckStatusKey('ok')).toBe('common.statusSuccess')
    expect(healthCheckStatusKey('info')).toBe('common.statusInfo')
    expect(healthCheckStatusKey('warning')).toBe('common.statusNeedsAttention')
    expect(healthCheckStatusKey('error')).toBe('common.statusBlocked')
    expect(healthCheckStatusKey('blocked')).toBe('common.statusBlocked')
    expect(healthCheckStatusKey('pending')).toBe('common.statusPending')
    expect(healthCheckStatusTone('ok')).toBe('success')
    expect(healthCheckStatusTone('info')).toBe('info')
    expect(healthCheckStatusTone('warning')).toBe('warning')
    expect(healthCheckStatusTone('error')).toBe('blocked')
    expect(healthCheckStatusTone('blocked')).toBe('blocked')
  })

  test('maps audit helpers to translated filters and tones', () => {
    expect(runStatusKey('success')).toBe('common.statusSuccess')
    expect(runStatusKey('failed')).toBe('common.statusFailed')
    expect(runStatusKey('running')).toBe('common.statusRunning')
    expect(runStatusKey('pending')).toBe('common.statusPending')
    expect(runTypeKey('backup')).toBe('audit.runTypeBackup')
    expect(runTypeKey('import')).toBe('audit.runTypeImport')
    expect(runTypeKey('rollback')).toBe('audit.runTypeRollback')
    expect(runTypeKey('restore')).toBe('audit.runTypeRestore')
    expect(runTypeKey('doctor')).toBe('audit.runTypeDoctor')
    expect(runTypeKey('snapshot_restore')).toBe('audit.runTypeSnapshotRestore')
    expect(runTypeKey('ai_index')).toBe('audit.runTypeAiIndex')
    expect(runTypeKey('assistant')).toBe('audit.runTypeAssistant')
    expect(runTypeKey('mcp_query')).toBe('audit.runTypeMcpQuery')
    expect(runTriggerKey('schedule')).toBe('audit.scheduledBackup')
    expect(runTriggerKey('manual')).toBe('audit.manualBackup')
    expect(
      sourceKindFromProfileScope(['chrome:Default', 'firefox:Default']),
    ).toEqual(['chrome', 'firefox'])
    expect(
      sourceKindFromProfileScope([
        'firefox:Default',
        'chrome:Personal',
        'chrome:Work',
      ]),
    ).toEqual(['chrome', 'firefox'])
    expect(sourceKindFromProfileScope([])).toEqual(['archive-wide'])
    expect(sourceKindFromProfileScope([':Imported profile'])).toEqual([
      'archive-wide',
    ])

    expect(
      auditSeverity({
        warnings: [],
        errorMessage: null,
      }),
    ).toBe('clear')
    expect(
      auditSeverity({
        warnings: ['Review this run'],
        errorMessage: null,
      }),
    ).toBe('warning')
    expect(
      auditSeverity({
        warnings: [],
        errorMessage: 'blocked',
      }),
    ).toBe('blocked')
    expect(auditSeverityKey('clear')).toBe('common.statusClear')
    expect(auditSeverityKey('warning')).toBe('common.statusNeedsAttention')
    expect(auditSeverityKey('blocked')).toBe('common.statusBlocked')
    expect(auditSeverityTone('clear')).toBe('success')
    expect(auditSeverityTone('warning')).toBe('warning')
    expect(auditSeverityTone('blocked')).toBe('blocked')
  })

  test('maps schedule install states to callout tones', () => {
    expect(scheduleInstallTone('installed')).toBe('success')
    expect(scheduleInstallTone('not-installed')).toBe('info')
    expect(scheduleInstallTone('manual-review')).toBe('warning')
    expect(scheduleInstallTone('mismatch')).toBe('warning')
    expect(scheduleInstallTone('permission-warning')).toBe('blocked')
    expect(scheduleInstallTone('legacy-install-detected')).toBe('blocked')
  })
})
