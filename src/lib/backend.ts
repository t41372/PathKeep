import { invoke, isTauri } from '@tauri-apps/api/core'
import type {
  AppConfig,
  AppSnapshot,
  ApplyResult,
  BackupReport,
  ExportRequest,
  ExportResult,
  HealthReport,
  HistoryQuery,
  HistoryQueryResponse,
  ImportBatchDetail,
  KeyringStatusReport,
  RekeyRequest,
  RemoteBackupPreview,
  RemoteBackupResult,
  SchedulePlan,
  S3CredentialInput,
  TakeoutInspection,
  TakeoutRequest,
} from './types'

// Stryker disable all: browser-preview fixtures are static reference data, not behavior.
const mockSnapshot: AppSnapshot = {
  directories: {
    appRoot: '~/Library/Application Support/Browser History Backup',
    configPath:
      '~/Library/Application Support/Browser History Backup/config.json',
    archiveDatabasePath:
      '~/Library/Application Support/Browser History Backup/archive/history-vault.sqlite',
    auditRepoPath: '~/Library/Application Support/Browser History Backup/audit',
    manifestsDir:
      '~/Library/Application Support/Browser History Backup/audit/manifests',
    exportsDir: '~/Library/Application Support/Browser History Backup/exports',
    rawSnapshotsDir:
      '~/Library/Application Support/Browser History Backup/raw-snapshots',
    stagingDir: '~/Library/Application Support/Browser History Backup/staging',
    quarantineDir:
      '~/Library/Application Support/Browser History Backup/quarantine',
    scheduleDir:
      '~/Library/Application Support/Browser History Backup/schedule',
    strongholdPath:
      '~/Library/Application Support/Browser History Backup/vault.hold',
    strongholdSaltPath:
      '~/Library/Application Support/Browser History Backup/stronghold-salt.txt',
  },
  config: {
    initialized: false,
    archiveMode: 'Encrypted',
    preferredLanguage: 'system',
    dueAfterHours: 72,
    scheduleCheckIntervalHours: 6,
    checkpointDays: 90,
    captureFavicons: true,
    selectedProfileIds: [],
    gitEnabled: true,
    rememberDatabaseKeyInKeyring: false,
    appAutostart: false,
    remoteBackup: {
      enabled: false,
      bucket: '',
      region: 'us-east-1',
      endpoint: null,
      prefix: 'browser-history-backup',
      pathStyle: true,
      uploadAfterBackup: false,
      credentialsSaved: false,
      lastUploadedAt: null,
      lastUploadedObjectKey: null,
      lastError: null,
    },
  },
  archiveStatus: {
    initialized: false,
    encrypted: true,
    unlocked: false,
    databasePath:
      '~/Library/Application Support/Browser History Backup/archive/history-vault.sqlite',
  },
  keyringStatus: {
    available: true,
    backend: 'Mock keyring',
    storedSecret: false,
  },
  browserProfiles: [],
  recentRuns: [],
  recentImportBatches: [],
}

const mockHistory: HistoryQueryResponse = {
  total: 2,
  items: [
    {
      id: 1,
      profileId: 'chrome:Default',
      url: 'https://developer.chrome.com/docs/devtools/storage/sqlite',
      title: 'SQLite inspection in browser developer tools',
      domain: 'developer.chrome.com',
      visitedAt: new Date().toISOString(),
      visitTime: Date.now(),
      durationMs: 24000,
      transition: 805306368,
      sourceVisitId: 1,
      appId: null,
    },
    {
      id: 2,
      profileId: 'chrome:Default',
      url: 'https://chromium.googlesource.com/chromium/src/+/main/components/history/core/browser/history_database.cc',
      title: 'Chromium history schema',
      domain: 'chromium.googlesource.com',
      visitedAt: new Date(Date.now() - 3_600_000).toISOString(),
      visitTime: Date.now() - 3_600_000,
      durationMs: 18000,
      transition: 805306368,
      sourceVisitId: 2,
      appId: null,
    },
  ],
}
// Stryker restore all

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (isTauri()) {
    return invoke<T>(command, args)
  }

  switch (command) {
    case 'app_snapshot':
    case 'save_config':
    case 'initialize_archive':
    case 'rekey_archive':
      return structuredClone(mockSnapshot) as T
    case 'set_session_database_key':
    case 'clear_session_database_key':
    case 'reset_local_secret_vault':
      return undefined as T
    case 'run_backup_now':
      return {
        dueSkipped: false,
        profiles: [],
        warnings: [],
        remoteBackup: null,
      } as T
    case 'query_history':
      return mockHistory as T
    case 'inspect_takeout':
    case 'import_takeout':
      return {
        sourcePath: args?.request
          ? String((args.request as TakeoutRequest).sourcePath)
          : '/tmp/takeout.zip',
        dryRun: true,
        recognizedFiles: [],
        quarantinedFiles: [],
        candidateItems: 0,
        importedItems: 0,
        duplicateItems: 0,
        previewEntries: [],
        importBatch: null,
        notes: ['Tauri is not available in browser preview mode.'],
      } as T
    case 'preview_import_batch':
    case 'revert_import_batch':
      return {
        batch: {
          id: 1,
          sourceKind: 'takeout',
          sourcePath: '/tmp/takeout.zip',
          profileId: 'takeout::browser-history',
          createdAt: new Date().toISOString(),
          importedAt: new Date().toISOString(),
          revertedAt:
            command === 'revert_import_batch' ? new Date().toISOString() : null,
          status: command === 'revert_import_batch' ? 'reverted' : 'imported',
          candidateItems: 0,
          importedItems: 0,
          duplicateItems: 0,
          visibleItems: command === 'revert_import_batch' ? 0 : 0,
          auditPath: null,
          gitCommit: null,
        },
        previewEntries: [],
        recognizedFiles: [],
        quarantinedFiles: [],
        notes: [
          'Desktop-only import batch preview is unavailable in browser preview mode.',
        ],
      } as T
    case 'preview_schedule':
      return {
        platform: 'macos',
        label: 'dev.codex.browser-history-backup.backup',
        executablePath: '/Applications/Browser History Backup.app',
        generatedFiles: [],
        manualSteps: ['Tauri is not available in browser preview mode.'],
        applyCommands: [],
        rollbackCommands: [],
        applySupported: false,
      } as T
    case 'doctor_report':
      return {
        generatedAt: new Date().toISOString(),
        checks: [],
      } as T
    case 'preview_remote_backup':
      return {
        bundlePath: '/tmp/browser-history-backup-remote.zip',
        objectKey: 'browser-history-backup/browser-history-backup-remote.zip',
        uploadUrl:
          'https://s3.us-east-1.amazonaws.com/example-bucket/browser-history-backup/browser-history-backup-remote.zip',
        previewCommand:
          'curl --fail --show-error --aws-sigv4 "aws:amz:us-east-1:s3" --user "$S3_ACCESS_KEY_ID:$S3_SECRET_ACCESS_KEY" -T \'/tmp/browser-history-backup-remote.zip\' \'https://s3.us-east-1.amazonaws.com/example-bucket/browser-history-backup/browser-history-backup-remote.zip\'',
        manualSteps: ['Browser preview mode cannot generate the real bundle.'],
        warnings: [],
      } as T
    case 'run_remote_backup':
      return {
        uploaded: false,
        bundlePath: '/tmp/browser-history-backup-remote.zip',
        objectKey: 'browser-history-backup/browser-history-backup-remote.zip',
        uploadUrl:
          'https://s3.us-east-1.amazonaws.com/example-bucket/browser-history-backup/browser-history-backup-remote.zip',
        message: 'Remote backup upload is only available in the desktop app.',
      } as T
    case 'keyring_status':
      return mockSnapshot.keyringStatus as T
    case 'keyring_get_database_key':
      return null as T
    case 'keyring_store_database_key':
    case 'keyring_clear_database_key':
    case 'store_s3_credentials':
    case 'clear_s3_credentials':
      return {
        available: true,
        backend: 'Mock keyring',
        storedSecret: command === 'keyring_store_database_key',
      } as T
    case 'export_history':
      return {
        format: 'jsonl',
        path: '/tmp/history-export.jsonl',
        count: mockHistory.items.length,
      } as T
    case 'apply_schedule':
      return {
        applied: false,
        platform: 'macos',
        files: [],
        message: 'Apply is not available in browser preview mode.',
      } as T
    default:
      throw new Error(`Mock backend does not implement ${command}`)
  }
}

export const backendTestHarness = {
  call,
}

export const backend = {
  getAppSnapshot: () => call<AppSnapshot>('app_snapshot'),
  saveConfig: (config: AppConfig) =>
    call<AppSnapshot>('save_config', { config }),
  initializeArchive: (config: AppConfig, databaseKey?: string | null) =>
    call<AppSnapshot>('initialize_archive', { config, databaseKey }),
  rekeyArchive: (request: RekeyRequest) =>
    call<AppSnapshot>('rekey_archive', { request }),
  setSessionDatabaseKey: (databaseKey: string) =>
    call<void>('set_session_database_key', { databaseKey }),
  clearSessionDatabaseKey: () => call<void>('clear_session_database_key'),
  runBackupNow: (dueOnly = false) =>
    call<BackupReport>('run_backup_now', { dueOnly }),
  queryHistory: (query: HistoryQuery) =>
    call<HistoryQueryResponse>('query_history', { query }),
  exportHistory: (request: ExportRequest) =>
    call<ExportResult>('export_history', { request }),
  previewRemoteBackup: () => call<RemoteBackupPreview>('preview_remote_backup'),
  runRemoteBackup: () => call<RemoteBackupResult>('run_remote_backup'),
  inspectTakeout: (request: TakeoutRequest) =>
    call<TakeoutInspection>('inspect_takeout', { request }),
  importTakeout: (request: TakeoutRequest) =>
    call<TakeoutInspection>('import_takeout', { request }),
  previewImportBatch: (batchId: number) =>
    call<ImportBatchDetail>('preview_import_batch', { batchId }),
  revertImportBatch: (batchId: number) =>
    call<ImportBatchDetail>('revert_import_batch', { batchId }),
  previewSchedule: (platform?: string) =>
    call<SchedulePlan>('preview_schedule', { platform }),
  applySchedule: (plan: SchedulePlan) =>
    call<ApplyResult>('apply_schedule', { plan }),
  doctor: () => call<HealthReport>('doctor_report'),
  keyringStatus: () => call<KeyringStatusReport>('keyring_status'),
  keyringGetDatabaseKey: () => call<string | null>('keyring_get_database_key'),
  keyringStoreDatabaseKey: (value: string) =>
    call<KeyringStatusReport>('keyring_store_database_key', { value }),
  keyringClearDatabaseKey: () =>
    call<KeyringStatusReport>('keyring_clear_database_key'),
  storeS3Credentials: (credentials: S3CredentialInput) =>
    call<void>('store_s3_credentials', { credentials }),
  clearS3Credentials: () => call<void>('clear_s3_credentials'),
  resetLocalSecretVault: () => call<void>('reset_local_secret_vault'),
}
