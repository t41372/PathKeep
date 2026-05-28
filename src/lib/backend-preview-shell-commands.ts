/**
 * @file backend-preview-shell-commands.ts
 * @description Browser-preview shell and bootstrap command owner for the compatibility backend facade.
 * @module lib/backend-preview-shell-commands
 *
 * ## Responsibilities
 * - Handle preview commands that mutate app/bootstrap state, shell lock state, and top-level archive session state.
 * - Build deterministic shell-facing payloads such as app snapshots, dashboard reads, history queries, and export results.
 * - Keep browser-preview update/open-path helpers truthful without leaking those cases back into `backend.ts`.
 *
 * ## Not responsible for
 * - AI queue, assistant, or intelligence read-surface preview behavior.
 * - Remote backup, import review, schedule review, or archive security workflow commands.
 * - Choosing when preview transport is active; `backend.ts` still owns the top-level transport decision.
 *
 * ## Dependencies
 * - Depends on the shared preview command sentinel contract from `./backend-preview-command-result`.
 * - Reuses canonical preview state sync helpers and read-model helpers from the extracted `backend-preview-*` modules.
 *
 * ## Performance notes
 * - These handlers run on hot shell reads, so they stay synchronous, bounded to the in-memory fixture surface, and avoid extra cloning beyond response payloads.
 */

import { mockBuildInfo } from './backend-preview-fixtures'
import {
  PREVIEW_COMMAND_UNHANDLED,
  type PreviewCommandResult,
} from './backend-preview-command-result'
import type { MockBackendState } from './backend-preview-state'
import {
  normalizeMockConfig,
  syncMockAiStatus,
  syncMockAppLockState,
  validateMockAppLockConfig,
} from './backend-preview-state'
import {
  filterMockHistory,
  loadMockHistoryFavicons,
  uniqueUrlCount,
} from './backend-preview-search'
import {
  buildMockAuditRunDetail,
  buildMockDashboardSnapshot,
  prependMockRun,
} from './backend-preview-support'
import type {
  AppConfig,
  ExportRequest,
  HistoryQuery,
  SetAppLockPasscodeRequest,
  UnlockAppSessionRequest,
} from './types'

/**
 * Routes the shell/bootstrap preview commands that still belong to one shared browser-preview owner.
 *
 * The main dispatcher now delegates to this module first so shell state, lock state, dashboard reads, and
 * export behavior can evolve together without forcing `backend.ts` to stay a thousand-line switch.
 */
export function handlePreviewShellCommand<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  state: MockBackendState,
): PreviewCommandResult<T> {
  switch (command) {
    case 'app_build_info':
      return mockBuildInfo as T
    case 'app_lock_status':
      return structuredClone(state.snapshot.appLockStatus) as T
    case 'app_snapshot':
      syncMockAiStatus(state)
      return structuredClone(state.snapshot) as T
    case 'save_config': {
      const nextConfig = normalizeMockConfig(
        structuredClone(args?.config as AppConfig),
      )
      validateMockAppLockConfig(state, nextConfig)
      state.snapshot.config = nextConfig
      state.snapshot.archiveStatus.encrypted =
        nextConfig.archiveMode === 'Encrypted'
      syncMockAppLockState(state)
      syncMockAiStatus(state)
      return structuredClone(state.snapshot) as T
    }
    case 'initialize_archive': {
      const nextConfig = normalizeMockConfig(
        structuredClone(args?.config as AppConfig),
      )
      validateMockAppLockConfig(state, nextConfig)
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
      state.snapshot.config = nextConfig
      state.snapshot.archiveStatus = {
        ...state.snapshot.archiveStatus,
        initialized: true,
        encrypted: nextConfig.archiveMode === 'Encrypted',
        unlocked:
          nextConfig.archiveMode === 'Plaintext' ||
          Boolean(databaseKey && databaseKey.trim()),
        warning: null,
      }
      syncMockAppLockState(state)
      return structuredClone(state.snapshot) as T
    }
    case 'set_app_lock_passcode': {
      const request = args?.request as SetAppLockPasscodeRequest | undefined
      const passcode = request?.passcode?.trim()
      if (!passcode || passcode.length < 4) {
        throw new Error(
          'App lock passcodes must be at least 4 characters long.',
        )
      }
      state.appLockPasscode = passcode
      state.appLockRecoveryHint = request?.recoveryHint?.trim() || null
      syncMockAppLockState(state)
      return structuredClone(state.snapshot.appLockStatus) as T
    }
    case 'clear_app_lock_passcode':
      state.appLockPasscode = null
      state.appLockRecoveryHint = null
      state.snapshot.config.appLock.enabled = false
      state.snapshot.appLockStatus = {
        ...state.snapshot.appLockStatus,
        locked: false,
        lockReason: null,
        lockedAt: null,
      }
      syncMockAppLockState(state)
      return structuredClone(state.snapshot.appLockStatus) as T
    case 'lock_app_session':
      if (state.snapshot.config.appLock.enabled) {
        state.snapshot.appLockStatus = {
          ...state.snapshot.appLockStatus,
          locked: true,
          lockReason:
            typeof args?.reason === 'string' && args.reason.trim()
              ? args.reason
              : 'manual',
          lockedAt: new Date().toISOString(),
        }
      }
      syncMockAppLockState(state)
      return structuredClone(state.snapshot.appLockStatus) as T
    case 'unlock_app_session': {
      const request = args?.request as UnlockAppSessionRequest | undefined
      if (request?.useBiometric) {
        if (!state.snapshot.config.appLock.biometricEnabled) {
          throw new Error(
            'Biometric unlock is currently turned off in Settings.',
          )
        }
        if (state.biometricState !== 'touch-id-available') {
          throw new Error(
            state.biometricState === 'touch-id-unavailable'
              ? 'Touch ID is unavailable on this Mac right now. Use the app lock passcode instead.'
              : 'Biometric unlock is not available in the current desktop build.',
          )
        }
      }
      if (state.snapshot.config.appLock.enabled) {
        if (
          !request?.useBiometric &&
          (request?.passcode?.trim() ?? '') !== state.appLockPasscode
        ) {
          if (!state.snapshot.config.appLock.passcodeEnabled) {
            throw new Error(
              'PathKeep cannot unlock without an enabled app lock credential.',
            )
          }
          throw new Error('The app lock passcode did not match.')
        }
        state.snapshot.appLockStatus = {
          ...state.snapshot.appLockStatus,
          locked: false,
          lockReason: null,
          lockedAt: null,
          lastUnlockedAt: new Date().toISOString(),
        }
      }
      syncMockAppLockState(state)
      return structuredClone(state.snapshot.appLockStatus) as T
    }
    case 'set_session_database_key':
      state.snapshot.archiveStatus.unlocked = true
      return undefined as T
    case 'clear_session_database_key':
      state.snapshot.archiveStatus.unlocked =
        state.snapshot.config.archiveMode === 'Plaintext'
      return undefined as T
    case 'reset_local_secret_vault':
      return undefined as T
    case 'open_path_in_file_manager':
      return (
        typeof args?.path === 'string'
          ? args.path
          : state.snapshot.directories.appRoot
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
          downloadUrl: 'https://github.com/t41372/PathKeep/releases',
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
      if (!state.snapshot.config.initialized) {
        throw new Error('Initialize the archive before running a backup.')
      }
      if (state.snapshot.config.selectedProfileIds.length === 0) {
        throw new Error('Select at least one profile before running a backup.')
      }
      const finishedAt = new Date().toISOString()
      const nextRunId = (state.snapshot.recentRuns[0]?.id ?? 1847) + 1
      const run = {
        id: nextRunId,
        startedAt: finishedAt,
        finishedAt,
        status: 'success',
        runType: 'backup',
        trigger: 'manual',
        profileScope: state.snapshot.config.selectedProfileIds,
        manifestHash: `preview-manifest-${nextRunId}`,
        profilesProcessed: state.snapshot.config.selectedProfileIds.filter(
          (profileId) => profileId.startsWith('chrome:'),
        ).length,
        newVisits: state.history.items.length,
        newUrls: uniqueUrlCount(state.history.items),
        newDownloads: 1,
      }
      prependMockRun(state, run)
      state.snapshot.archiveStatus.initialized = true
      state.snapshot.archiveStatus.unlocked = true
      state.snapshot.archiveStatus.lastSuccessfulBackupAt = finishedAt
      return {
        dueSkipped: false,
        run,
        profiles: state.snapshot.config.selectedProfileIds
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
      } as T
    }
    case 'query_history':
      return filterMockHistory(state, args?.query as HistoryQuery) as T
    case 'load_history_favicons':
      return loadMockHistoryFavicons(
        state,
        (args?.entries as
          | { profileId: string; url: string; visitTime: number }[]
          | undefined) ?? [],
      ) as T
    case 'load_history_og_images':
      return ((args?.entries as { url: string }[] | undefined) ?? []).map(
        (entry) => ({
          url: entry.url,
          ogImage: null,
          fetchStatus: 'pending',
        }),
      ) as T
    case 'mark_og_images_shown':
      return undefined as T
    case 'trigger_og_image_refetch':
      return 0 as T
    case 'get_og_image_storage_stats':
      return {
        rowCount: 0,
        blobCount: 0,
        totalBytes: 0,
        oldestFetchedAt: null,
      } as T
    case 'clear_og_image_cache':
    case 'run_og_image_cleanup':
      return { deletedRows: 0, deletedBlobs: 0, reclaimedBytes: 0 } as T
    case 'load_dashboard_snapshot':
      return buildMockDashboardSnapshot(state) as T
    case 'load_audit_run_detail':
      return buildMockAuditRunDetail(
        state,
        Number(args?.runId ?? state.snapshot.recentRuns[0]?.id ?? 1848),
      ) as T
    case 'export_history': {
      const exportRequest = args?.request as ExportRequest | undefined
      const exportedItems = filterMockHistory(state, {
        ...exportRequest?.query,
        page: null,
        cursor: null,
        limit: Math.max(1, state.history.items.length),
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
    default:
      return PREVIEW_COMMAND_UNHANDLED
  }
}
