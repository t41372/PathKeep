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
      analytics: {
        enabled: false,
        consentGrantedAt: null,
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
    expect(
      shouldAttemptKeyringAutoUnlock(
        buildSnapshot({
          archiveStatus: {
            initialized: true,
            encrypted: true,
            unlocked: true,
            databasePath: '/tmp/archive.sqlite',
          },
        }),
      ),
    ).toBe(false)
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
      activeStep: 0,
      progressLabel: '1 / 3',
    })

    const fallback = buildBackupOverlay(
      buildProgress({
        phase: 'unknown-phase' as BackupProgressEvent['phase'],
        totalSteps: 0,
      }),
      t,
    )
    expect(fallback.label).toBe(t('shell.runningManualBackup'))
    expect(fallback.progressValue).toBeNull()
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
      progressLabel: '2 / 4',
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
      progressLabel: '4 / 4',
      progressValue: 100,
    })
  })
})
