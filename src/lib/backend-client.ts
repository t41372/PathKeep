import { invoke, isTauri } from '@tauri-apps/api/core'
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
  ExplainInsightRequest,
  ExportRequest,
  ExportResult,
  HealthRepairReport,
  HealthReport,
  HistoryQuery,
  HistoryQueryResponse,
  ImportBatchDetail,
  InsightExplanation,
  InsightSnapshot,
  InsightThreadDetail,
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
  RunInsightsReport,
  RunInsightsRequest,
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

type BackendArgs = Record<string, unknown> | undefined

// Keep the browser-preview fixture surface out of the live app import graph.
async function call<T>(command: string, args?: BackendArgs): Promise<T> {
  if (isTauri()) {
    return invoke<T>(command, args)
  }

  const { backendTestHarness } = await import('./backend')
  return backendTestHarness.call<T>(command, args)
}

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
  runInsightsNow: (request: RunInsightsRequest) =>
    call<RunInsightsReport>('run_insights_now', { request }),
  clearDerivedIntelligence: () =>
    call<ClearDerivedIntelligenceReport>('clear_derived_intelligence'),
  loadInsights: (request: RunInsightsRequest) =>
    call<InsightSnapshot>('load_insights', { request }),
  loadThreadDetail: (threadId: string) =>
    call<InsightThreadDetail>('load_thread_detail', { threadId }),
  explainInsight: (request: ExplainInsightRequest) =>
    call<InsightExplanation>('explain_insight', { request }),
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
