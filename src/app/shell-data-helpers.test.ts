/**
 * @file shell-data-helpers.test.ts
 * @description Focused unit coverage for the pure helper logic extracted out of the shell-data provider.
 * @module app/shell-data
 *
 * ## Responsibilities
 * - Prove the helper split preserved shell bootstrap and busy-overlay semantics.
 * - Cover the scope-key, queue-count, and backup-progress branches without booting the full provider.
 *
 * ## Not responsible for
 * - Re-testing provider wiring or backend mutation flows already covered by shell-data provider suites.
 * - Verifying shell route rendering or context consumers.
 *
 * ## Dependencies
 * - Depends on the shared i18n translator contract and typed shell-data helper exports.
 * - Uses the real translation catalog so helper assertions stay aligned with shipped copy.
 *
 * ## Performance notes
 * - These tests stay pure and synchronous so helper regressions are cheap to catch before provider suites run.
 */

import { describe, expect, test } from 'vitest'
import { defaultExplorerBackgroundPrefetchPages } from '../lib/explorer-preferences'
import { createTranslator } from '../lib/i18n'
import type {
  AppSnapshot,
  BackupProgressEvent,
  DashboardSnapshot,
} from '../lib/types'
import {
  backupStepLabels,
  buildBackupOverlay,
  buildUninitializedDashboardFallback,
  countActiveRuntimeJobs,
  emptyRuntimeStatus,
  isAppLockError,
  runtimeStatusScopeKey,
  shouldAttemptKeyringAutoUnlock,
} from './shell-data-helpers'

function buildSnapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  return {
    directories: {
      appRoot: '/tmp/pathkeep',
      configPath: '/tmp/pathkeep/config.json',
      archiveDatabasePath: '/tmp/pathkeep/archive.sqlite',
      searchDatabasePath: '/tmp/pathkeep/search.sqlite',
      intelligenceDatabasePath: '/tmp/pathkeep/intelligence.sqlite',
      auditRepoPath: '/tmp/pathkeep/audit',
      manifestsDir: '/tmp/pathkeep/manifests',
      exportsDir: '/tmp/pathkeep/exports',
      rawSnapshotsDir: '/tmp/pathkeep/snapshots',
      stagingDir: '/tmp/pathkeep/staging',
      quarantineDir: '/tmp/pathkeep/quarantine',
      scheduleDir: '/tmp/pathkeep/schedule',
      semanticIndexDir: '/tmp/pathkeep/semantic',
      intelligenceBlobsDir: '/tmp/pathkeep/blobs',
      logsDir: '/tmp/pathkeep/logs',
      rustLogPath: '/tmp/pathkeep/logs/rust.log',
      frontendLogPath: '/tmp/pathkeep/logs/frontend.log',
      crashReportsDir: '/tmp/pathkeep/crash',
      strongholdPath: '/tmp/pathkeep/pathkeep.stronghold',
      strongholdSaltPath: '/tmp/pathkeep/pathkeep.salt',
    },
    runtimeDiagnostics: {
      logDirectory: '/tmp/pathkeep/logs',
      rustLogPath: '/tmp/pathkeep/logs/rust.log',
      frontendLogPath: '/tmp/pathkeep/logs/frontend.log',
      crashReportsDirectory: '/tmp/pathkeep/crash',
      latestCrashReport: null,
    },
    config: {
      initialized: true,
      archiveMode: 'Encrypted',
      preferredLanguage: 'en',
      dueAfterHours: 72,
      scheduleCheckIntervalHours: 6,
      checkpointDays: 90,
      captureFavicons: true,
      selectedProfileIds: ['chrome:Default'],
      gitEnabled: true,
      rememberDatabaseKeyInKeyring: false,
      appAutostart: false,
      explorerBackgroundPrefetchPages: defaultExplorerBackgroundPrefetchPages,
      appLock: {
        enabled: false,
        idleTimeoutMinutes: 5,
        biometricEnabled: false,
        passcodeEnabled: true,
        passcodeConfigured: false,
        recoveryHint: null,
      },
      remoteBackup: {
        enabled: false,
        bucket: '',
        region: 'us-east-1',
        endpoint: null,
        prefix: 'pathkeep',
        pathStyle: true,
        uploadAfterBackup: false,
        credentialsSaved: false,
        lastUploadedAt: null,
        lastUploadedObjectKey: null,
        lastError: null,
      },
      enrichment: {
        plugins: [],
      },
      deterministic: {
        modules: [],
      },
      ai: {
        enabled: false,
        assistantEnabled: false,
        semanticIndexEnabled: false,
        mcpEnabled: false,
        skillEnabled: false,
        autoIndexAfterBackup: false,
        jobQueuePaused: false,
        jobQueueConcurrency: 1,
        enrichmentEnabled: true,
        enrichmentPlugins: [],
        llmProviderId: null,
        embeddingProviderId: null,
        retrievalTopK: 8,
        assistantSystemPrompt: 'Evidence only.',
        llmProviders: [],
        embeddingProviders: [],
      },
    },
    archiveStatus: {
      initialized: true,
      encrypted: true,
      unlocked: true,
      databasePath: '/tmp/archive.sqlite',
    },
    keyringStatus: {
      available: false,
      storedSecret: false,
      backend: 'preview',
      message: null,
    },
    appLockStatus: {
      enabled: false,
      locked: false,
      idleTimeoutMinutes: 5,
      biometricAvailable: false,
      biometricEnabled: false,
      biometricState: 'touch-id-unavailable',
      passcodeEnabled: true,
      passcodeConfigured: false,
      configPath: '/tmp/pathkeep/app-lock.json',
      lockReason: null,
      lockedAt: null,
      lastUnlockedAt: null,
      recoveryHint: null,
      warnings: [],
      degradationNotes: [],
    },
    aiStatus: {
      enabled: false,
      assistantEnabled: false,
      mcpEnabled: false,
      skillEnabled: false,
      state: 'disabled',
      ready: false,
      indexedItems: 0,
      lastIndexedAt: null,
      llmProviderId: null,
      embeddingProviderId: null,
      queuePaused: false,
      queueConcurrency: 1,
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      recentJobs: [],
      semanticSidecarBytes: 0,
      semanticMetadataBytes: 0,
      estimatedEmbeddingTokens: 0,
      warning: null,
    },
    intelligenceStatus: {
      ready: false,
      lastRunAt: null,
      runs: 0,
      cards: 0,
      topics: 0,
      threads: 0,
      queryGroups: 0,
      referencePages: 0,
      contentCoverage: 0,
      warning: null,
    },
    browserProfiles: [],
    recentRuns: [],
    recentImportBatches: [],
    ...overrides,
  }
}

function buildProgress(
  overrides: Partial<BackupProgressEvent> = {},
): BackupProgressEvent {
  return {
    phase: 'prepare',
    label: 'Preparing backup',
    detail: 'Preparing archive',
    step: 0,
    totalSteps: 3,
    completedProfiles: 0,
    totalProfiles: 0,
    profileId: null,
    ...overrides,
  }
}

describe('shell-data helpers', () => {
  const t = createTranslator('en')

  test('detects app-lock refusal errors without catching unrelated failures', () => {
    expect(isAppLockError(new Error('PathKeep is currently locked'))).toBe(true)
    expect(isAppLockError(new Error('unlock the app before reading'))).toBe(
      true,
    )
    expect(isAppLockError(new Error('archive read failed'))).toBe(false)
    expect(isAppLockError('locked')).toBe(false)
  })

  test('builds an uninitialized dashboard fallback that preserves recent runs', () => {
    const snapshot = buildSnapshot({
      recentRuns: [
        {
          id: 7,
          startedAt: new Date().toISOString(),
          finishedAt: new Date().toISOString(),
          status: 'success',
          runType: 'backup',
          profileScope: ['chrome:Default'],
          trigger: 'manual',
          profilesProcessed: 1,
          newVisits: 10,
          newUrls: 5,
          newDownloads: 0,
        },
      ],
    })

    const dashboard = buildUninitializedDashboardFallback(snapshot)
    expect(dashboard.totalProfiles).toBe(0)
    expect(dashboard.totalVisits).toBe(0)
    expect(dashboard.recentRuns).toEqual(snapshot.recentRuns)
    expect(dashboard.storage).toMatchObject({
      archiveDatabaseBytes: 0,
      sourceEvidenceDatabaseBytes: 0,
      intelligenceBlobBytes: 0,
    } satisfies Partial<DashboardSnapshot['storage']>)
  })

  test('only attempts keyring auto unlock when all explicit prerequisites are present', () => {
    const unlockable = buildSnapshot({
      archiveStatus: {
        initialized: true,
        encrypted: true,
        unlocked: false,
        databasePath: '/tmp/archive.sqlite',
      },
      keyringStatus: {
        available: true,
        storedSecret: true,
        backend: 'preview',
        message: null,
      },
      config: {
        ...buildSnapshot().config,
        rememberDatabaseKeyInKeyring: true,
      },
    })

    expect(shouldAttemptKeyringAutoUnlock(unlockable)).toBe(true)
    const cases: Array<[string, AppSnapshot]> = [
      [
        'unencrypted archive',
        buildSnapshot({
          archiveStatus: {
            initialized: true,
            encrypted: false,
            unlocked: false,
            databasePath: '/tmp/archive.sqlite',
          },
          config: {
            ...unlockable.config,
            rememberDatabaseKeyInKeyring: true,
          },
          keyringStatus: unlockable.keyringStatus,
        }),
      ],
      [
        'already unlocked archive',
        buildSnapshot({
          archiveStatus: {
            initialized: true,
            encrypted: true,
            unlocked: true,
            databasePath: '/tmp/archive.sqlite',
          },
          config: {
            ...unlockable.config,
            rememberDatabaseKeyInKeyring: true,
          },
          keyringStatus: unlockable.keyringStatus,
        }),
      ],
      [
        'remembered key disabled',
        buildSnapshot({
          archiveStatus: unlockable.archiveStatus,
          config: {
            ...unlockable.config,
            rememberDatabaseKeyInKeyring: false,
          },
          keyringStatus: unlockable.keyringStatus,
        }),
      ],
      [
        'keyring unavailable',
        buildSnapshot({
          archiveStatus: unlockable.archiveStatus,
          config: unlockable.config,
          keyringStatus: {
            ...unlockable.keyringStatus,
            available: false,
          },
        }),
      ],
      [
        'no stored keyring secret',
        buildSnapshot({
          archiveStatus: unlockable.archiveStatus,
          config: unlockable.config,
          keyringStatus: {
            ...unlockable.keyringStatus,
            storedSecret: false,
          },
        }),
      ],
    ]

    for (const [name, snapshot] of cases) {
      expect(shouldAttemptKeyringAutoUnlock(snapshot), name).toBe(false)
    }
  })

  test('returns neutral runtime status and stable scope keys for locked vs unlocked archives', () => {
    expect(emptyRuntimeStatus()).toEqual({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    })

    expect(runtimeStatusScopeKey(null)).toBe('locked-or-uninitialized')
    expect(
      runtimeStatusScopeKey(
        buildSnapshot({
          archiveStatus: {
            initialized: true,
            encrypted: true,
            unlocked: false,
            databasePath: '/tmp/archive.sqlite',
          },
        }),
      ),
    ).toBe('locked-or-uninitialized')
    expect(runtimeStatusScopeKey(buildSnapshot())).toBe(
      '/tmp/archive.sqlite|chrome:Default|live',
    )
    expect(
      runtimeStatusScopeKey(
        buildSnapshot({
          config: {
            ...buildSnapshot().config,
            selectedProfileIds: ['chrome:Default', 'chrome:Work'],
            ai: {
              ...buildSnapshot().config.ai,
              jobQueuePaused: true,
            },
          },
        }),
      ),
    ).toBe('/tmp/archive.sqlite|chrome:Default,chrome:Work|paused')
  })

  test('counts queue work across both AI and intelligence runtime owners', () => {
    expect(
      countActiveRuntimeJobs({
        aiQueue: {
          paused: false,
          concurrency: 1,
          queued: 2,
          running: 1,
          failed: 0,
          recentJobs: [],
        },
        intelligence: {
          queue: {
            queued: 3,
            running: 4,
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            lastActivityAt: null,
          },
          plugins: [],
          modules: [],
          recentJobs: [],
          notes: [],
        },
        loading: false,
        error: null,
      }),
    ).toBe(10)
    expect(countActiveRuntimeJobs(emptyRuntimeStatus())).toBe(0)
  })

  test('returns stable backup step labels in shell order', () => {
    expect(backupStepLabels(t)).toEqual([
      t('shell.backupStepPrepare'),
      t('shell.backupStepArchive'),
      t('shell.backupStepRefresh'),
    ])
  })

  test('maps backup prepare and fallback phases to shell overlay copy', () => {
    const prepare = buildBackupOverlay(buildProgress(), t)
    expect(prepare).toMatchObject({
      label: t('shell.runningManualBackup'),
      detail: t('shell.runningManualBackupDetail'),
      activeStep: 0,
      progressLabel: t('shell.backupProgressPending'),
      progressValue: null,
    })
    expect(prepare.logLines).toEqual([t('shell.runningManualBackupDetail')])

    const fallback = buildBackupOverlay(
      buildProgress({
        phase: 'unknown-phase' as BackupProgressEvent['phase'],
        totalSteps: 0,
      }),
      t,
    )
    expect(fallback.label).toBe(t('shell.runningManualBackup'))
    expect(fallback.detail).toBe(t('shell.runningManualBackupDetail'))
    expect(fallback.progressLabel).toBe(t('shell.backupProgressPending'))
    expect(fallback.progressValue).toBeNull()
    expect(fallback.logLines).toEqual([t('shell.runningManualBackupDetail')])
  })

  test('maps profile and finalize progress to archive/write and refresh steps', () => {
    const stage = buildBackupOverlay(
      buildProgress({
        phase: 'stage-profile',
        step: 1,
        totalSteps: 3,
        completedProfiles: 1,
        totalProfiles: 4,
        profileId: 'chrome:Work',
      }),
      t,
    )
    expect(stage).toMatchObject({
      label: t('shell.backupWritingArchive'),
      activeStep: 1,
      progressLabel: t('shell.backupRecordProgressPending'),
      progressValue: null,
    })
    expect(stage.detail).toContain('chrome:Work')

    const finalize = buildBackupOverlay(
      buildProgress({
        phase: 'finalize',
        step: 2,
        totalSteps: 3,
        completedProfiles: 4,
        totalProfiles: 4,
      }),
      t,
    )
    expect(finalize).toMatchObject({
      label: t('shell.refreshingArchiveViews'),
      activeStep: 2,
      detail: t('shell.backupFinalizeProgress', {
        current: 4,
        total: 4,
      }),
      progressLabel: '4 / 4',
      progressValue: 100,
    })
    expect(finalize.logLines).toEqual([
      t('shell.backupFinalizeProgress', {
        current: 4,
        total: 4,
      }),
    ])
  })

  test('prefers record-level backup progress over profile-level percentages', () => {
    const overlay = buildBackupOverlay(
      buildProgress({
        phase: 'ingest-profile',
        step: 1,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 2,
        profileId: 'chrome:Default',
        sourceLabel: 'Google Chrome / Default',
        processedRecords: 10_000,
        importedRecords: 9_500,
        duplicateRecords: 500,
      }),
      t,
    )

    expect(overlay).toMatchObject({
      label: t('shell.backupWritingArchive'),
      activeStep: 1,
      detail: 'Google Chrome / Default',
      progressLabel: '10,000 records processed',
      progressValue: null,
    })
    expect(overlay.logLines).toEqual([
      '10,000 records processed',
      '9,500 new · 500 duplicates',
    ])
  })

  test('maps record totals, partial stats, and skipped rows into backup overlay detail', () => {
    const duplicateOnly = buildBackupOverlay(
      buildProgress({
        phase: 'ingest-profile',
        step: 1,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 1,
        processedRecords: 10,
        totalRecords: 20,
        duplicateRecords: 3,
        skippedRecords: 0,
      }),
      t,
    )

    expect(duplicateOnly.progressValue).toBe(50)
    expect(duplicateOnly.logLines).toEqual([
      '10 records processed',
      '0 new · 3 duplicates',
    ])

    const importedAndSkipped = buildBackupOverlay(
      buildProgress({
        phase: 'ingest-profile',
        step: 1,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 1,
        processedRecords: 12,
        totalRecords: 24,
        importedRecords: 12,
        duplicateRecords: null,
        skippedRecords: 2,
      }),
      t,
    )

    expect(importedAndSkipped.progressValue).toBe(50)
    expect(importedAndSkipped.logLines).toEqual([
      '12 records processed',
      '12 new · 0 duplicates',
      '2 skipped',
    ])
  })

  test('ignores invalid record counters instead of leaking impossible progress values', () => {
    const overlay = buildBackupOverlay(
      buildProgress({
        phase: 'ingest-profile',
        step: 1,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 1,
        processedRecords: Number.NaN,
        totalRecords: Number.POSITIVE_INFINITY,
        importedRecords: -1,
        duplicateRecords: Number.NEGATIVE_INFINITY,
        skippedRecords: -5,
      }),
      t,
    )

    expect(overlay).toMatchObject({
      progressLabel: t('shell.backupRecordProgressPending'),
      progressValue: null,
    })
    expect(overlay.logLines).toEqual([t('shell.backupWritingArchiveDetail')])
  })

  test('keeps zero record totals and zero profile totals indeterminate', () => {
    const recordOverlay = buildBackupOverlay(
      buildProgress({
        phase: 'ingest-profile',
        step: 1,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 1,
        processedRecords: 0,
        totalRecords: 0,
        importedRecords: 0,
        duplicateRecords: 0,
        skippedRecords: 0,
      }),
      t,
    )

    expect(recordOverlay).toMatchObject({
      progressLabel: '0 records processed',
      progressValue: null,
    })
    expect(recordOverlay.logLines).toEqual([
      '0 records processed',
      '0 new · 0 duplicates',
    ])

    const finalizeOverlay = buildBackupOverlay(
      buildProgress({
        phase: 'finalize',
        step: 2,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 0,
      }),
      t,
    )

    expect(finalizeOverlay.progressLabel).toBeNull()
    expect(finalizeOverlay.progressValue).toBeNull()
  })
})
