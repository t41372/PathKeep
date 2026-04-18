/**
 * This module defines typed front-end contracts for app bootstrap, configuration, diagnostics, and updater state.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - `LanguagePreference`
 * - `AppLockConfig`
 * - `AppLockBiometricState`
 * - `AppLockStatus`
 * - `UnlockAppSessionRequest`
 * - `SetAppLockPasscodeRequest`
 * - `AnalyticsConfig`
 * - `AppConfig`
 * - `UpdateAvailability`
 * - `UpdateInstallPhase`
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

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
  IntelligenceStatus,
} from './intelligence'
import type { RemoteBackupConfig } from './remote'
import type { KeyringStatusReport } from './security'

/**
 * Defines the type-level contract for language preference.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export type LanguagePreference = 'system' | 'en' | 'zh-CN' | 'zh-TW'
/**
 * Represents persisted configuration for app lock.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AppLockConfig {
  enabled: boolean
  idleTimeoutMinutes: number
  biometricEnabled: boolean
  passcodeEnabled: boolean
  passcodeConfigured: boolean
  recoveryHint?: string | null
}

/**
 * Names the allowed states for app lock biometric.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export type AppLockBiometricState =
  | 'touch-id-available'
  | 'touch-id-unavailable'
  | 'unsupported'

/**
 * Represents a read model or status snapshot for app lock.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface UnlockAppSessionRequest {
  passcode?: string | null
  useBiometric?: boolean
}

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface SetAppLockPasscodeRequest {
  passcode: string
  recoveryHint?: string | null
}

/**
 * Represents persisted configuration for analytics.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AnalyticsConfig {
  enabled: boolean
  consentGrantedAt?: string | null
}

/**
 * Represents persisted configuration for app.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Defines the typed shape for update availability.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Defines the type-level contract for update install phase.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Captures the state shape used by `UpdateInstall`.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface UpdateInstallState {
  phase: UpdateInstallPhase
  version?: string | null
  downloadedBytes?: number | null
  contentLength?: number | null
  message?: string | null
}

/**
 * Defines the typed shape for pending app update.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface PendingAppUpdate {
  currentVersion?: string | null
  version: string
  notes?: string | null
  publishedAt?: string | null
  downloadUrl?: string | null
}

/**
 * Defines the typed shape for app update check result.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AppUpdateCheckResult {
  availability: UpdateAvailability
  pendingUpdate: PendingAppUpdate | null
}

/**
 * Defines the type-level contract for analytics event.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
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

/**
 * Defines the typed shape for app directories.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AppDirectories {
  appRoot: string
  configPath: string
  archiveDatabasePath: string
  searchDatabasePath: string
  intelligenceDatabasePath: string
  auditRepoPath: string
  manifestsDir: string
  exportsDir: string
  rawSnapshotsDir: string
  stagingDir: string
  quarantineDir: string
  scheduleDir: string
  semanticIndexDir: string
  intelligenceBlobsDir: string
  logsDir: string
  rustLogPath: string
  frontendLogPath: string
  crashReportsDir: string
  strongholdPath: string
  strongholdSaltPath: string
}

/**
 * Defines the typed shape for app build info.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AppBuildInfo {
  productName: string
  version: string
  gitCommitShort: string
  gitCommitFull: string
  gitDirty: boolean
}

/**
 * Represents a condensed summary for crash report.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface CrashReportSummary {
  source: string
  recordedAt: string
  fatal: boolean
  message: string
  location?: string | null
  path: string
}

/**
 * Defines the typed shape for runtime diagnostics.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface RuntimeDiagnostics {
  logDirectory: string
  rustLogPath: string
  frontendLogPath: string
  crashReportsDirectory: string
  latestCrashReport?: CrashReportSummary | null
}

/**
 * Defines the typed shape for app snapshot.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AppSnapshot {
  directories: AppDirectories
  runtimeDiagnostics: RuntimeDiagnostics
  config: AppConfig
  archiveStatus: ArchiveStatus
  appLockStatus: AppLockStatus
  keyringStatus: KeyringStatusReport
  aiStatus: AiIndexStatus
  intelligenceStatus: IntelligenceStatus
  browserProfiles: BrowserProfile[]
  recentRuns: BackupRunOverview[]
  recentImportBatches: ImportBatchOverview[]
}
