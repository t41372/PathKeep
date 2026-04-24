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
import { handlePreviewAiCommand } from './backend-preview-ai-commands'
import { isPreviewCommandHandled } from './backend-preview-command-result'
import { handlePreviewIntelligenceCommand } from './backend-preview-intelligence-commands'
import { handlePreviewShellCommand } from './backend-preview-shell-commands'
import {
  createMockState,
  ensureMockUnlocked,
  type MockBackendState,
  normalizeMockConfig,
  syncMockAiStatus,
  syncMockAppLockState,
  syncMockIntelligenceRuntime,
} from './backend-preview-state'
import { overrideMockSchedule } from './backend-preview-schedule'
import { handlePreviewWorkflowCommand } from './backend-preview-workflow-commands'
import type {
  AiAssistantRequest,
  AiAssistantResponse,
  AiIndexReport,
  AiIndexRequest,
  AiIntegrationPreview,
  AiProviderConnectionTestRequest,
  AiProviderConnectionTestReport,
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
  BrowserHistoryImportRequest,
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

  const shellResult = handlePreviewShellCommand<T>(command, args, mockState)
  if (isPreviewCommandHandled(shellResult)) {
    return shellResult
  }

  const workflowResult = handlePreviewWorkflowCommand<T>(
    command,
    args,
    mockState,
  )
  if (isPreviewCommandHandled(workflowResult)) {
    return workflowResult
  }

  const aiResult = handlePreviewAiCommand<T>(command, args, mockState)
  if (isPreviewCommandHandled(aiResult)) {
    return aiResult
  }

  const intelligenceResult = handlePreviewIntelligenceCommand<T>(
    command,
    args,
    mockState,
  )
  if (isPreviewCommandHandled(intelligenceResult)) {
    return intelligenceResult
  }

  throw new Error(`Mock backend does not implement ${command}`)
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
  loadHistoryFavicons: (entries: { profileId: string; url: string }[]) =>
    call<
      { profileId: string; url: string; favicon?: { dataUrl: string } | null }[]
    >('load_history_favicons', { entries }),
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
  inspectBrowserHistory: (request: BrowserHistoryImportRequest) =>
    call<TakeoutInspection>('inspect_browser_history', { request }),
  importBrowserHistory: (request: BrowserHistoryImportRequest) =>
    call<TakeoutInspection>('import_browser_history', { request }),
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
