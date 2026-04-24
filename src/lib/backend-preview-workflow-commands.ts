/**
 * @file backend-preview-workflow-commands.ts
 * @description Browser-preview workflow and trust-review command owner for archive maintenance, import review, schedule review, and remote backup flows.
 * @module lib/backend-preview-workflow-commands
 *
 * ## Responsibilities
 * - Handle preview commands for rekey, snapshot restore, retention prune, import batch review, schedule review, remote backup, and security/keyring helpers.
 * - Keep browser-preview workflow mutations aligned with the extracted preview helper modules instead of re-embedding workflow logic in `backend.ts`.
 * - Return truthful manual-review payloads for commands that desktop can execute but browser-preview cannot.
 *
 * ## Not responsible for
 * - Shell/bootstrap reads such as app snapshot, dashboard snapshot, or archive export.
 * - AI queue/assistant behavior or intelligence read-surface preview fallbacks.
 * - Deciding whether desktop transport is active; `backend.ts` still owns transport selection.
 *
 * ## Dependencies
 * - Depends on the shared preview command sentinel contract from `./backend-preview-command-result`.
 * - Reuses canonical preview workflow/search/support helpers from the extracted `backend-preview-*` modules.
 *
 * ## Performance notes
 * - These handlers run against bounded in-memory fixture state, so they avoid whole-state cloning and only materialize the response payloads each workflow needs.
 */

import {
  PREVIEW_COMMAND_UNHANDLED,
  type PreviewCommandResult,
} from './backend-preview-command-result'
import type { MockBackendState } from './backend-preview-state'
import { normalizeMockConfig } from './backend-preview-state'
import {
  buildMockSchedulePlan,
  buildMockScheduleStatus,
  normalizeMockPlatform,
} from './backend-preview-schedule'
import {
  prependMockRun,
  previewRemoteBackupFixture,
  verifyRemoteBackupFixture,
} from './backend-preview-support'
import {
  buildMockRekeyPreview,
  buildMockRetentionPreview,
  buildMockSecurityStatus,
  buildMockSnapshotRestorePreview,
  buildMockBrowserHistoryInspection,
  buildMockTakeoutInspection,
  mutateImportBatch,
} from './backend-preview-workflows'
import type {
  BrowserHistoryImportRequest,
  RekeyRequest,
  RetentionPruneRequest,
  S3CredentialInput,
  SnapshotRestoreRequest,
  TakeoutRequest,
} from './types'

/**
 * Routes workflow/trust-review preview commands that mutate archive maintenance state or expose manual-review workflow payloads.
 *
 * This isolates the review-heavy command surface so `backend.ts` can focus on top-level delegation instead of
 * staying responsible for every workflow case directly.
 */
export function handlePreviewWorkflowCommand<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  state: MockBackendState,
): PreviewCommandResult<T> {
  switch (command) {
    case 'rekey_archive': {
      const request = args?.request as RekeyRequest
      const finishedAt = new Date().toISOString()
      const run = prependMockRun(state, {
        id: (state.snapshot.recentRuns[0]?.id ?? 1847) + 1,
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
      state.snapshot.config.archiveMode = request.newMode
      state.snapshot.archiveStatus.encrypted = request.newMode === 'Encrypted'
      state.snapshot.archiveStatus.unlocked =
        request.newMode === 'Plaintext' ||
        Boolean(request.newKey && request.newKey.trim())
      void run
      return structuredClone(state.snapshot) as T
    }
    case 'preview_rekey_archive':
      return buildMockRekeyPreview(
        state,
        structuredClone(args?.request as RekeyRequest),
      ) as T
    case 'preview_snapshot_restore':
      return buildMockSnapshotRestorePreview(
        state,
        structuredClone(args?.request as SnapshotRestoreRequest),
      ) as T
    case 'run_snapshot_restore': {
      const request = structuredClone(
        args?.request as SnapshotRestoreRequest,
      ) ?? {
        snapshotPath: `${state.snapshot.directories.rawSnapshotsDir}/run-1`,
      }
      const preview = buildMockSnapshotRestorePreview(state, request)
      if (!preview.executeSupported) {
        throw new Error(
          'Automatic restore is only supported for saved browser source checkpoints right now.',
        )
      }
      const profileId = preview.sourceProfileId!
      const finishedAt = new Date().toISOString()
      const run = prependMockRun(state, {
        id: (state.snapshot.recentRuns[0]?.id ?? 1847) + 1,
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
        manifestPath: `${state.snapshot.directories.manifestsDir}/2026-04-09/run-${run.id}-snapshot-restore.json`,
        gitCommit: null,
        warnings: [],
        remoteBackup: null,
      } as T
    }
    case 'preview_retention_prune':
      return buildMockRetentionPreview(state) as T
    case 'run_retention_prune': {
      const request = args?.request as RetentionPruneRequest | undefined
      const preview = buildMockRetentionPreview(state)
      const selected = preview.buckets.filter((bucket) =>
        request?.bucketIds?.includes(bucket.id),
      )
      const finishedAt = new Date().toISOString()
      const run = prependMockRun(state, {
        id: (state.snapshot.recentRuns[0]?.id ?? 1847) + 1,
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
    case 'inspect_takeout':
      return buildMockTakeoutInspection(
        state,
        args?.request
          ? String((args.request as TakeoutRequest).sourcePath)
          : '/tmp/takeout.zip',
        true,
      ) as T
    case 'import_takeout':
      return buildMockTakeoutInspection(
        state,
        args?.request
          ? String((args.request as TakeoutRequest).sourcePath)
          : '/tmp/takeout.zip',
        false,
      ) as T
    case 'inspect_browser_history':
      return buildMockBrowserHistoryInspection(
        state,
        (args?.request as BrowserHistoryImportRequest | undefined) ?? {
          sourcePath: '/tmp/History',
          dryRun: true,
        },
        true,
      ) as T
    case 'import_browser_history':
      return buildMockBrowserHistoryInspection(
        state,
        (args?.request as BrowserHistoryImportRequest | undefined) ?? {
          sourcePath: '/tmp/History',
          dryRun: false,
        },
        false,
      ) as T
    case 'preview_import_batch': {
      const batchId = Number(args?.batchId ?? 0)
      if (state.importBatchDetails[batchId]) {
        return structuredClone(state.importBatchDetails[batchId]) as T
      }
      buildMockTakeoutInspection(state, '/tmp/takeout.zip', false)
      return structuredClone(state.importBatchDetails[1]) as T
    }
    case 'revert_import_batch':
      return mutateImportBatch(state, Number(args?.batchId ?? 1), 'revert') as T
    case 'restore_import_batch':
      return mutateImportBatch(
        state,
        Number(args?.batchId ?? 1),
        'restore',
      ) as T
    case 'preview_schedule':
      return (state.schedulePlanOverrides[
        normalizeMockPlatform(args?.platform)
      ] ?? buildMockSchedulePlan(args?.platform)) as T
    case 'schedule_status':
      return (state.scheduleStatusOverrides[
        normalizeMockPlatform(args?.platform)
      ] ?? buildMockScheduleStatus(state, args?.platform)) as T
    case 'doctor_report':
      return {
        generatedAt: new Date().toISOString(),
        checks: [
          {
            name: 'import-artifacts',
            status: state.snapshot.recentImportBatches.length ? 'ok' : 'info',
            message: state.snapshot.recentImportBatches.length
              ? 'Import batch audit artifacts are present and reviewable.'
              : 'No import batches have been created yet.',
          },
          {
            name: 'visibility-state',
            status: state.snapshot.recentImportBatches.some(
              (batch) => batch.status === 'reverted',
            )
              ? 'warning'
              : 'ok',
            message: state.snapshot.recentImportBatches.some(
              (batch) => batch.status === 'reverted',
            )
              ? 'One or more batches are reverted. Verify downstream read models after restore.'
              : 'Visible import rows match the current batch state.',
          },
        ],
      } as T
    case 'repair_health': {
      const repairRun = prependMockRun(state, {
        id: (state.snapshot.recentRuns[0]?.id ?? 1) + 1,
        startedAt: new Date().toISOString(),
        finishedAt: new Date().toISOString(),
        status: 'success',
        runType: 'doctor',
        trigger: 'manual',
        profileScope: [],
        manifestHash: null,
        profilesProcessed: 0,
        newVisits: state.snapshot.recentImportBatches.some(
          (batch) => batch.status === 'reverted',
        )
          ? 1
          : 0,
        newUrls: 0,
        newDownloads: 0,
      })
      return {
        runId: repairRun.id,
        repairedImportAudits: state.snapshot.recentImportBatches.length ? 1 : 0,
        repairedVisibilityRows: state.snapshot.recentImportBatches.some(
          (batch) => batch.status === 'reverted',
        )
          ? 1
          : 0,
        clearedDerivedRows: state.snapshot.recentImportBatches.length ? 2 : 0,
        notes: ['Browser preview mode simulates a targeted doctor repair run.'],
      } as T
    }
    case 'preview_remote_backup':
      return previewRemoteBackupFixture(state) as T
    case 'run_remote_backup': {
      const preview = previewRemoteBackupFixture(state)
      const uploaded = Boolean(state.s3Credentials)
      const finishedAt = new Date().toISOString()
      state.snapshot.config.remoteBackup.lastError = uploaded
        ? null
        : 'Store S3 credentials before executing the remote backup.'
      if (uploaded) {
        state.snapshot.config.remoteBackup.lastUploadedAt = finishedAt
        state.snapshot.config.remoteBackup.lastUploadedObjectKey =
          preview.objectKey
      }
      state.snapshot.config = normalizeMockConfig(
        state.snapshot.config,
        state.s3Credentials,
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
        state,
        typeof args?.bundlePath === 'string' ? args.bundlePath : undefined,
      ) as T
    case 'keyring_status':
      return structuredClone(state.snapshot.keyringStatus) as T
    case 'security_status':
      return buildMockSecurityStatus(state) as T
    case 'keyring_get_database_key':
      return state.keyringSecret as T
    case 'keyring_store_database_key':
      state.keyringSecret =
        typeof args?.value === 'string' ? args.value : state.keyringSecret
      state.snapshot.keyringStatus.storedSecret = Boolean(state.keyringSecret)
      return structuredClone(state.snapshot.keyringStatus) as T
    case 'keyring_clear_database_key':
      state.keyringSecret = null
      state.snapshot.keyringStatus.storedSecret = false
      return structuredClone(state.snapshot.keyringStatus) as T
    case 'store_s3_credentials':
      state.s3Credentials = structuredClone(
        args?.credentials as S3CredentialInput,
      )
      state.snapshot.config = normalizeMockConfig(
        state.snapshot.config,
        state.s3Credentials,
      )
      return undefined as T
    case 'clear_s3_credentials':
      state.s3Credentials = null
      state.snapshot.config.remoteBackup.lastError = null
      state.snapshot.config = normalizeMockConfig(
        state.snapshot.config,
        state.s3Credentials,
      )
      return undefined as T
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
    default:
      return PREVIEW_COMMAND_UNHANDLED
  }
}
