/**
 * @file backend.ts
 * @description Browser-preview backend command fixture and compatibility facade for preview runtime consumers and tests.
 * @module lib/backend
 *
 * ## Responsibilities
 * - Route browser-preview command calls through a deterministic in-memory fixture state when Tauri transport is unavailable.
 * - Expose the compatibility `backend` facade and `backendTestHarness` used by preview-mode consumers and tests.
 * - Keep browser-preview command behavior aligned with shipping frontend contracts instead of ad hoc route-local mocks.
 *
 * ## Not responsible for
 * - Owning typed desktop command clients; that belongs to `backend-client/*` and `ipc/bridge.ts`.
 * - Becoming the default runtime path for new route code when desktop transport is available.
 * - Hiding the fact that browser-preview is a fixture-backed environment rather than the real desktop backend.
 *
 * ## Dependencies
 * - Depends on typed frontend contracts from `./types` and enrichment config helpers.
 * - Reuses static browser-preview fixture data from `./backend-preview-fixtures`.
 *
 * ## Performance notes
 * - Browser-preview state stays in memory and deterministic, so command handlers should remain cheap and free of unbounded cloning or recomputation.
 */

import { invoke, isTauri } from '@tauri-apps/api/core'
import { mockBuildInfo } from './backend-preview-fixtures'
import {
  buildMockQueueStatus,
  createMockState,
  ensureMockUnlocked,
  type MockBackendState,
  normalizeMockConfig,
  syncMockAiStatus,
  syncMockAppLockState,
  syncMockIntelligenceRuntime,
  validateMockAppLockConfig,
} from './backend-preview-state'
import {
  buildMockSchedulePlan,
  buildMockScheduleStatus,
  normalizeMockPlatform,
  overrideMockSchedule,
} from './backend-preview-schedule'
import {
  buildMockSearchQueries,
  filterMockHistory,
  paginateMockAiSearch,
  uniqueUrlCount,
} from './backend-preview-search'
import {
  buildMockAuditRunDetail,
  buildMockDashboardSnapshot,
  clearDerivedIntelligenceFixture,
  prependMockRun,
  previewRemoteBackupFixture,
  verifyRemoteBackupFixture,
} from './backend-preview-support'
import {
  buildMockRekeyPreview,
  buildMockRetentionPreview,
  buildMockSecurityStatus,
  buildMockSnapshotRestorePreview,
  buildMockTakeoutInspection,
  mutateImportBatch,
} from './backend-preview-workflows'
import type {
  AiAssistantRequest,
  AiAssistantResponse,
  AiIndexReport,
  AiIndexRequest,
  AiIntegrationPreview,
  AiProviderConnectionTestReport,
  AiProviderConnectionTestRequest,
  AiProviderSecretInput,
  AiQueueJob,
  AiQueueStatus,
  AiSearchRequest,
  AiSearchResponse,
  AppBuildInfo,
  AppConfig,
  AppUpdateCheckResult,
  AppLockStatus,
  AppSnapshot,
  ApplyResult,
  AuditRunDetail,
  BackupReport,
  ClearDerivedIntelligenceReport,
  DashboardSnapshot,
  ExportRequest,
  ExportResult,
  HealthRepairReport,
  HealthReport,
  HistoryQuery,
  HistoryQueryResponse,
  ImportBatchDetail,
  IntelligenceRuntimeSnapshot,
  KeyringStatusReport,
  RekeyPreview,
  RekeyRequest,
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
  RetentionPreview,
  RetentionPruneRequest,
  RetentionPruneResult,
  SchedulePlan,
  ScheduleStatus,
  SecurityStatus,
  SetAppLockPasscodeRequest,
  S3CredentialInput,
  SnapshotRestorePreview,
  SnapshotRestoreRequest,
  TakeoutInspection,
  TakeoutRequest,
  UnlockAppSessionRequest,
  UpdateInstallState,
} from './types'
import type {
  SearchEngineRule,
  SearchEngineRuleInput,
} from './core-intelligence'

let mockState = createMockState()
// Stryker restore all

/**
 * Explains how call works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauri()) {
    return invoke<T>(command, args)
  }

  mockState.snapshot.config = normalizeMockConfig(
    mockState.snapshot.config,
    mockState.s3Credentials,
  )
  syncMockAppLockState(mockState)
  ensureMockUnlocked(command, mockState)

  switch (command) {
    case 'app_build_info':
      return mockBuildInfo as T
    case 'app_lock_status':
      return structuredClone(mockState.snapshot.appLockStatus) as T
    case 'app_snapshot':
      syncMockAiStatus(mockState)
      return structuredClone(mockState.snapshot) as T
    case 'save_config': {
      const nextConfig = normalizeMockConfig(
        structuredClone(args?.config as AppConfig),
        mockState.s3Credentials,
      )
      validateMockAppLockConfig(mockState, nextConfig)
      mockState.snapshot.config = nextConfig
      mockState.snapshot.archiveStatus.encrypted =
        nextConfig.archiveMode === 'Encrypted'
      syncMockAppLockState(mockState)
      syncMockAiStatus(mockState)
      return structuredClone(mockState.snapshot) as T
    }
    case 'initialize_archive': {
      const nextConfig = normalizeMockConfig(
        structuredClone(args?.config as AppConfig),
        mockState.s3Credentials,
      )
      validateMockAppLockConfig(mockState, nextConfig)
      const databaseKey =
        typeof args?.databaseKey === 'string' ? args.databaseKey : null
      if (
        nextConfig.archiveMode === 'Encrypted' &&
        (!databaseKey || !databaseKey.trim())
      ) {
        throw new Error(
          'Mock encrypted archive initialization requires a database key.',
        )
      }
      nextConfig.initialized = true
      mockState.snapshot.config = nextConfig
      mockState.snapshot.archiveStatus = {
        ...mockState.snapshot.archiveStatus,
        initialized: true,
        encrypted: nextConfig.archiveMode === 'Encrypted',
        unlocked:
          nextConfig.archiveMode === 'Plaintext' ||
          Boolean(databaseKey && databaseKey.trim()),
        warning: null,
      }
      syncMockAppLockState(mockState)
      return structuredClone(mockState.snapshot) as T
    }
    case 'set_app_lock_passcode': {
      const request = args?.request as SetAppLockPasscodeRequest | undefined
      const passcode = request?.passcode?.trim()
      if (!passcode || passcode.length < 4) {
        throw new Error(
          'App lock passcodes must be at least 4 characters long.',
        )
      }
      mockState.appLockPasscode = passcode
      mockState.appLockRecoveryHint = request?.recoveryHint?.trim() || null
      syncMockAppLockState(mockState)
      return structuredClone(mockState.snapshot.appLockStatus) as T
    }
    case 'clear_app_lock_passcode':
      mockState.appLockPasscode = null
      mockState.appLockRecoveryHint = null
      mockState.snapshot.config.appLock.enabled = false
      mockState.snapshot.appLockStatus = {
        ...mockState.snapshot.appLockStatus,
        locked: false,
        lockReason: null,
        lockedAt: null,
      }
      syncMockAppLockState(mockState)
      return structuredClone(mockState.snapshot.appLockStatus) as T
    case 'lock_app_session':
      if (mockState.snapshot.config.appLock.enabled) {
        mockState.snapshot.appLockStatus = {
          ...mockState.snapshot.appLockStatus,
          locked: true,
          lockReason:
            typeof args?.reason === 'string' && args.reason.trim()
              ? args.reason
              : 'manual',
          lockedAt: new Date().toISOString(),
        }
      }
      syncMockAppLockState(mockState)
      return structuredClone(mockState.snapshot.appLockStatus) as T
    case 'unlock_app_session': {
      const request = args?.request as UnlockAppSessionRequest | undefined
      if (request?.useBiometric) {
        if (!mockState.snapshot.config.appLock.biometricEnabled) {
          throw new Error(
            'Biometric unlock is currently turned off in Settings.',
          )
        }
        if (mockState.biometricState !== 'touch-id-available') {
          throw new Error(
            mockState.biometricState === 'touch-id-unavailable'
              ? 'Touch ID is unavailable on this Mac right now. Use the app lock passcode instead.'
              : 'Biometric unlock is not available in the current desktop build.',
          )
        }
      }
      if (mockState.snapshot.config.appLock.enabled) {
        if (
          !request?.useBiometric &&
          (request?.passcode?.trim() ?? '') !== mockState.appLockPasscode
        ) {
          if (!mockState.snapshot.config.appLock.passcodeEnabled) {
            throw new Error(
              'PathKeep cannot unlock without an enabled app lock credential.',
            )
          }
          throw new Error('The app lock passcode did not match.')
        }
        mockState.snapshot.appLockStatus = {
          ...mockState.snapshot.appLockStatus,
          locked: false,
          lockReason: null,
          lockedAt: null,
          lastUnlockedAt: new Date().toISOString(),
        }
      }
      syncMockAppLockState(mockState)
      return structuredClone(mockState.snapshot.appLockStatus) as T
    }
    case 'rekey_archive': {
      const request = args?.request as RekeyRequest
      const finishedAt = new Date().toISOString()
      const run = prependMockRun(mockState, {
        id: (mockState.snapshot.recentRuns[0]?.id ?? 1847) + 1,
        startedAt: finishedAt,
        finishedAt,
        status: 'success',
        runType: 'rekey',
        trigger: 'manual',
        profileScope: [],
        manifestHash: `preview-manifest-rekey-${finishedAt}`,
        profilesProcessed: 0,
        newVisits: 0,
        newUrls: 0,
        newDownloads: 0,
      })
      mockState.snapshot.config.archiveMode = request.newMode
      mockState.snapshot.archiveStatus.encrypted =
        request.newMode === 'Encrypted'
      mockState.snapshot.archiveStatus.unlocked =
        request.newMode === 'Plaintext' ||
        Boolean(request.newKey && request.newKey.trim())
      void run
      return structuredClone(mockState.snapshot) as T
    }
    case 'preview_rekey_archive':
      return buildMockRekeyPreview(
        mockState,
        structuredClone(args?.request as RekeyRequest),
      ) as T
    case 'preview_snapshot_restore':
      return buildMockSnapshotRestorePreview(
        mockState,
        structuredClone(args?.request as SnapshotRestoreRequest),
      ) as T
    case 'run_snapshot_restore': {
      const request = structuredClone(
        args?.request as SnapshotRestoreRequest,
      ) ?? {
        snapshotPath: `${mockState.snapshot.directories.rawSnapshotsDir}/run-1`,
      }
      const preview = buildMockSnapshotRestorePreview(mockState, request)
      if (!preview.executeSupported) {
        throw new Error(
          'Automatic restore is only supported for saved browser source checkpoints right now.',
        )
      }
      const profileId = preview.sourceProfileId!
      const finishedAt = new Date().toISOString()
      const run = prependMockRun(mockState, {
        id: (mockState.snapshot.recentRuns[0]?.id ?? 1847) + 1,
        startedAt: finishedAt,
        finishedAt,
        status: 'success',
        runType: 'snapshot_restore',
        trigger: 'manual',
        profileScope: [profileId],
        manifestHash: `preview-manifest-snapshot-${finishedAt}`,
        profilesProcessed: 1,
        newVisits: preview.estimatedVisits,
        newUrls: preview.estimatedUrls,
        newDownloads: preview.estimatedDownloads,
      })
      return {
        dueSkipped: false,
        reason: null,
        run,
        profiles: [
          {
            profileId,
            newVisits: preview.estimatedVisits,
            newUrls: preview.estimatedUrls,
            newDownloads: preview.estimatedDownloads,
            checkpointCreated: false,
            notes: [],
          },
        ],
        manifestPath: `${mockState.snapshot.directories.manifestsDir}/2026-04-09/run-${run.id}-snapshot-restore.json`,
        gitCommit: null,
        warnings: [],
        remoteBackup: null,
      } as T
    }
    case 'preview_retention_prune':
      return buildMockRetentionPreview(mockState) as T
    case 'run_retention_prune': {
      const request = args?.request as RetentionPruneRequest | undefined
      const preview = buildMockRetentionPreview(mockState)
      const selected = preview.buckets.filter((bucket) =>
        request?.bucketIds?.includes(bucket.id),
      )
      const finishedAt = new Date().toISOString()
      const run = prependMockRun(mockState, {
        id: (mockState.snapshot.recentRuns[0]?.id ?? 1847) + 1,
        startedAt: finishedAt,
        finishedAt,
        status: 'success',
        runType: 'retention_prune',
        trigger: 'manual',
        profileScope: [],
        manifestHash: `preview-manifest-prune-${finishedAt}`,
        profilesProcessed: 0,
        newVisits: 0,
        newUrls: 0,
        newDownloads: 0,
      })
      return {
        runId: run.id,
        deletedBytes: selected.reduce(
          (total, bucket) => total + bucket.bytes,
          0,
        ),
        deletedFiles: selected.reduce(
          (total, bucket) => total + bucket.itemCount,
          0,
        ),
        buckets: selected,
        warnings: preview.warnings,
      } as T
    }
    case 'set_session_database_key':
      mockState.snapshot.archiveStatus.unlocked = true
      return undefined as T
    case 'clear_session_database_key':
      mockState.snapshot.archiveStatus.unlocked =
        mockState.snapshot.config.archiveMode === 'Plaintext'
      return undefined as T
    case 'reset_local_secret_vault':
      return undefined as T
    case 'open_path_in_file_manager':
      return (
        typeof args?.path === 'string'
          ? args.path
          : mockState.snapshot.directories.appRoot
      ) as T
    case 'open_external_url':
      return (
        typeof args?.url === 'string' ? args.url : 'https://example.com'
      ) as T
    case 'check_for_app_update':
      return {
        availability: {
          supported: false,
          checkedAt: new Date().toISOString(),
          available: false,
          currentVersion: mockBuildInfo.version,
          version: null,
          notes: null,
          publishedAt: null,
          error:
            'In-browser preview cannot check desktop update channels. Use a packaged desktop build instead.',
          downloadUrl:
            'https://github.com/t41372/BrowserHistoryBackup/releases',
        },
        pendingUpdate: null,
      } as T
    case 'download_and_install_app_update':
      return {
        phase: 'unsupported',
        version: null,
        downloadedBytes: null,
        contentLength: null,
        message:
          'In-browser preview cannot download or install desktop updates.',
      } as T
    case 'relaunch_after_update':
      return false as T
    case 'run_backup_now': {
      if (!mockState.snapshot.config.initialized) {
        throw new Error('Initialize the archive before running a backup.')
      }
      if (mockState.snapshot.config.selectedProfileIds.length === 0) {
        throw new Error('Select at least one profile before running a backup.')
      }
      const finishedAt = new Date().toISOString()
      const nextRunId = (mockState.snapshot.recentRuns[0]?.id ?? 1847) + 1
      const run = {
        id: nextRunId,
        startedAt: finishedAt,
        finishedAt,
        status: 'success',
        runType: 'backup',
        trigger: 'manual',
        profileScope: mockState.snapshot.config.selectedProfileIds,
        manifestHash: `preview-manifest-${nextRunId}`,
        profilesProcessed: mockState.snapshot.config.selectedProfileIds.filter(
          (profileId) => profileId.startsWith('chrome:'),
        ).length,
        newVisits: mockState.history.items.length,
        newUrls: uniqueUrlCount(mockState.history.items),
        newDownloads: 1,
      }
      prependMockRun(mockState, run)
      mockState.snapshot.archiveStatus.initialized = true
      mockState.snapshot.archiveStatus.unlocked = true
      mockState.snapshot.archiveStatus.lastSuccessfulBackupAt = finishedAt
      return {
        dueSkipped: false,
        run,
        profiles: mockState.snapshot.config.selectedProfileIds
          .filter((profileId) => profileId.startsWith('chrome:'))
          .map((profileId) => ({
            profileId,
            newVisits: 1,
            newUrls: 1,
            newDownloads: 1,
            checkpointCreated: true,
            notes: [],
          })),
        warnings: [],
        remoteBackup: null,
      } as T
    }
    case 'query_history':
      return filterMockHistory(mockState, args?.query as HistoryQuery) as T
    case 'load_dashboard_snapshot':
      return buildMockDashboardSnapshot(mockState) as T
    case 'load_audit_run_detail':
      return buildMockAuditRunDetail(
        mockState,
        Number(args?.runId ?? mockState.snapshot.recentRuns[0]?.id ?? 1848),
      ) as T
    case 'load_intelligence_runtime':
      return structuredClone(mockState.intelligenceRuntime) as T
    case 'retry_intelligence_job': {
      const jobId = Number(args?.jobId ?? 0)
      mockState.intelligenceRuntime.recentJobs =
        mockState.intelligenceRuntime.recentJobs.map((job) =>
          job.id === jobId && job.retryable
            ? {
                ...job,
                state: 'queued',
                finishedAt: null,
                updatedAt: new Date().toISOString(),
                heartbeatAt: null,
                progressLabel: null,
                progressDetail: null,
                progressCurrent: null,
                progressTotal: null,
                progressPercent: null,
                lastError: null,
                retryable: false,
                cancellable: true,
              }
            : job,
        )
      syncMockIntelligenceRuntime(mockState)
      return structuredClone(mockState.intelligenceRuntime) as T
    }
    case 'cancel_intelligence_job': {
      const jobId = Number(args?.jobId ?? 0)
      mockState.intelligenceRuntime.recentJobs =
        mockState.intelligenceRuntime.recentJobs.map((job) =>
          job.id === jobId && job.cancellable
            ? {
                ...job,
                state: 'cancelled',
                finishedAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
                heartbeatAt: null,
                lastError: null,
                retryable: true,
                cancellable: false,
              }
            : job,
        )
      syncMockIntelligenceRuntime(mockState)
      return structuredClone(mockState.intelligenceRuntime) as T
    }
    case 'inspect_takeout':
      return buildMockTakeoutInspection(
        mockState,
        args?.request
          ? String((args.request as TakeoutRequest).sourcePath)
          : '/tmp/takeout.zip',
        true,
      ) as T
    case 'import_takeout':
      return buildMockTakeoutInspection(
        mockState,
        args?.request
          ? String((args.request as TakeoutRequest).sourcePath)
          : '/tmp/takeout.zip',
        false,
      ) as T
    case 'preview_import_batch': {
      const batchId = Number(args?.batchId ?? 0)
      if (mockState.importBatchDetails[batchId]) {
        return structuredClone(mockState.importBatchDetails[batchId]) as T
      }
      buildMockTakeoutInspection(mockState, '/tmp/takeout.zip', false)
      return structuredClone(mockState.importBatchDetails[1]) as T
    }
    case 'revert_import_batch':
      return mutateImportBatch(
        mockState,
        Number(args?.batchId ?? 1),
        'revert',
      ) as T
    case 'restore_import_batch':
      return mutateImportBatch(
        mockState,
        Number(args?.batchId ?? 1),
        'restore',
      ) as T
    case 'preview_schedule':
      return (mockState.schedulePlanOverrides[
        normalizeMockPlatform(args?.platform)
      ] ?? buildMockSchedulePlan(args?.platform)) as T
    case 'schedule_status':
      return (mockState.scheduleStatusOverrides[
        normalizeMockPlatform(args?.platform)
      ] ?? buildMockScheduleStatus(mockState, args?.platform)) as T
    case 'doctor_report':
      return {
        generatedAt: new Date().toISOString(),
        checks: [
          {
            name: 'import-artifacts',
            status: mockState.snapshot.recentImportBatches.length
              ? 'ok'
              : 'info',
            message: mockState.snapshot.recentImportBatches.length
              ? 'Import batch audit artifacts are present and reviewable.'
              : 'No import batches have been created yet.',
          },
          {
            name: 'visibility-state',
            status: mockState.snapshot.recentImportBatches.some(
              (batch) => batch.status === 'reverted',
            )
              ? 'warning'
              : 'ok',
            message: mockState.snapshot.recentImportBatches.some(
              (batch) => batch.status === 'reverted',
            )
              ? 'One or more batches are reverted. Verify downstream read models after restore.'
              : 'Visible import rows match the current batch state.',
          },
        ],
      } as T
    case 'repair_health': {
      const repairRun = prependMockRun(mockState, {
        id: (mockState.snapshot.recentRuns[0]?.id ?? 1) + 1,
        startedAt: new Date().toISOString(),
        finishedAt: new Date().toISOString(),
        status: 'success',
        runType: 'doctor',
        trigger: 'manual',
        profileScope: [],
        manifestHash: null,
        profilesProcessed: 0,
        newVisits: mockState.snapshot.recentImportBatches.some(
          (batch) => batch.status === 'reverted',
        )
          ? 1
          : 0,
        newUrls: 0,
        newDownloads: 0,
      })
      return {
        runId: repairRun.id,
        repairedImportAudits: mockState.snapshot.recentImportBatches.length
          ? 1
          : 0,
        repairedVisibilityRows: mockState.snapshot.recentImportBatches.some(
          (batch) => batch.status === 'reverted',
        )
          ? 1
          : 0,
        clearedDerivedRows: mockState.snapshot.recentImportBatches.length
          ? 2
          : 0,
        notes: ['Browser preview mode simulates a targeted doctor repair run.'],
      } as T
    }
    case 'preview_remote_backup':
      return previewRemoteBackupFixture(mockState) as T
    case 'run_remote_backup': {
      const preview = previewRemoteBackupFixture(mockState)
      const uploaded = Boolean(mockState.s3Credentials)
      const finishedAt = new Date().toISOString()
      mockState.snapshot.config.remoteBackup.lastError = uploaded
        ? null
        : 'Store S3 credentials before executing the remote backup.'
      if (uploaded) {
        mockState.snapshot.config.remoteBackup.lastUploadedAt = finishedAt
        mockState.snapshot.config.remoteBackup.lastUploadedObjectKey =
          preview.objectKey
      }
      mockState.snapshot.config = normalizeMockConfig(
        mockState.snapshot.config,
        mockState.s3Credentials,
      )
      return {
        uploaded,
        bundlePath: preview.bundlePath,
        objectKey: preview.objectKey,
        uploadUrl: preview.uploadUrl,
        message: uploaded
          ? 'Browser preview mode simulated the upload and produced a local bundle for verification.'
          : 'Store S3 credentials before executing the remote backup.',
      } as T
    }
    case 'verify_remote_backup':
      return verifyRemoteBackupFixture(
        mockState,
        typeof args?.bundlePath === 'string' ? args.bundlePath : undefined,
      ) as T
    case 'keyring_status':
      return structuredClone(mockState.snapshot.keyringStatus) as T
    case 'security_status':
      return buildMockSecurityStatus(mockState) as T
    case 'keyring_get_database_key':
      return mockState.keyringSecret as T
    case 'keyring_store_database_key':
      mockState.keyringSecret =
        typeof args?.value === 'string' ? args.value : mockState.keyringSecret
      mockState.snapshot.keyringStatus.storedSecret = Boolean(
        mockState.keyringSecret,
      )
      return structuredClone(mockState.snapshot.keyringStatus) as T
    case 'keyring_clear_database_key':
      mockState.keyringSecret = null
      mockState.snapshot.keyringStatus.storedSecret = false
      return structuredClone(mockState.snapshot.keyringStatus) as T
    case 'store_s3_credentials':
      mockState.s3Credentials = structuredClone(
        args?.credentials as S3CredentialInput,
      )
      mockState.snapshot.config = normalizeMockConfig(
        mockState.snapshot.config,
        mockState.s3Credentials,
      )
      return undefined as T
    case 'clear_s3_credentials':
      mockState.s3Credentials = null
      mockState.snapshot.config.remoteBackup.lastError = null
      mockState.snapshot.config = normalizeMockConfig(
        mockState.snapshot.config,
        mockState.s3Credentials,
      )
      return undefined as T
    case 'store_ai_provider_api_key': {
      const providerId = (args?.input as AiProviderSecretInput | undefined)
        ?.providerId
      mockState.snapshot.config.ai.llmProviders =
        mockState.snapshot.config.ai.llmProviders.map((provider) =>
          provider.id === providerId
            ? { ...provider, apiKeySaved: true }
            : provider,
        )
      mockState.snapshot.config.ai.embeddingProviders =
        mockState.snapshot.config.ai.embeddingProviders.map((provider) =>
          provider.id === providerId
            ? { ...provider, apiKeySaved: true }
            : provider,
        )
      return structuredClone(mockState.snapshot) as T
    }
    case 'clear_ai_provider_api_key': {
      const providerId = args?.providerId as string | undefined
      mockState.snapshot.config.ai.llmProviders =
        mockState.snapshot.config.ai.llmProviders.map((provider) =>
          provider.id === providerId
            ? { ...provider, apiKeySaved: false }
            : provider,
        )
      mockState.snapshot.config.ai.embeddingProviders =
        mockState.snapshot.config.ai.embeddingProviders.map((provider) =>
          provider.id === providerId
            ? { ...provider, apiKeySaved: false }
            : provider,
        )
      return structuredClone(mockState.snapshot) as T
    }
    case 'test_ai_provider_connection':
      return {
        providerId:
          (args?.request as AiProviderConnectionTestRequest | undefined)
            ?.providerId ?? 'preview-provider',
        purpose:
          (args?.request as AiProviderConnectionTestRequest | undefined)
            ?.purpose ?? 'embedding',
        model: 'preview-model',
        ok: true,
        latencyMs: 24,
        capabilities: {
          supportsChat: true,
          supportsEmbeddings: true,
          supportsStreaming: true,
          supportsToolUse: true,
          supportsStructuredOutput: true,
        },
        warnings: [],
        message: 'Browser preview mode fakes a successful provider probe.',
      } as T
    case 'load_ai_queue_status':
      syncMockAiStatus(mockState)
      return buildMockQueueStatus(mockState) as T
    case 'run_ai_queue_jobs':
      mockState.queueJobs = mockState.queueJobs.map((job) =>
        job.state === 'queued'
          ? {
              ...job,
              state: 'succeeded',
              attempt: job.attempt + 1,
              runId: 42,
              finishedAt: new Date().toISOString(),
              summary: 'Preview queue drained this job.',
            }
          : job,
      )
      syncMockAiStatus(mockState)
      return buildMockQueueStatus(mockState) as T
    case 'replay_ai_job': {
      const jobId = args?.jobId as number
      mockState.queueJobs = mockState.queueJobs.map((job) =>
        job.id === jobId
          ? {
              ...job,
              state: mockState.snapshot.config.ai.jobQueuePaused
                ? 'paused'
                : 'queued',
              attempt: 0,
              runId: null,
              startedAt: null,
              finishedAt: null,
              heartbeatAt: null,
              errorCode: null,
              errorMessage: null,
            }
          : job,
      )
      syncMockAiStatus(mockState)
      return structuredClone(
        mockState.queueJobs.find((job) => job.id === jobId),
      ) as T
    }
    case 'cancel_ai_job': {
      const jobId = args?.jobId as number
      mockState.queueJobs = mockState.queueJobs.map((job) =>
        job.id === jobId
          ? {
              ...job,
              state: 'cancelled',
              finishedAt: new Date().toISOString(),
            }
          : job,
      )
      syncMockAiStatus(mockState)
      return structuredClone(
        mockState.queueJobs.find((job) => job.id === jobId),
      ) as T
    }
    case 'build_ai_index': {
      const buildJobId = mockState.nextAiJobId++
      mockState.queueJobs = [
        {
          id: buildJobId,
          jobType: 'index-build',
          state: 'succeeded',
          priority: 70,
          attempt: 1,
          maxAttempts: 3,
          runId: 31,
          summary: 'Browser preview finished a static index build.',
          queuedAt: new Date().toISOString(),
          availableAt: new Date().toISOString(),
          startedAt: new Date().toISOString(),
          finishedAt: new Date().toISOString(),
          heartbeatAt: new Date().toISOString(),
          errorCode: null,
          errorMessage: null,
        },
        ...mockState.queueJobs,
      ]
      mockState.snapshot.aiStatus = {
        ...mockState.snapshot.aiStatus,
        enabled: true,
        assistantEnabled: true,
        state: 'ready',
        ready: true,
        indexedItems: 2,
        lastIndexedAt: new Date().toISOString(),
        embeddingProviderId: 'mock-embedding',
        semanticSidecarBytes: 196_608,
        semanticMetadataBytes: 24_576,
        estimatedEmbeddingTokens: 1_024,
      }
      syncMockAiStatus(mockState)
      return {
        jobId: buildJobId,
        runId: 31,
        providerId: 'mock-embedding',
        model: 'text-embedding-3-large',
        indexedItems: 2,
        updatedItems: 0,
        skippedItems: 0,
        removedItems: 0,
        lastIndexedAt: new Date().toISOString(),
        notes: ['Browser preview mode uses a static AI index fixture.'],
      } as T
    }
    case 'search_ai_history':
      return paginateMockAiSearch(
        mockState,
        args?.request as AiSearchRequest | undefined,
      ) as T
    case 'ask_ai_assistant': {
      const assistantJobId = mockState.nextAiJobId++
      mockState.queueJobs = [
        {
          id: assistantJobId,
          jobType: 'assistant',
          state: 'succeeded',
          priority: 100,
          attempt: 1,
          maxAttempts: 1,
          runId: 32,
          summary: 'Browser preview answered a static assistant request.',
          queuedAt: new Date().toISOString(),
          availableAt: new Date().toISOString(),
          startedAt: new Date().toISOString(),
          finishedAt: new Date().toISOString(),
          heartbeatAt: new Date().toISOString(),
          errorCode: null,
          errorMessage: null,
        },
        ...mockState.queueJobs,
      ]
      syncMockAiStatus(mockState)
      return {
        state: 'completed',
        answer:
          'Browser preview mode can show the assistant layout, but real LLM answers only run in the desktop app.',
        jobId: assistantJobId,
        runId: 32,
        providerId: 'preview-llm',
        embeddingProviderId: 'lexical-fallback',
        citations: mockState.history.items.map((item) => ({
          historyId: item.id,
          profileId: item.profileId,
          url: item.url,
          title: item.title,
          visitedAt: item.visitedAt,
          score: 0.8,
        })),
        notes: ['Open the desktop build to run real agentic history analysis.'],
      } as T
    }
    case 'load_ai_assistant_job':
      return {
        state: 'completed',
        answer:
          'Browser preview mode loads a deterministic queued assistant reply.',
        jobId: args?.jobId as number,
        runId: 32,
        providerId: 'preview-llm',
        embeddingProviderId: 'lexical-fallback',
        citations: mockState.history.items.map((item) => ({
          historyId: item.id,
          profileId: item.profileId,
          url: item.url,
          title: item.title,
          visitedAt: item.visitedAt,
          score: 0.78,
        })),
        notes: [
          'Queued assistant replies use preview fixtures in browser mode.',
        ],
      } as T
    case 'preview_ai_integrations':
      return {
        mcpCommand: '/Applications/PathKeep.app --worker mcp-server',
        consentSummary:
          'External AI integrations stay local-first and only start after the user enables them in Settings.',
        manualSteps: [
          'Enable MCP or Skill integration in Settings first.',
          'Store the database key in the native keyring if the archive is encrypted.',
          'Copy the generated MCP JSON into your MCP client configuration.',
        ],
        capabilityNotes: [
          'MCP server toggle is currently disabled in saved Settings.',
          'Skill integration toggle is currently disabled in saved Settings.',
          'No embedding provider is selected right now, so external tools fall back to lexical recall only.',
        ],
        scopeBoundary: [
          'Only visible archive facts are returned to external tools.',
          'If App Lock re-locks the session, MCP search returns a locked refusal.',
        ],
        auditTrace: [
          'Each MCP search writes a dedicated run-ledger entry.',
          'Assistant and semantic-index work keep distinct run types.',
        ],
        generatedFiles: [
          {
            relativePath: 'integrations/pathkeep-mcp.json',
            absolutePath:
              '~/Library/Application Support/PathKeep/integrations/pathkeep-mcp.json',
            purpose: 'PathKeep MCP client snippet',
            contents: '{\n  "mcpServers": {}\n}',
          },
          {
            relativePath: 'integrations/codex-pathkeep-skill/SKILL.md',
            absolutePath:
              '~/Library/Application Support/PathKeep/integrations/codex-pathkeep-skill/SKILL.md',
            purpose: 'Codex skill starter',
            contents: '# PathKeep Search\n',
          },
        ],
        warnings: [],
      } as T
    case 'export_history': {
      const exportRequest = args?.request as ExportRequest | undefined
      const exportedItems = filterMockHistory(mockState, {
        ...exportRequest?.query,
        page: null,
        cursor: null,
        limit: Math.max(1, mockState.history.items.length),
      }).items

      return {
        format: exportRequest?.format ?? 'jsonl',
        path: `/tmp/pathkeep-export-${new Date()
          .toISOString()
          .replaceAll(
            ':',
            '-',
          )}.${(exportRequest?.format ?? 'jsonl').replace('markdown', 'md')}`,
        count: exportedItems.length,
      } as T
    }
    case 'apply_schedule':
      return {
        applied: false,
        platform: 'macos',
        files: [],
        message: 'Apply is not available in browser preview mode.',
      } as T
    case 'remove_schedule':
      return {
        applied: false,
        platform: 'macos',
        files: [],
        message: 'Remove is not available in browser preview mode.',
      } as T
    case 'clear_derived_intelligence':
      return clearDerivedIntelligenceFixture(mockState) as T
    // --- Core Intelligence read surfaces ---
    // Browser preview has no real deterministic pipeline behind these queries,
    // so return empty/neutral payloads and let the Intelligence UI fall back to
    // its empty states instead of blowing up in unit tests.
    case 'get_on_this_day':
      return [] as T
    case 'get_top_sites':
    case 'get_refind_pages':
    case 'get_search_engine_ranking':
    case 'get_top_search_concepts':
    case 'get_stable_sources':
    case 'get_friction_signals':
    case 'get_reopened_investigations':
    case 'get_habit_patterns':
    case 'get_interrupted_habits':
    case 'get_path_flows':
    case 'get_compare_sets':
    case 'get_observed_interactions':
    case 'get_hub_pages':
      return [] as T
    case 'list_search_engine_rules':
      return structuredClone(mockState.searchEngineRules) as T
    case 'upsert_search_engine_rule': {
      const input = (args?.input as SearchEngineRuleInput | undefined) ?? null
      if (!input) {
        return structuredClone(mockState.searchEngineRules) as T
      }
      const ruleId =
        input.ruleId?.trim() ||
        `custom:${input.engineId || 'engine'}:${mockState.searchEngineRules.length + 1}`
      const nextRule: SearchEngineRule = {
        ruleId,
        engineId: input.engineId,
        displayName: input.displayName,
        hostPattern: input.hostPattern,
        pathPrefix: input.pathPrefix ?? null,
        queryParamKey: input.queryParamKey,
        enabled: input.enabled,
        note: input.note ?? null,
        exampleUrl: input.exampleUrl ?? null,
        builtIn: false,
      }
      mockState.searchEngineRules = [
        ...mockState.searchEngineRules.filter((rule) => rule.ruleId !== ruleId),
        nextRule,
      ]
      return structuredClone(mockState.searchEngineRules) as T
    }
    case 'delete_search_engine_rule': {
      const ruleId =
        args &&
        typeof args === 'object' &&
        'ruleId' in args &&
        typeof args.ruleId === 'string'
          ? args.ruleId
          : ''
      mockState.searchEngineRules = mockState.searchEngineRules.filter(
        (rule) => rule.ruleId !== ruleId || rule.builtIn,
      )
      return structuredClone(mockState.searchEngineRules) as T
    }
    case 'get_digest_summary':
      return {
        dateRange: { start: '', end: '' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      } as T
    case 'get_day_insights':
      return {
        date: '',
        digestSummary: {
          dateRange: { start: '', end: '' },
          totalVisits: { value: 0, trend: 'flat' },
          totalSearches: { value: 0, trend: 'flat' },
          newDomains: { value: 0, trend: 'flat' },
          deepReadPages: { value: 0, trend: 'flat' },
          refindPages: { value: 0, trend: 'flat' },
        },
        topSites: [],
        activityMix: { categories: [], changeVsPrevious: [] },
        refindPages: [],
        queryFamilies: {
          families: [],
          total: 0,
          page: 0,
          pageSize: 8,
        },
        hourlyActivity: Array.from({ length: 24 }, (_, hour) => ({
          hour,
          visitCount: 0,
        })),
        drilldown: {
          explorerDateRange: { start: '', end: '' },
        },
      } as T
    case 'get_intelligence_primary_overview':
      throw new Error(
        'PathKeep intelligence overview batching is unavailable in browser preview mode.',
      )
    case 'get_intelligence_secondary_overview':
      throw new Error(
        'PathKeep intelligence overview batching is unavailable in browser preview mode.',
      )
    case 'get_activity_mix':
      return { categories: [], changeVsPrevious: [] } as T
    case 'get_activity_mix_trend':
      return { points: [] } as T
    case 'get_discovery_trend':
      return { points: [], availableYears: [] } as T
    case 'get_browsing_rhythm':
      return { cells: [], maxCount: 0 } as T
    case 'get_breadth_index':
      return { hhi: 0, breadthScore: 0, concentrationDomainCount: 0 } as T
    case 'get_multi_browser_diff':
      return {
        profiles: [],
        exclusiveDomains: [],
        sharedDomains: [],
        categoryDistributions: [],
      } as T
    case 'get_sessions':
    case 'get_search_trails':
    case 'get_query_families':
      return {
        sessions: [],
        trails: [],
        families: [],
        total: 0,
        page: 0,
        pageSize: 20,
      } as T
    case 'get_search_queries':
      return buildMockSearchQueries(
        (args?.request as Parameters<typeof buildMockSearchQueries>[0]) ??
          undefined,
      ) as T
    case 'get_query_family_detail':
      return {
        data: {
          family: {
            familyId: '',
            anchorQuery: '',
            memberCount: 0,
            searchEngine: '',
            queries: [],
            firstSeenAt: '',
            lastSeenAt: '',
          },
          relatedTrails: [],
        },
        meta: {
          sectionId: 'query-family-detail',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '', end: '' },
          },
          moduleIds: [],
          sourceTables: [],
          includesEnrichment: false,
          state: 'degraded',
          stateReason: null,
          notes: [],
        },
      } as T
    case 'get_search_effectiveness':
      return {
        engineStats: [],
        topResolvingSources: [],
        hardestTopics: [],
      } as T
    case 'get_domain_trend':
      return { registrableDomain: '', points: [] } as T
    case 'get_domain_deep_dive':
      return {
        registrableDomain: '',
        displayName: null,
        domainCategory: 'unknown',
        totalVisits: 0,
        activeDays: 0,
        trailCount: 0,
        arrivalBreakdown: { search: 0, link: 0, typed: 0, other: 0 },
        topPages: [],
        topReferrers: [],
        topExits: [],
        visitTrend: [],
      } as T
    case 'get_refind_page_detail':
      return {
        data: {
          page: {
            canonicalUrl: '',
            url: '',
            title: null,
            registrableDomain: '',
            crossDayCount: 0,
            trailCount: 0,
            searchArrivalCount: 0,
            typedRevisitCount: 0,
            refindScore: 0,
            firstSeenAt: '',
            lastSeenAt: '',
          },
          explanation: {
            canonicalUrl: '',
            refindScore: 0,
            factors: [],
            visitIds: [],
          },
          recentDays: [],
          relatedTrails: [],
        },
        meta: {
          sectionId: 'refind-page-detail',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '', end: '' },
          },
          moduleIds: [],
          sourceTables: [],
          includesEnrichment: false,
          state: 'degraded',
          stateReason: null,
          notes: [],
        },
      } as T
    case 'get_compare_set_detail':
      return {
        data: {
          compareSet: {
            compareSetId: '',
            trailId: '',
            searchQuery: '',
            pageCategory: '',
            pages: [],
          },
          trail: {
            trailId: '',
            sessionId: null,
            initialQuery: '',
            searchEngine: '',
            reformulationCount: 0,
            visitCount: 0,
            landingUrl: null,
            landingDomain: null,
            firstVisitMs: 0,
            lastVisitMs: 0,
            maxDepth: 0,
            queries: [],
          },
          session: null,
          recentDays: [],
        },
        meta: {
          sectionId: 'compare-set-detail',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '', end: '' },
          },
          moduleIds: [],
          sourceTables: [],
          includesEnrichment: false,
          state: 'degraded',
          stateReason: null,
          notes: [],
        },
      } as T
    case 'get_session_detail':
      return { session: null, visits: [], trails: [] } as T
    case 'get_trail_detail':
      return { trail: null, members: [] } as T
    case 'get_navigation_path':
      return { targetVisitId: 0, steps: [] } as T
    case 'explain_refind':
    case 'explain_entity':
      return {
        entityType: 'unknown',
        entityId: '',
        triggerRule: '',
        factors: [],
        participatingVisitIds: [],
      } as T
    case 'preview_intelligence_local_host':
    case 'build_intelligence_local_host': {
      const request = (args?.request as
        | {
            dateRange?: { start?: string; end?: string }
            profileId?: string | null
            locale?: string
          }
        | undefined) ?? {
        dateRange: { start: '', end: '' },
        profileId: null,
        locale: 'en',
      }
      const artifactRoot =
        '/tmp/pathkeep-preview/integrations/core-intelligence/browser-snippet-v1'
      const bundle = {
        bundleVersion: 'pathkeep.core-intelligence.local-host.v1',
        hostId: 'browser-snippet-v1',
        generatedAt: new Date().toISOString(),
        locale: request.locale ?? 'en',
        dateRange: {
          start: request.dateRange?.start ?? '',
          end: request.dateRange?.end ?? '',
        },
        profileId: request.profileId ?? null,
        embedCards: [
          {
            cardId: 'digest:visits',
            cardType: 'digest',
            title: 'Visits',
            eyebrow: `${request.dateRange?.start ?? ''} → ${
              request.dateRange?.end ?? ''
            }`,
            body: 'Preview fixture for the trusted local snippet host.',
            metricLabel: 'visit_count',
            metricValue: '42',
            href: null,
            internalOnly: false,
          },
        ],
        widgetSnapshot: {
          generatedAt: new Date().toISOString(),
          dateRange: {
            start: request.dateRange?.start ?? '',
            end: request.dateRange?.end ?? '',
          },
          digestSummary: {
            dateRange: {
              start: request.dateRange?.start ?? '',
              end: request.dateRange?.end ?? '',
            },
            totalVisits: { value: 42, trend: 'flat' },
            totalSearches: { value: 7, trend: 'flat' },
            newDomains: { value: 3, trend: 'flat' },
            deepReadPages: { value: 2, trend: 'flat' },
            refindPages: { value: 1, trend: 'flat' },
          },
          highlights: [],
          notes: ['Preview fixture for browser-only mode.'],
        },
        publicSnapshot: {
          generatedAt: new Date().toISOString(),
          dateRange: {
            start: request.dateRange?.start ?? '',
            end: request.dateRange?.end ?? '',
          },
          digestSummary: {
            dateRange: {
              start: request.dateRange?.start ?? '',
              end: request.dateRange?.end ?? '',
            },
            totalVisits: { value: 42, trend: 'flat' },
            totalSearches: { value: 7, trend: 'flat' },
            newDomains: { value: 3, trend: 'flat' },
            deepReadPages: { value: 2, trend: 'flat' },
            refindPages: { value: 1, trend: 'flat' },
          },
          topDomains: ['example.com'],
          searchEngines: [],
          discoveryTrend: { points: [], availableYears: [] },
          notes: ['Preview fixture for browser-only mode.'],
        },
        trustedOnlyCardIds: [],
        trustedOnlyCardCount: 0,
        boundaryNotes: [
          'Browser preview mode only simulates the trusted local host contract.',
        ],
      }
      const response = {
        artifactRoot,
        entryFilePath: `${artifactRoot}/index.html`,
        generatedFiles: [
          {
            relativePath:
              'integrations/core-intelligence/browser-snippet-v1/index.html',
            absolutePath: `${artifactRoot}/index.html`,
            purpose: 'Preview local browser snippet.',
            contents: '<!doctype html><title>PathKeep Preview</title>',
          },
          {
            relativePath:
              'integrations/core-intelligence/browser-snippet-v1/bundle.json',
            absolutePath: `${artifactRoot}/bundle.json`,
            purpose: 'Preview local browser snippet bundle.',
            contents: JSON.stringify(bundle, null, 2),
          },
        ],
        bundle,
        boundaryNotes: bundle.boundaryNotes,
        manualSteps: [
          'Review the generated files in Settings.',
          'Open the local snippet after creating it in the desktop build.',
        ],
        warnings: [],
        installedHost:
          command === 'build_intelligence_local_host'
            ? {
                artifactRoot,
                entryFilePath: `${artifactRoot}/index.html`,
                bundle,
              }
            : null,
      }
      return response as T
    }
    case 'run_core_intelligence_now':
    case 'queue_core_intelligence_rebuild': {
      const jobId = Date.now()
      return {
        jobId,
        state: mockState.snapshot.config.ai.jobQueuePaused
          ? 'queued'
          : 'running',
        notes: [
          mockState.snapshot.config.ai.jobQueuePaused
            ? `Queued Core Intelligence rebuild job ${jobId}. Resume background work to process it.`
            : `Queued Core Intelligence rebuild job ${jobId}. PathKeep is processing it in the background.`,
        ],
      } as T
    }
    default:
      throw new Error(`Mock backend does not implement ${command}`)
  }
}

/**
 * Exposes test-only hooks for mutating and resetting the browser-preview backend fixture state.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
export const backendTestHarness = {
  call,
  reset: () => {
    mockState = createMockState()
  },
  mutateState: (mutator: (state: MockBackendState) => void) => {
    mutator(mockState)
    mockState.snapshot.config = normalizeMockConfig(
      mockState.snapshot.config,
      mockState.s3Credentials,
    )
    syncMockAppLockState(mockState)
    syncMockAiStatus(mockState)
    syncMockIntelligenceRuntime(mockState)
  },
  seedSchedule: (plan: SchedulePlan, status?: ScheduleStatus) => {
    overrideMockSchedule(mockState, plan, status)
  },
}

/**
 * Exposes the browser-preview compatibility facade consumed by preview-mode routes,
 * tests, and the `backend-client/shared` fallback path.
 *
 * This is still a live compatibility surface while browser-preview exists; it is not a
 * fake legacy stub that can drift away from current frontend contracts.
 */
export const backend = {
  getAppBuildInfo: () => call<AppBuildInfo>('app_build_info'),
  loadAppLockStatus: () => call<AppLockStatus>('app_lock_status'),
  getAppSnapshot: () => call<AppSnapshot>('app_snapshot'),
  saveConfig: (config: AppConfig) =>
    call<AppSnapshot>('save_config', { config }),
  initializeArchive: (config: AppConfig, databaseKey?: string | null) =>
    call<AppSnapshot>('initialize_archive', { config, databaseKey }),
  rekeyArchive: (request: RekeyRequest) =>
    call<AppSnapshot>('rekey_archive', { request }),
  previewRekeyArchive: (request: RekeyRequest) =>
    call<RekeyPreview>('preview_rekey_archive', { request }),
  previewSnapshotRestore: (request: SnapshotRestoreRequest) =>
    call<SnapshotRestorePreview>('preview_snapshot_restore', { request }),
  runSnapshotRestore: (request: SnapshotRestoreRequest) =>
    call<BackupReport>('run_snapshot_restore', { request }),
  previewRetentionPrune: () =>
    call<RetentionPreview>('preview_retention_prune'),
  runRetentionPrune: (request: RetentionPruneRequest) =>
    call<RetentionPruneResult>('run_retention_prune', { request }),
  setSessionDatabaseKey: (databaseKey: string) =>
    call<void>('set_session_database_key', { databaseKey }),
  clearSessionDatabaseKey: () => call<void>('clear_session_database_key'),
  setAppLockPasscode: (request: SetAppLockPasscodeRequest) =>
    call<AppLockStatus>('set_app_lock_passcode', { request }),
  clearAppLockPasscode: () => call<AppLockStatus>('clear_app_lock_passcode'),
  lockAppSession: (reason?: string | null) =>
    call<AppLockStatus>('lock_app_session', { reason }),
  unlockAppSession: (request: UnlockAppSessionRequest) =>
    call<AppLockStatus>('unlock_app_session', { request }),
  runBackupNow: (dueOnly = false) =>
    call<BackupReport>('run_backup_now', { dueOnly }),
  queryHistory: (query: HistoryQuery) =>
    call<HistoryQueryResponse>('query_history', { query }),
  loadDashboardSnapshot: () =>
    call<DashboardSnapshot>('load_dashboard_snapshot'),
  loadAuditRunDetail: (runId: number) =>
    call<AuditRunDetail>('load_audit_run_detail', { runId }),
  exportHistory: (request: ExportRequest) =>
    call<ExportResult>('export_history', { request }),
  previewRemoteBackup: () => call<RemoteBackupPreview>('preview_remote_backup'),
  runRemoteBackup: () => call<RemoteBackupResult>('run_remote_backup'),
  verifyRemoteBackup: (bundlePath: string) =>
    call<RemoteBackupVerification>('verify_remote_backup', { bundlePath }),
  inspectTakeout: (request: TakeoutRequest) =>
    call<TakeoutInspection>('inspect_takeout', { request }),
  importTakeout: (request: TakeoutRequest) =>
    call<TakeoutInspection>('import_takeout', { request }),
  previewImportBatch: (batchId: number) =>
    call<ImportBatchDetail>('preview_import_batch', { batchId }),
  revertImportBatch: (batchId: number) =>
    call<ImportBatchDetail>('revert_import_batch', { batchId }),
  restoreImportBatch: (batchId: number) =>
    call<ImportBatchDetail>('restore_import_batch', { batchId }),
  previewSchedule: (platform?: string) =>
    call<SchedulePlan>('preview_schedule', { platform }),
  scheduleStatus: (platform?: string) =>
    call<ScheduleStatus>('schedule_status', { platform }),
  applySchedule: (plan: SchedulePlan) =>
    call<ApplyResult>('apply_schedule', { plan }),
  removeSchedule: (plan: SchedulePlan) =>
    call<ApplyResult>('remove_schedule', { plan }),
  doctor: () => call<HealthReport>('doctor_report'),
  repairHealth: () => call<HealthRepairReport>('repair_health'),
  keyringStatus: () => call<KeyringStatusReport>('keyring_status'),
  securityStatus: () => call<SecurityStatus>('security_status'),
  keyringGetDatabaseKey: () => call<string | null>('keyring_get_database_key'),
  keyringStoreDatabaseKey: (value: string) =>
    call<KeyringStatusReport>('keyring_store_database_key', { value }),
  keyringClearDatabaseKey: () =>
    call<KeyringStatusReport>('keyring_clear_database_key'),
  storeS3Credentials: (credentials: S3CredentialInput) =>
    call<void>('store_s3_credentials', { credentials }),
  clearS3Credentials: () => call<void>('clear_s3_credentials'),
  storeAiProviderApiKey: (input: AiProviderSecretInput) =>
    call<AppSnapshot>('store_ai_provider_api_key', { input }),
  clearAiProviderApiKey: (providerId: string) =>
    call<AppSnapshot>('clear_ai_provider_api_key', { providerId }),
  testAiProviderConnection: (request: AiProviderConnectionTestRequest) =>
    call<AiProviderConnectionTestReport>('test_ai_provider_connection', {
      request,
    }),
  loadAiQueueStatus: () => call<AiQueueStatus>('load_ai_queue_status'),
  runAiQueueJobs: (maxJobs?: number) =>
    call<AiQueueStatus>('run_ai_queue_jobs', { maxJobs }),
  replayAiJob: (jobId: number) => call<AiQueueJob>('replay_ai_job', { jobId }),
  cancelAiJob: (jobId: number) => call<AiQueueJob>('cancel_ai_job', { jobId }),
  buildAiIndex: (request: AiIndexRequest) =>
    call<AiIndexReport>('build_ai_index', { request }),
  searchAiHistory: (request: AiSearchRequest) =>
    call<AiSearchResponse>('search_ai_history', { request }),
  askAiAssistant: (request: AiAssistantRequest) =>
    call<AiAssistantResponse>('ask_ai_assistant', { request }),
  loadAiAssistantJob: (jobId: number) =>
    call<AiAssistantResponse>('load_ai_assistant_job', { jobId }),
  listSearchEngineRules: () =>
    call<SearchEngineRule[]>('list_search_engine_rules'),
  upsertSearchEngineRule: (input: SearchEngineRuleInput) =>
    call<SearchEngineRule[]>('upsert_search_engine_rule', { input }),
  deleteSearchEngineRule: (ruleId: string) =>
    call<SearchEngineRule[]>('delete_search_engine_rule', { ruleId }),
  clearDerivedIntelligence: () =>
    call<ClearDerivedIntelligenceReport>('clear_derived_intelligence'),
  loadIntelligenceRuntime: () =>
    call<IntelligenceRuntimeSnapshot>('load_intelligence_runtime'),
  retryIntelligenceJob: (jobId: number) =>
    call<IntelligenceRuntimeSnapshot>('retry_intelligence_job', { jobId }),
  cancelIntelligenceJob: (jobId: number) =>
    call<IntelligenceRuntimeSnapshot>('cancel_intelligence_job', { jobId }),
  previewAiIntegrations: () =>
    call<AiIntegrationPreview>('preview_ai_integrations'),
  resetLocalSecretVault: () => call<void>('reset_local_secret_vault'),
  openPathInFileManager: (path: string) =>
    call<string>('open_path_in_file_manager', { path }),
  openExternalUrl: (url: string) => call<string>('open_external_url', { url }),
  checkForAppUpdate: () => call<AppUpdateCheckResult>('check_for_app_update'),
  downloadAndInstallAppUpdate: (expectedVersion?: string | null) =>
    call<UpdateInstallState>('download_and_install_app_update', {
      request: { expectedVersion: expectedVersion ?? null },
    }),
  relaunchAfterUpdate: () => call<boolean>('relaunch_after_update'),
}
