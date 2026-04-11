import type {
  ArchiveMode,
  ArchiveStatus,
  BackupRunOverview,
  BrowserProfile,
} from './archive'
import type { ImportBatchOverview } from './import'
import type {
  AiIndexStatus,
  AiSettings,
  DeterministicSettings,
  EnrichmentSettings,
  InsightStatus,
} from './intelligence'
import type { RemoteBackupConfig } from './remote'
import type { KeyringStatusReport } from './security'

export type LanguagePreference = 'system' | 'en' | 'zh-CN' | 'zh-TW'
export interface AppLockConfig {
  enabled: boolean
  idleTimeoutMinutes: number
  biometricEnabled: boolean
  passcodeEnabled: boolean
  passcodeConfigured: boolean
  recoveryHint?: string | null
}

export type AppLockBiometricState =
  | 'touch-id-available'
  | 'touch-id-unavailable'
  | 'unsupported'

export interface AppLockStatus {
  enabled: boolean
  locked: boolean
  idleTimeoutMinutes: number
  biometricAvailable: boolean
  biometricEnabled: boolean
  biometricState: AppLockBiometricState
  passcodeEnabled: boolean
  passcodeConfigured: boolean
  configPath: string
  lockReason?: string | null
  lockedAt?: string | null
  lastUnlockedAt?: string | null
  recoveryHint?: string | null
  warnings: string[]
  degradationNotes: string[]
}

export interface UnlockAppSessionRequest {
  passcode?: string | null
  useBiometric?: boolean
}

export interface SetAppLockPasscodeRequest {
  passcode: string
  recoveryHint?: string | null
}

export interface AnalyticsConfig {
  enabled: boolean
  consentGrantedAt?: string | null
}

export interface AppConfig {
  initialized: boolean
  archiveMode: ArchiveMode
  preferredLanguage: LanguagePreference
  dueAfterHours: number
  scheduleCheckIntervalHours: number
  checkpointDays: number
  captureFavicons: boolean
  selectedProfileIds: string[]
  gitEnabled: boolean
  rememberDatabaseKeyInKeyring: boolean
  appAutostart: boolean
  appLock: AppLockConfig
  analytics: AnalyticsConfig
  remoteBackup: RemoteBackupConfig
  enrichment: EnrichmentSettings
  deterministic: DeterministicSettings
  ai: AiSettings
}

export interface UpdateAvailability {
  supported: boolean
  checkedAt: string
  available: boolean
  currentVersion?: string | null
  version?: string | null
  notes?: string | null
  publishedAt?: string | null
  error?: string | null
  downloadUrl?: string | null
}

export type UpdateInstallPhase =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'installing'
  | 'installed'
  | 'uptodate'
  | 'error'
  | 'unsupported'

export interface UpdateInstallState {
  phase: UpdateInstallPhase
  version?: string | null
  downloadedBytes?: number | null
  contentLength?: number | null
  message?: string | null
}

export interface PendingAppUpdate {
  currentVersion?: string | null
  version: string
  notes?: string | null
  publishedAt?: string | null
  downloadUrl?: string | null
}

export interface AppUpdateCheckResult {
  availability: UpdateAvailability
  pendingUpdate: PendingAppUpdate | null
}

export type AnalyticsEvent =
  | {
      type: 'route-view'
      route: string
      screen: string
      language: LanguagePreference
    }
  | {
      type: 'cta-click'
      screen: string
      action: string
      feature: string
    }
  | {
      type: 'update-lifecycle'
      screen: string
      action: string
      status: string
      version?: string | null
    }

export interface AppDirectories {
  appRoot: string
  configPath: string
  archiveDatabasePath: string
  auditRepoPath: string
  manifestsDir: string
  exportsDir: string
  rawSnapshotsDir: string
  stagingDir: string
  quarantineDir: string
  scheduleDir: string
  logsDir: string
  rustLogPath: string
  frontendLogPath: string
  crashReportsDir: string
  strongholdPath: string
  strongholdSaltPath: string
}

export interface AppBuildInfo {
  productName: string
  version: string
  gitCommitShort: string
  gitCommitFull: string
  gitDirty: boolean
}

export interface CrashReportSummary {
  source: string
  recordedAt: string
  fatal: boolean
  message: string
  location?: string | null
  path: string
}

export interface RuntimeDiagnostics {
  logDirectory: string
  rustLogPath: string
  frontendLogPath: string
  crashReportsDirectory: string
  latestCrashReport?: CrashReportSummary | null
}

export interface AppSnapshot {
  directories: AppDirectories
  runtimeDiagnostics: RuntimeDiagnostics
  config: AppConfig
  archiveStatus: ArchiveStatus
  appLockStatus: AppLockStatus
  keyringStatus: KeyringStatusReport
  aiStatus: AiIndexStatus
  insightStatus: InsightStatus
  browserProfiles: BrowserProfile[]
  recentRuns: BackupRunOverview[]
  recentImportBatches: ImportBatchOverview[]
}
