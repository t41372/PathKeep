/**
 * This module is the legacy preview-aware backend facade and deterministic browser-preview fixture surface used by older front-end consumers and tests.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `backendTestHarness`
 * - `backend`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { invoke, isTauri } from '@tauri-apps/api/core'
import {
  defaultEnrichmentSettings,
  resolveEnrichmentSettings,
  READABLE_CONTENT_REFETCH_PLUGIN_ID,
} from './enrichment'
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
  BackupRunOverview,
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
  ImportBatchOverview,
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

// Stryker disable all: browser-preview fixtures are static reference data, not behavior.
const mockBuildInfo: AppBuildInfo = {
  productName: 'PathKeep',
  version: '0.1.0',
  gitCommitShort: 'preview',
  gitCommitFull: 'preview-build',
  gitDirty: true,
}

const mockSnapshot: AppSnapshot = {
  directories: {
    appRoot: '~/Library/Application Support/com.yi-ting.pathkeep',
    configPath:
      '~/Library/Application Support/com.yi-ting.pathkeep/config.json',
    archiveDatabasePath:
      '~/Library/Application Support/com.yi-ting.pathkeep/archive/history-vault.sqlite',
    searchDatabasePath:
      '~/Library/Application Support/com.yi-ting.pathkeep/derived/history-search.sqlite',
    intelligenceDatabasePath:
      '~/Library/Application Support/com.yi-ting.pathkeep/derived/history-intelligence.sqlite',
    auditRepoPath: '~/Library/Application Support/com.yi-ting.pathkeep/audit',
    manifestsDir:
      '~/Library/Application Support/com.yi-ting.pathkeep/audit/manifests',
    exportsDir: '~/Library/Application Support/com.yi-ting.pathkeep/exports',
    rawSnapshotsDir:
      '~/Library/Application Support/com.yi-ting.pathkeep/raw-snapshots',
    stagingDir: '~/Library/Application Support/com.yi-ting.pathkeep/staging',
    quarantineDir:
      '~/Library/Application Support/com.yi-ting.pathkeep/quarantine',
    scheduleDir: '~/Library/Application Support/com.yi-ting.pathkeep/schedule',
    semanticIndexDir:
      '~/Library/Application Support/com.yi-ting.pathkeep/sidecars/semantic-index',
    intelligenceBlobsDir:
      '~/Library/Application Support/com.yi-ting.pathkeep/sidecars/intelligence-blobs',
    logsDir: '~/Library/Application Support/com.yi-ting.pathkeep/logs',
    rustLogPath:
      '~/Library/Application Support/com.yi-ting.pathkeep/logs/rust.log',
    frontendLogPath:
      '~/Library/Application Support/com.yi-ting.pathkeep/logs/frontend.log',
    crashReportsDir:
      '~/Library/Application Support/com.yi-ting.pathkeep/diagnostics/crash-reports',
    strongholdPath:
      '~/Library/Application Support/com.yi-ting.pathkeep/vault.hold',
    strongholdSaltPath:
      '~/Library/Application Support/com.yi-ting.pathkeep/stronghold-salt.txt',
  },
  runtimeDiagnostics: {
    logDirectory: '~/Library/Application Support/com.yi-ting.pathkeep/logs',
    rustLogPath:
      '~/Library/Application Support/com.yi-ting.pathkeep/logs/rust.log',
    frontendLogPath:
      '~/Library/Application Support/com.yi-ting.pathkeep/logs/frontend.log',
    crashReportsDirectory:
      '~/Library/Application Support/com.yi-ting.pathkeep/diagnostics/crash-reports',
    latestCrashReport: null,
  },
  config: {
    initialized: false,
    archiveMode: 'Encrypted',
    preferredLanguage: 'system',
    dueAfterHours: 72,
    scheduleCheckIntervalHours: 6,
    checkpointDays: 90,
    captureFavicons: true,
    selectedProfileIds: [
      'chrome:Default',
      'chrome:Profile 2',
      'safari:default',
    ],
    gitEnabled: true,
    rememberDatabaseKeyInKeyring: false,
    appAutostart: false,
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
    enrichment: defaultEnrichmentSettings(),
    deterministic: {
      modules: [
        { id: 'visit-derived-facts', enabled: true, version: 'ci-v1' },
        { id: 'daily-rollups', enabled: true, version: 'ci-v1' },
        { id: 'sessions', enabled: true, version: 'ci-v1' },
        { id: 'search-trails', enabled: true, version: 'ci-v1' },
        { id: 'refind-pages', enabled: true, version: 'ci-v1' },
        { id: 'activity-mix', enabled: true, version: 'ci-v1' },
        { id: 'search-effectiveness', enabled: true, version: 'ci-v1' },
        { id: 'domain-deep-dive', enabled: true, version: 'ci-v1' },
      ],
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
      enrichmentPlugins: [
        { pluginId: 'title-normalization', enabled: true },
        { pluginId: 'readable-content-refetch', enabled: true },
      ],
      llmProviderId: null,
      embeddingProviderId: null,
      retrievalTopK: 8,
      assistantSystemPrompt:
        'You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.',
      llmProviders: [],
      embeddingProviders: [],
    },
  },
  archiveStatus: {
    initialized: false,
    encrypted: true,
    unlocked: false,
    databasePath:
      '~/Library/Application Support/com.yi-ting.pathkeep/archive/history-vault.sqlite',
  },
  appLockStatus: {
    enabled: false,
    locked: false,
    idleTimeoutMinutes: 5,
    biometricAvailable: false,
    biometricEnabled: false,
    biometricState: 'unsupported',
    passcodeEnabled: true,
    passcodeConfigured: false,
    configPath:
      '~/Library/Application Support/com.yi-ting.pathkeep/config.json',
    lockReason: null,
    lockedAt: null,
    lastUnlockedAt: null,
    recoveryHint: null,
    warnings: [],
    degradationNotes: [],
  },
  keyringStatus: {
    available: true,
    backend: 'Mock keyring',
    storedSecret: false,
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
    ready: true,
    lastRunAt: new Date().toISOString(),
    runs: 4,
    cards: 4,
    topics: 3,
    threads: 2,
    queryGroups: 2,
    referencePages: 2,
    contentCoverage: 0.64,
    warning: null,
  },
  browserProfiles: [
    {
      profileId: 'chrome:Default',
      profileName: 'Primary',
      browserFamily: 'chromium',
      browserName: 'Google Chrome',
      userName: 'primary@example.test',
      profilePath: '~/Library/Application Support/Google/Chrome/Default',
      historyPath:
        '~/Library/Application Support/Google/Chrome/Default/History',
      faviconsPath:
        '~/Library/Application Support/Google/Chrome/Default/Favicons',
      historyExists: true,
      browserVersion: '146.0.7680.178',
      historyFileName: 'History',
      historyBytes: 58 * 1024 * 1024,
      faviconsBytes: 14 * 1024 * 1024,
      supportingBytes: 6 * 1024 * 1024,
      retentionBoundary: {
        kind: 'browser-managed',
        localDays: null,
      },
    },
    {
      profileId: 'chrome:Profile 2',
      profileName: 'Research',
      browserFamily: 'chromium',
      browserName: 'Google Chrome',
      userName: 'research@example.test',
      profilePath: '~/Library/Application Support/Google/Chrome/Profile 2',
      historyPath:
        '~/Library/Application Support/Google/Chrome/Profile 2/History',
      faviconsPath:
        '~/Library/Application Support/Google/Chrome/Profile 2/Favicons',
      historyExists: true,
      browserVersion: '146.0.7680.178',
      historyFileName: 'History',
      historyBytes: 312 * 1024 * 1024,
      faviconsBytes: 20 * 1024 * 1024,
      supportingBytes: 14 * 1024 * 1024,
      retentionBoundary: {
        kind: 'browser-managed',
        localDays: null,
      },
    },
    {
      profileId: 'chrome:Profile 5',
      profileName: 'Archive',
      browserFamily: 'chromium',
      browserName: 'Google Chrome',
      userName: 'archive@example.test',
      profilePath: '~/Library/Application Support/Google/Chrome/Profile 5',
      historyPath:
        '~/Library/Application Support/Google/Chrome/Profile 5/History',
      faviconsPath:
        '~/Library/Application Support/Google/Chrome/Profile 5/Favicons',
      historyExists: true,
      browserVersion: '146.0.7680.178',
      historyFileName: 'History',
      historyBytes: 24 * 1024 * 1024,
      faviconsBytes: 8 * 1024 * 1024,
      supportingBytes: 3 * 1024 * 1024,
      retentionBoundary: {
        kind: 'browser-managed',
        localDays: null,
      },
    },
    {
      profileId: 'safari:default',
      profileName: 'Safari',
      browserFamily: 'safari',
      browserName: 'Safari',
      userName: null,
      profilePath: '~/Library/Safari',
      historyPath: '~/Library/Safari/History.db',
      faviconsPath: null,
      historyExists: true,
      browserVersion: '18.4',
      historyFileName: 'History.db',
      historyBytes: 18 * 1024 * 1024,
      faviconsBytes: 0,
      supportingBytes: 2 * 1024 * 1024,
      retentionBoundary: {
        kind: 'macos-safari',
        localDays: 365,
      },
    },
  ],
  recentRuns: [],
  recentImportBatches: [],
}

const mockHistory: HistoryQueryResponse = {
  total: 2,
  page: 1,
  pageSize: 50,
  pageCount: 1,
  hasPrevious: false,
  hasNext: false,
  items: [
    {
      id: 1,
      profileId: 'chrome:Default',
      url: 'https://developer.chrome.com/docs/devtools/storage/sqlite',
      title: 'SQLite inspection in browser developer tools',
      domain: 'developer.chrome.com',
      favicon: {
        dataUrl:
          'data:image/svg+xml;utf8,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 32 32%22%3E%3Crect width=%2232%22 height=%2232%22 rx=%228%22 fill=%22%230f172a%22/%3E%3Ccircle cx=%2216%22 cy=%2216%22 r=%229%22 fill=%22%2338bdf8%22/%3E%3C/svg%3E',
      },
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
      favicon: {
        dataUrl:
          'data:image/svg+xml;utf8,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 32 32%22%3E%3Crect width=%2232%22 height=%2232%22 rx=%228%22 fill=%22%23111827%22/%3E%3Cpath d=%22M16 6a10 10 0 1 0 10 10A10 10 0 0 0 16 6Zm0 3.2a6.8 6.8 0 0 1 5.9 3.4H16Zm-6 6.8a6 6 0 0 1 .1-1.1H16v7a6.8 6.8 0 0 1-6-5.9Zm9.2 5.9V16H23a6.8 6.8 0 0 1-3.8 5.9Z%22 fill=%22%23f59e0b%22/%3E%3C/svg%3E',
      },
      visitedAt: new Date(Date.now() - 3_600_000).toISOString(),
      visitTime: Date.now() - 3_600_000,
      durationMs: 18000,
      transition: 805306368,
      sourceVisitId: 2,
      appId: null,
    },
  ],
  nextCursor: null,
}

const mockIntelligenceRuntime: IntelligenceRuntimeSnapshot = {
  queue: {
    queued: 1,
    running: 0,
    succeeded: 9,
    failed: 1,
    cancelled: 0,
    lastActivityAt: new Date().toISOString(),
  },
  plugins: [
    {
      pluginId: 'title-normalization',
      sourceKind: 'local',
      enabled: true,
      storedRecords: 24,
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      lastCompletedAt: new Date().toISOString(),
      lastError: null,
    },
    {
      pluginId: 'readable-content-refetch',
      sourceKind: 'network',
      enabled: true,
      storedRecords: 8,
      queuedJobs: 1,
      runningJobs: 0,
      failedJobs: 1,
      lastCompletedAt: new Date().toISOString(),
      lastError: '429 from upstream host',
    },
  ],
  modules: [
    {
      moduleId: 'visit-derived-facts',
      enabled: true,
      version: 'ci-v1',
      status: 'ready',
      dependsOn: [],
      derivedTables: ['visit_derived_facts'],
      lastRunId: 12,
      lastBuiltAt: new Date().toISOString(),
      lastInvalidatedAt: null,
      staleReason: null,
      notes: [
        'Normalized visit evidence is up to date for the current archive scope.',
      ],
    },
    {
      moduleId: 'daily-rollups',
      enabled: true,
      version: 'ci-v1',
      status: 'ready',
      dependsOn: ['visit-derived-facts'],
      derivedTables: [
        'domain_daily_rollups',
        'category_daily_rollups',
        'engine_daily_rollups',
        'daily_summary_rollups',
      ],
      lastRunId: 12,
      lastBuiltAt: new Date().toISOString(),
      lastInvalidatedAt: null,
      staleReason: null,
      notes: ['Daily rollups are aligned with the latest visible visits.'],
    },
    {
      moduleId: 'sessions',
      enabled: true,
      version: 'ci-v1',
      status: 'ready',
      dependsOn: ['visit-derived-facts'],
      derivedTables: ['sessions'],
      lastRunId: 12,
      lastBuiltAt: new Date().toISOString(),
      lastInvalidatedAt: null,
      staleReason: null,
      notes: ['Session grouping stayed stable across the latest rebuild.'],
    },
    {
      moduleId: 'search-trails',
      enabled: true,
      version: 'ci-v1',
      status: 'ready',
      dependsOn: ['visit-derived-facts', 'sessions'],
      derivedTables: [
        'search_trails',
        'search_trail_members',
        'search_events',
        'search_event_terms',
        'query_families',
      ],
      lastRunId: 12,
      lastBuiltAt: new Date().toISOString(),
      lastInvalidatedAt: null,
      staleReason: null,
      notes: [
        'Search trails and query families reflect the latest normalized visits.',
      ],
    },
    {
      moduleId: 'refind-pages',
      enabled: true,
      version: 'ci-v1',
      status: 'stale',
      dependsOn: ['visit-derived-facts', 'search-trails'],
      derivedTables: ['refind_pages', 'source_effectiveness'],
      lastRunId: 11,
      lastBuiltAt: new Date(Date.now() - 86_400_000).toISOString(),
      lastInvalidatedAt: new Date().toISOString(),
      staleReason: 'Visibility changed after the last deterministic rebuild.',
      notes: [
        'Manual rebuild required before refind pages and source effectiveness are fresh again.',
      ],
    },
    {
      moduleId: 'activity-mix',
      enabled: true,
      version: 'ci-v1',
      status: 'ready',
      dependsOn: ['visit-derived-facts', 'daily-rollups'],
      derivedTables: [],
      lastRunId: 12,
      lastBuiltAt: new Date().toISOString(),
      lastInvalidatedAt: null,
      staleReason: null,
      notes: [
        'Digest and activity mix surfaces are in sync with current rollups.',
      ],
    },
    {
      moduleId: 'search-effectiveness',
      enabled: true,
      version: 'ci-v1',
      status: 'ready',
      dependsOn: ['search-trails', 'refind-pages', 'daily-rollups'],
      derivedTables: ['reopened_investigations'],
      lastRunId: 12,
      lastBuiltAt: new Date().toISOString(),
      lastInvalidatedAt: null,
      staleReason: null,
      notes: [
        'Reopened investigations reflect current search convergence and follow-up paths.',
      ],
    },
    {
      moduleId: 'domain-deep-dive',
      enabled: true,
      version: 'ci-v1',
      status: 'ready',
      dependsOn: ['visit-derived-facts', 'daily-rollups'],
      derivedTables: ['habit_patterns', 'path_flows'],
      lastRunId: 12,
      lastBuiltAt: new Date().toISOString(),
      lastInvalidatedAt: null,
      staleReason: null,
      notes: [
        'Habit and path-flow surfaces were refreshed from the latest deterministic state.',
      ],
    },
  ],
  recentJobs: [
    {
      id: 11,
      jobType: 'enrichment-plugin',
      pluginId: 'readable-content-refetch',
      state: 'failed',
      historyId: 2,
      profileId: 'chrome:Default',
      url: mockHistory.items[1].url,
      title: mockHistory.items[1].title,
      attempt: 2,
      createdAt: new Date(Date.now() - 60_000).toISOString(),
      startedAt: new Date(Date.now() - 55_000).toISOString(),
      finishedAt: new Date(Date.now() - 54_000).toISOString(),
      updatedAt: new Date(Date.now() - 54_000).toISOString(),
      heartbeatAt: null,
      progressLabel: null,
      progressDetail: null,
      progressCurrent: null,
      progressTotal: null,
      progressPercent: null,
      lastError: '429 from upstream host',
      retryable: true,
      cancellable: false,
    },
    {
      id: 12,
      jobType: 'enrichment-plugin',
      pluginId: 'title-normalization',
      state: 'succeeded',
      historyId: 1,
      profileId: 'chrome:Default',
      url: mockHistory.items[0].url,
      title: mockHistory.items[0].title,
      attempt: 1,
      createdAt: new Date(Date.now() - 120_000).toISOString(),
      startedAt: new Date(Date.now() - 118_000).toISOString(),
      finishedAt: new Date(Date.now() - 117_000).toISOString(),
      updatedAt: new Date(Date.now() - 117_000).toISOString(),
      heartbeatAt: null,
      progressLabel: null,
      progressDetail: null,
      progressCurrent: null,
      progressTotal: null,
      progressPercent: null,
      lastError: null,
      retryable: false,
      cancellable: false,
    },
  ],
  notes: ['Browser preview mode shows a deterministic queue/runtime fixture.'],
}

/**
 * Captures the state shape used by `MockBackend`.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
interface MockBackendState {
  snapshot: AppSnapshot
  history: HistoryQueryResponse
  keyringSecret: string | null
  s3Credentials: S3CredentialInput | null
  appLockPasscode: string | null
  appLockRecoveryHint: string | null
  biometricState: AppLockStatus['biometricState']
  importBatchDetails: Record<number, ImportBatchDetail>
  schedulePlanOverrides: Partial<
    Record<'macos' | 'windows' | 'linux', SchedulePlan>
  >
  scheduleStatusOverrides: Partial<
    Record<'macos' | 'windows' | 'linux', ScheduleStatus>
  >
  intelligenceRuntime: IntelligenceRuntimeSnapshot
  queueJobs: AiQueueJob[]
  nextAiJobId: number
  nextImportBatchId: number
  lastRemoteBundlePath: string | null
  derivedStateCleared: boolean
}

/**
 * Explains how browser kind from profile id works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function browserKindFromProfileId(profileId: string) {
  const separatorIndex = profileId.indexOf(':')
  return separatorIndex === -1 ? profileId : profileId.slice(0, separatorIndex)
}

/**
 * Explains how unique url count works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function uniqueUrlCount(items: HistoryQueryResponse['items']) {
  return new Set(items.map((item) => item.url)).size
}

/**
 * Normalizes mock config into the canonical UI shape.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function normalizeMockConfig(
  config: AppConfig,
  s3Credentials: S3CredentialInput | null = null,
): AppConfig {
  return {
    ...config,
    appLock: {
      ...config.appLock,
      idleTimeoutMinutes: Math.min(
        60,
        Math.max(1, config.appLock.idleTimeoutMinutes),
      ),
    },
    enrichment: resolveEnrichmentSettings(config.enrichment),
    remoteBackup: {
      ...config.remoteBackup,
      credentialsSaved: Boolean(s3Credentials),
    },
  }
}

/**
 * Explains how sync mock app lock state works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function syncMockAppLockState(state: MockBackendState) {
  const passcodeConfigured = Boolean(state.appLockPasscode)
  const enabled = state.snapshot.config.appLock.enabled
  const passcodeEnabled = state.snapshot.config.appLock.passcodeEnabled
  const biometricEnabled = state.snapshot.config.appLock.biometricEnabled
  const pendingPasscodeWarning =
    enabled && passcodeEnabled && !passcodeConfigured
  const locked = enabled ? state.snapshot.appLockStatus.locked : false
  const lockReason = locked ? state.snapshot.appLockStatus.lockReason : null
  const lockedAt = locked ? state.snapshot.appLockStatus.lockedAt : null
  const lastUnlockedAt = state.snapshot.appLockStatus.lastUnlockedAt

  state.snapshot.config.appLock = {
    ...state.snapshot.config.appLock,
    passcodeConfigured,
    recoveryHint: state.appLockRecoveryHint,
  }
  state.snapshot.appLockStatus = {
    ...state.snapshot.appLockStatus,
    enabled,
    locked,
    idleTimeoutMinutes: state.snapshot.config.appLock.idleTimeoutMinutes,
    biometricAvailable: state.biometricState === 'touch-id-available',
    biometricEnabled,
    biometricState: state.biometricState,
    passcodeEnabled,
    passcodeConfigured,
    recoveryHint: state.appLockRecoveryHint,
    lockReason,
    lockedAt,
    lastUnlockedAt,
    warnings: pendingPasscodeWarning
      ? ['Set an app lock passcode before relying on session lock.']
      : biometricEnabled && state.biometricState !== 'touch-id-available'
        ? [
            state.biometricState === 'touch-id-unavailable'
              ? 'Touch ID is unavailable on this Mac right now, so PathKeep falls back to the app-lock passcode.'
              : 'Biometric unlock is reserved for future platform integration; this preview falls back to the app-lock passcode.',
          ]
        : [],
    degradationNotes: [
      'App Lock only protects the PathKeep UI session. Archive encryption still protects data at rest.',
      state.biometricState === 'touch-id-available'
        ? 'Touch ID is available on this Mac and can unlock the current PathKeep session.'
        : state.biometricState === 'touch-id-unavailable'
          ? 'Touch ID is unavailable on this Mac right now, so PathKeep falls back to the app-lock passcode.'
          : 'Biometric unlock is reserved for future platform integration; this preview falls back to the app-lock passcode.',
    ],
  }
}

/**
 * Explains how ensure mock unlocked works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function ensureMockUnlocked(command: string, state: MockBackendState) {
  if (!state.snapshot.appLockStatus.locked) {
    return
  }

  if (
    command === 'app_build_info' ||
    command === 'app_lock_status' ||
    command === 'unlock_app_session' ||
    command === 'open_path_in_file_manager' ||
    command === 'open_external_url'
  ) {
    return
  }

  throw new Error(
    'PathKeep is currently locked. Unlock the app before requesting archive data.',
  )
}

/**
 * Explains how validate mock app lock config works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function validateMockAppLockConfig(state: MockBackendState, config: AppConfig) {
  if (!config.appLock.enabled) {
    return
  }

  if (
    config.appLock.biometricEnabled &&
    state.biometricState !== 'touch-id-available'
  ) {
    throw new Error(
      state.biometricState === 'touch-id-unavailable'
        ? 'Touch ID is unavailable on this Mac right now. Use the app lock passcode instead.'
        : 'Biometric unlock is not available in the current desktop build.',
    )
  }

  if (!config.appLock.passcodeEnabled) {
    throw new Error(
      'Enable a passcode before turning on App Lock in this build.',
    )
  }

  if (!state.appLockPasscode) {
    throw new Error('Set an app lock passcode before turning on App Lock.')
  }
}

/**
 * Explains how remote bundle path works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function remoteBundlePath() {
  const timestamp = new Date().toISOString().replaceAll(':', '-')
  return `/tmp/pathkeep-remote-${timestamp}.zip`
}

/**
 * Explains how remote object key works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function remoteObjectKey(config: AppConfig, bundlePath: string) {
  const prefix = config.remoteBackup.prefix.trim().replace(/^\/+|\/+$/g, '')
  const fileName = bundlePath.split('/').pop()!
  return prefix ? `${prefix}/${fileName}` : fileName
}

/**
 * Explains how remote upload url works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function remoteUploadUrl(config: AppConfig, objectKey: string) {
  const trimmedObjectKey = objectKey.replace(/^\/+/, '')
  const endpoint = config.remoteBackup.endpoint?.trim()

  if (endpoint) {
    const normalized =
      endpoint.startsWith('http://') || endpoint.startsWith('https://')
        ? endpoint.replace(/\/+$/g, '')
        : `https://${endpoint.replace(/\/+$/g, '')}`
    if (config.remoteBackup.pathStyle) {
      return `${normalized}/${config.remoteBackup.bucket}/${trimmedObjectKey}`
    }

    const url = new URL(normalized)
    url.hostname = `${config.remoteBackup.bucket}.${url.hostname}`
    return `${url.toString().replace(/\/+$/g, '')}/${trimmedObjectKey}`
  }

  if (config.remoteBackup.pathStyle) {
    return `https://s3.${config.remoteBackup.region}.amazonaws.com/${config.remoteBackup.bucket}/${trimmedObjectKey}`
  }

  return `https://${config.remoteBackup.bucket}.s3.${config.remoteBackup.region}.amazonaws.com/${trimmedObjectKey}`
}

/**
 * Explains how preview remote backup fixture works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function previewRemoteBackupFixture(
  state: MockBackendState,
): RemoteBackupPreview {
  const config = state.snapshot.config
  const bundlePath = remoteBundlePath()
  const objectKey = remoteObjectKey(config, bundlePath)
  const uploadUrl = remoteUploadUrl(config, objectKey)
  const warnings = []

  if (config.archiveMode === 'Plaintext') {
    warnings.push(
      'The remote bundle will contain a plaintext archive because local encryption is currently disabled.',
    )
  }
  if (!state.snapshot.config.remoteBackup.credentialsSaved) {
    warnings.push(
      'Remote credentials are not stored yet. Save the access key and secret before using Execute.',
    )
  }
  if (config.remoteBackup.endpoint) {
    warnings.push(
      'A custom S3-compatible endpoint is configured. Verify TLS, bucket policy, and path-style compatibility before trusting automatic upload.',
    )
  }

  state.lastRemoteBundlePath = bundlePath

  return {
    bundlePath,
    objectKey,
    uploadUrl,
    previewCommand: `curl --fail --show-error --aws-sigv4 "aws:amz:${config.remoteBackup.region}:s3" --user "$S3_ACCESS_KEY_ID:$S3_SECRET_ACCESS_KEY" -T '${bundlePath}' '${uploadUrl}'`,
    manualSteps: [
      'Review the bundle path, object key, and upload URL before you trust the destination.',
      'Store S3 credentials in Settings or copy the preview command into your own terminal session.',
      'After execute finishes, run Verify to confirm checksums and restore readiness on the generated bundle.',
    ],
    warnings,
  }
}

/**
 * Explains how verify remote backup fixture works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function verifyRemoteBackupFixture(
  state: MockBackendState,
  bundlePath?: string,
): RemoteBackupVerification {
  const resolvedBundlePath =
    bundlePath ?? state.lastRemoteBundlePath ?? remoteBundlePath()
  const objectKey = remoteObjectKey(state.snapshot.config, resolvedBundlePath)
  return {
    bundlePath: resolvedBundlePath,
    bundleVersion: 'pathkeep.remote-backup.v1',
    appVersion: mockBuildInfo.version,
    createdAt: new Date().toISOString(),
    archiveMode:
      state.snapshot.config.archiveMode === 'Encrypted'
        ? 'encrypted'
        : 'plaintext',
    objectKey,
    restoreReady: true,
    checks: [
      {
        name: 'bundle-manifest',
        status: 'ok',
        message:
          'Bundle manifest exists and declares a supported PathKeep remote bundle version.',
      },
      {
        name: 'checksums',
        status: 'ok',
        message:
          'Preview verification recalculated bundle checksums and found no drift.',
      },
      {
        name: 'restore-validation',
        status: 'ok',
        message:
          'Required archive/config entries are present, so the bundle is restorable in the desktop app.',
      },
    ],
    warnings:
      state.snapshot.config.archiveMode === 'Plaintext'
        ? [
            'Restore validation passed, but the archive inside this bundle stays plaintext at rest.',
          ]
        : [],
    restoreSteps: [
      'Download the bundle to a local disk before attempting restore.',
      'Verify the manifest and archive entries before replacing a live PathKeep archive.',
      'If the archive is encrypted, unlock PathKeep with the current database key before restore.',
    ],
    manifestFiles: [
      {
        relativePath: 'archive/history-vault.sqlite',
        sha256: 'preview-archive-sha256',
        sizeBytes: 146_800_640,
      },
      {
        relativePath: 'config/config.json',
        sha256: 'preview-config-sha256',
        sizeBytes: 4_096,
      },
      {
        relativePath: 'metadata/bundle-manifest.json',
        sha256: 'preview-manifest-sha256',
        sizeBytes: 1_024,
      },
    ],
  }
}

/**
 * Explains how clear derived intelligence fixture works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function clearDerivedIntelligenceFixture(
  state: MockBackendState,
): ClearDerivedIntelligenceReport {
  state.derivedStateCleared = true
  state.snapshot.intelligenceStatus = {
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
  }
  state.intelligenceRuntime.modules = state.intelligenceRuntime.modules.map(
    (module) => ({
      ...module,
      status: module.enabled ? 'stale' : 'disabled',
      lastInvalidatedAt: new Date().toISOString(),
      staleReason: module.enabled
        ? 'Derived intelligence state was cleared manually.'
        : null,
      notes: module.enabled
        ? [
            'Manual rebuild required before this deterministic module is fresh again.',
          ]
        : ['Disabled in Settings.'],
    }),
  )

  return {
    clearedVisitDerivedFactRows: 8,
    clearedDailyRollupRows: 11,
    clearedStructuralRows: 27,
    clearedRuntimeRows: 12,
    notes: [
      'Only Core Intelligence derived rows, checkpoints, and runtime traces were cleared.',
      'Canonical archive visits, manifests, and import history were left untouched.',
    ],
  }
}

/**
 * Builds mock queue status.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockQueueStatus(state: MockBackendState): AiQueueStatus {
  return {
    paused: state.snapshot.config.ai.jobQueuePaused,
    concurrency: state.snapshot.config.ai.jobQueueConcurrency,
    queued: state.queueJobs.filter(
      (job) => job.state === 'queued' || job.state === 'paused',
    ).length,
    running: state.queueJobs.filter((job) => job.state === 'running').length,
    failed: state.queueJobs.filter((job) => job.state === 'failed').length,
    recentJobs: state.queueJobs.slice(0, 8).map((job) => structuredClone(job)),
  }
}

/**
 * Explains how sync mock ai status works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function syncMockAiStatus(state: MockBackendState) {
  const queue = buildMockQueueStatus(state)
  state.snapshot.aiStatus.queuePaused = queue.paused
  state.snapshot.aiStatus.queueConcurrency = queue.concurrency
  state.snapshot.aiStatus.queuedJobs = queue.queued
  state.snapshot.aiStatus.runningJobs = queue.running
  state.snapshot.aiStatus.failedJobs = queue.failed
  state.snapshot.aiStatus.recentJobs = structuredClone(queue.recentJobs)
}

/**
 * Explains how sync mock intelligence runtime works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function syncMockIntelligenceRuntime(state: MockBackendState) {
  const enabledById = new Map(
    resolveEnrichmentSettings(state.snapshot.config.enrichment).plugins.map(
      (plugin) => [plugin.id, plugin.enabled],
    ),
  )
  const moduleEnabledById = new Map(
    state.snapshot.config.deterministic.modules.map((module) => [
      module.id,
      module.enabled,
    ]),
  )
  const recentJobs = state.intelligenceRuntime.recentJobs
  const activityTimes = recentJobs
    .flatMap((job) => [job.finishedAt, job.startedAt, job.createdAt])
    .filter((value): value is string => Boolean(value))
  const lastActivityAt =
    activityTimes.length > 0
      ? activityTimes.sort()[activityTimes.length - 1]
      : null

  state.intelligenceRuntime.queue = {
    queued: recentJobs.filter((job) => job.state === 'queued').length,
    running: recentJobs.filter((job) => job.state === 'running').length,
    succeeded: recentJobs.filter((job) => job.state === 'succeeded').length,
    failed: recentJobs.filter((job) => job.state === 'failed').length,
    cancelled: recentJobs.filter((job) => job.state === 'cancelled').length,
    lastActivityAt,
  }
  state.intelligenceRuntime.plugins = state.intelligenceRuntime.plugins.map(
    (plugin) => ({
      ...plugin,
      enabled: enabledById.get(plugin.pluginId) ?? plugin.enabled,
      queuedJobs: recentJobs.filter(
        (job) => job.pluginId === plugin.pluginId && job.state === 'queued',
      ).length,
      runningJobs: recentJobs.filter(
        (job) => job.pluginId === plugin.pluginId && job.state === 'running',
      ).length,
      failedJobs: recentJobs.filter(
        (job) => job.pluginId === plugin.pluginId && job.state === 'failed',
      ).length,
      lastCompletedAt:
        recentJobs
          .filter(
            (job) =>
              job.pluginId === plugin.pluginId &&
              job.state === 'succeeded' &&
              job.finishedAt,
          )
          .map((job) => job.finishedAt!)
          .sort()
          .at(-1) ?? null,
      lastError:
        recentJobs.find(
          (job) => job.pluginId === plugin.pluginId && job.state === 'failed',
        )?.lastError ?? null,
    }),
  )
  state.intelligenceRuntime.modules = state.intelligenceRuntime.modules.map(
    (module) => ({
      ...module,
      enabled: moduleEnabledById.get(module.moduleId) ?? module.enabled,
      status:
        moduleEnabledById.get(module.moduleId) === false
          ? 'disabled'
          : module.status,
      notes:
        moduleEnabledById.get(module.moduleId) === false
          ? ['Disabled in Settings.']
          : module.notes,
    }),
  )
  state.intelligenceRuntime.notes = [
    'Browser preview mode shows a deterministic queue/runtime fixture.',
    enabledById.get(READABLE_CONTENT_REFETCH_PLUGIN_ID) === false
      ? 'Readable content refetch is disabled, so queued network enrichment will stay paused until you re-enable it.'
      : 'Built-in enrichment stays inside the first-party runtime boundary in browser preview mode.',
  ]
}

/**
 * Builds mock dashboard snapshot.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockDashboardSnapshot(
  state: MockBackendState,
): DashboardSnapshot {
  if (!state.snapshot.config.initialized) {
    return {
      generatedAt: new Date().toISOString(),
      totalProfiles: 0,
      totalUrls: 0,
      totalVisits: 0,
      totalDownloads: 0,
      lastSuccessfulBackupAt: null,
      recentRuns: state.snapshot.recentRuns,
      storage: {
        archiveDatabaseBytes: 0,
        sourceEvidenceDatabaseBytes: 0,
        searchDatabaseBytes: 0,
        intelligenceDatabaseBytes: 0,
        manifestBytes: 0,
        snapshotBytes: 0,
        exportBytes: 0,
        stagingBytes: 0,
        quarantineBytes: 0,
        semanticSidecarBytes: 0,
        intelligenceBlobBytes: 0,
      },
      nextAction:
        'Initialize the archive, confirm at least one Chromium profile, and run the first manual backup.',
    }
  }

  return {
    generatedAt: new Date().toISOString(),
    totalProfiles: state.snapshot.config.selectedProfileIds.filter(
      (profileId) =>
        profileId.startsWith('chrome:') || profileId.startsWith('arc:'),
    ).length,
    totalUrls: uniqueUrlCount(state.history.items),
    totalVisits: state.history.items.length,
    totalDownloads: state.snapshot.recentRuns[0]?.newDownloads ?? 1,
    lastSuccessfulBackupAt: state.snapshot.archiveStatus.lastSuccessfulBackupAt,
    recentRuns: state.snapshot.recentRuns,
    storage: {
      archiveDatabaseBytes: 146_800_640,
      sourceEvidenceDatabaseBytes: 9_830_400,
      searchDatabaseBytes: 18_432_000,
      intelligenceDatabaseBytes: 24_576_000,
      manifestBytes: 384_000,
      snapshotBytes: 1_228_800,
      exportBytes: 96_000,
      stagingBytes: 0,
      quarantineBytes: 0,
      semanticSidecarBytes: 41_943_040,
      intelligenceBlobBytes: 12_582_912,
    },
    nextAction:
      state.snapshot.recentRuns.length === 0
        ? 'Run the first manual backup to write the manifest chain and explorer index.'
        : null,
  }
}

/**
 * Builds mock audit run detail.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockAuditRunDetail(
  state: MockBackendState,
  runId: number,
): AuditRunDetail {
  const run = state.snapshot.recentRuns.find((item) => item.id === runId)
  if (!run) {
    throw new Error(`Mock backend does not know audit run ${runId}`)
  }

  const artifactPath =
    run.runType === 'rekey'
      ? `${state.snapshot.directories.rawSnapshotsDir}/rekey/archive-before-rekey-${run.id}.sqlite`
      : run.runType === 'snapshot_restore'
        ? `${state.snapshot.directories.rawSnapshotsDir}/chrome:Default/2026-04-09T10-00-00.000Z`
        : run.runType === 'retention_prune'
          ? state.snapshot.directories.appRoot
          : `${state.snapshot.directories.rawSnapshotsDir}/run-${run.id}`
  const artifactReason =
    run.runType === 'rekey'
      ? 'before-rekey'
      : run.runType === 'snapshot_restore'
        ? 'restored-source-checkpoint'
        : run.runType === 'retention_prune'
          ? 'pruned-retention-buckets'
          : 'periodic-checkpoint'
  const artifactKind =
    run.runType === 'retention_prune' ? 'retention' : 'snapshot'

  return {
    run,
    trigger: run.trigger ?? 'manual',
    timezone: 'America/Phoenix',
    dueOnly: false,
    profileScope: run.profileScope ?? state.snapshot.config.selectedProfileIds,
    warnings: [],
    errorMessage: null,
    stats: {
      profilesProcessed: run.profilesProcessed,
      newVisits: run.newVisits,
      newUrls: run.newUrls,
      newDownloads: run.newDownloads,
    },
    manifestPath: `${state.snapshot.directories.manifestsDir}/2026-04-06/run-${run.id}-preview.json`,
    manifestHash: run.manifestHash ?? `preview-manifest-${run.id}`,
    artifacts: [
      {
        kind: artifactKind,
        path: artifactPath,
        checksum: `snapshot-${run.id}`,
        sizeBytes: 4096,
        createdAt: run.finishedAt ?? run.startedAt,
        reason: artifactReason,
      },
    ],
  }
}

/**
 * Explains how prepend mock run works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function prependMockRun(
  state: MockBackendState,
  run: BackupRunOverview,
): BackupRunOverview {
  state.snapshot.recentRuns = [run, ...state.snapshot.recentRuns]
  return run
}

/**
 * Normalizes mock platform into the canonical UI shape.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function normalizeMockPlatform(
  platform?: unknown,
): 'macos' | 'windows' | 'linux' {
  if (platform === 'windows') return 'windows'
  if (platform === 'linux') return 'linux'
  return 'macos'
}

/**
 * Builds mock schedule plan.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockSchedulePlan(platform?: unknown): SchedulePlan {
  const resolvedPlatform = normalizeMockPlatform(platform)
  if (resolvedPlatform === 'windows') {
    return {
      platform: 'windows',
      label: 'com.yi-ting.pathkeep.backup',
      executablePath: 'C:/Program Files/PathKeep/pathkeep.exe',
      generatedFiles: [
        {
          relativePath: 'schedule/com.yi-ting.pathkeep.task.xml',
          absolutePath:
            'C:/Users/test/AppData/Local/com.yi-ting.pathkeep/schedule/com.yi-ting.pathkeep.task.xml',
          purpose: 'Task Scheduler XML',
          contents:
            '<Task><Triggers><TimeTrigger /></Triggers><Settings><StartWhenAvailable>true</StartWhenAvailable></Settings></Task>',
        },
      ],
      manualSteps: [
        'Review the generated Task Scheduler XML.',
        'Import it manually in Task Scheduler if you do not want PathKeep to apply it.',
      ],
      applyCommands: [
        ['schtasks', '/Create', '/XML', 'com.yi-ting.pathkeep.task.xml'],
      ],
      rollbackCommands: [
        ['schtasks', '/Delete', '/TN', 'com.yi-ting.pathkeep.backup', '/F'],
      ],
      applySupported: false,
    }
  }

  if (resolvedPlatform === 'linux') {
    return {
      platform: 'linux',
      label: 'com.yi-ting.pathkeep.backup',
      executablePath: '/usr/bin/pathkeep',
      generatedFiles: [
        {
          relativePath: 'schedule/com.yi-ting.pathkeep.service',
          absolutePath:
            '/home/test/.config/systemd/user/com.yi-ting.pathkeep.service',
          purpose: 'systemd user service',
          contents:
            '[Unit]\nDescription=PathKeep backup\n[Service]\nExecStart=/usr/bin/pathkeep backup',
        },
        {
          relativePath: 'schedule/com.yi-ting.pathkeep.timer',
          absolutePath:
            '/home/test/.config/systemd/user/com.yi-ting.pathkeep.timer',
          purpose: 'systemd user timer',
          contents:
            '[Timer]\nOnCalendar=hourly\nPersistent=true\n[Install]\nWantedBy=timers.target',
        },
      ],
      manualSteps: [
        'Review the generated systemd user unit files.',
        'Copy them into ~/.config/systemd/user and run systemctl --user daemon-reload.',
      ],
      applyCommands: [
        [
          'systemctl',
          '--user',
          'enable',
          '--now',
          'com.yi-ting.pathkeep.timer',
        ],
      ],
      rollbackCommands: [
        [
          'systemctl',
          '--user',
          'disable',
          '--now',
          'com.yi-ting.pathkeep.timer',
        ],
      ],
      applySupported: false,
    }
  }

  return {
    platform: 'macos',
    label: 'com.yi-ting.pathkeep.backup',
    executablePath: '/Applications/PathKeep.app',
    generatedFiles: [
      {
        relativePath: 'schedule/com.yi-ting.pathkeep.backup.plist',
        absolutePath:
          '/Users/test/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
        purpose: 'LaunchAgent plist',
        contents:
          '<?xml version="1.0"?><plist><dict><key>Label</key><string>com.yi-ting.pathkeep.backup</string></dict></plist>',
      },
    ],
    manualSteps: [
      'Open the desktop build to verify the LaunchAgent artifact and install status.',
    ],
    applyCommands: [
      [
        'launchctl',
        'bootstrap',
        'gui/501',
        'com.yi-ting.pathkeep.backup.plist',
      ],
    ],
    rollbackCommands: [
      ['launchctl', 'bootout', 'gui/501', 'com.yi-ting.pathkeep.backup'],
    ],
    applySupported: false,
  }
}

/**
 * Builds mock schedule status.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockScheduleStatus(
  state: MockBackendState,
  platform?: unknown,
): ScheduleStatus {
  const resolvedPlatform = normalizeMockPlatform(platform)
  return {
    platform: resolvedPlatform,
    label: 'com.yi-ting.pathkeep.backup',
    dueAfterHours: state.snapshot.config.dueAfterHours,
    checkIntervalHours: state.snapshot.config.scheduleCheckIntervalHours,
    applySupported: false,
    installState: 'manual-review',
    detectedFiles: [],
    manualSteps:
      resolvedPlatform === 'windows'
        ? [
            'Browser preview mode cannot inspect Task Scheduler directly.',
            'Review the XML, then import it manually if you want to test the plan.',
          ]
        : resolvedPlatform === 'linux'
          ? [
              'Browser preview mode cannot inspect systemd user services directly.',
              'Review the generated units, then run the documented systemctl --user commands manually.',
            ]
          : [
              'Browser preview mode cannot inspect the installed native schedule state.',
              'Open the desktop build to verify the LaunchAgent artifact and install status.',
            ],
    auditPath: null,
    lastSuccessfulBackupAt: state.snapshot.archiveStatus.lastSuccessfulBackupAt,
    warnings: [
      resolvedPlatform === 'windows'
        ? 'Browser preview mode keeps Task Scheduler verification read-only. Use the desktop app or Task Scheduler to inspect the real install state.'
        : resolvedPlatform === 'linux'
          ? 'Browser preview mode keeps systemd verification read-only. Use the desktop app or systemctl --user to inspect the real install state.'
          : 'Browser preview mode keeps schedule verification read-only. Use the desktop app for the real platform status.',
    ],
  }
}

/**
 * Explains how override mock schedule works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function overrideMockSchedule(
  state: MockBackendState,
  plan: SchedulePlan,
  status?: ScheduleStatus,
) {
  const resolvedPlanPlatform = normalizeMockPlatform(plan.platform)
  state.schedulePlanOverrides[resolvedPlanPlatform] = structuredClone(plan)
  if (status) {
    const resolvedStatusPlatform = normalizeMockPlatform(status.platform)
    state.scheduleStatusOverrides[resolvedStatusPlatform] =
      structuredClone(status)
    return
  }

  const resolvedStatus: ScheduleStatus = {
    ...buildMockScheduleStatus(state, plan.platform),
    platform: plan.platform,
    label: plan.label,
    applySupported: plan.applySupported,
    detectedFiles: plan.generatedFiles
      .map((file) => file.absolutePath ?? file.relativePath)
      .filter((value): value is string => Boolean(value)),
    manualSteps:
      plan.manualSteps.length > 0
        ? structuredClone(plan.manualSteps)
        : buildMockScheduleStatus(state, plan.platform).manualSteps,
    installState: plan.applySupported ? 'installed' : 'manual-review',
    warnings: [],
  }
  state.scheduleStatusOverrides[resolvedPlanPlatform] = resolvedStatus
}

/**
 * Builds mock security status.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockSecurityStatus(state: MockBackendState): SecurityStatus {
  const warnings = state.snapshot.archiveStatus.warning
    ? [state.snapshot.archiveStatus.warning]
    : []
  const lastRekeyRun =
    state.snapshot.recentRuns.find((run) => run.runType === 'rekey') ?? null

  if (
    state.snapshot.config.archiveMode === 'Encrypted' &&
    state.snapshot.config.rememberDatabaseKeyInKeyring &&
    !state.snapshot.keyringStatus.storedSecret
  ) {
    warnings.push(
      'Archive is encrypted, but the database key is not currently stored in the system keyring.',
    )
  }

  const mode = !state.snapshot.archiveStatus.initialized
    ? 'uninitialized'
    : !state.snapshot.archiveStatus.encrypted
      ? 'plaintext'
      : state.snapshot.archiveStatus.unlocked
        ? 'encrypted'
        : 'locked'

  return {
    initialized: state.snapshot.archiveStatus.initialized,
    mode,
    encrypted: state.snapshot.archiveStatus.encrypted,
    unlocked: state.snapshot.archiveStatus.unlocked,
    databasePath: state.snapshot.archiveStatus.databasePath,
    strongholdPath: state.snapshot.directories.strongholdPath,
    rememberDatabaseKeyInKeyring:
      state.snapshot.config.rememberDatabaseKeyInKeyring,
    lastSuccessfulBackupAt: state.snapshot.archiveStatus.lastSuccessfulBackupAt,
    lastRekeyAt: lastRekeyRun?.finishedAt ?? null,
    lastRekeyRunId: lastRekeyRun?.id ?? null,
    lastRekeySnapshotPath: lastRekeyRun
      ? `${state.snapshot.directories.rawSnapshotsDir}/rekey/archive-before-rekey-${lastRekeyRun.id}.sqlite`
      : null,
    keyringStatus: structuredClone(state.snapshot.keyringStatus),
    warnings,
  }
}

/**
 * Builds mock rekey preview.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockRekeyPreview(
  state: MockBackendState,
  request: RekeyRequest,
): RekeyPreview {
  if (!state.snapshot.archiveStatus.initialized) {
    throw new Error(
      'Initialize the archive before previewing a rekey operation.',
    )
  }

  const warnings: string[] = []
  if (
    state.snapshot.archiveStatus.encrypted &&
    !state.snapshot.archiveStatus.unlocked
  ) {
    warnings.push(
      'The archive is currently locked. Unlock it before executing the rekey.',
    )
  }
  if (request.newMode === 'Encrypted' && !request.newKey?.trim()) {
    warnings.push(
      'Encrypted rekey requires a new database key before execute can run.',
    )
  }
  if (state.snapshot.config.archiveMode === request.newMode) {
    warnings.push(
      'The target mode matches the current mode, so PathKeep will treat this as a key rotation / validation pass.',
    )
  }

  return {
    currentMode: state.snapshot.config.archiveMode,
    nextMode: request.newMode,
    requiresNewKey: request.newMode === 'Encrypted',
    snapshotPath: `${state.snapshot.directories.rawSnapshotsDir}/rekey/archive-before-rekey-<timestamp>.sqlite`,
    tempDatabasePath: `${state.snapshot.directories.archiveDatabasePath}.rekey.sqlite`,
    steps: [
      'Create a safety snapshot before rewriting the archive.',
      'Export the archive into a temporary database using the requested target mode.',
      'Swap the rewritten database into place only after the export succeeds.',
    ],
    warnings,
  }
}

/**
 * Builds mock snapshot restore preview.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockSnapshotRestorePreview(
  state: MockBackendState,
  request: SnapshotRestoreRequest,
): SnapshotRestorePreview {
  const snapshotPath =
    request.snapshotPath ||
    `${state.snapshot.directories.rawSnapshotsDir}/run-1`
  const archiveSnapshot = snapshotPath.endsWith('.sqlite')
  return {
    snapshotPath,
    snapshotKind: archiveSnapshot
      ? 'archive-safety-snapshot'
      : 'raw-source-checkpoint',
    sourceRunId: state.snapshot.recentRuns[0]?.id ?? null,
    sourceProfileId: archiveSnapshot ? null : 'chrome:Default',
    sourceBrowserName: archiveSnapshot ? null : 'Google Chrome',
    createdAt: new Date().toISOString(),
    reason: archiveSnapshot ? 'before-rekey' : 'periodic-checkpoint',
    executeSupported: !archiveSnapshot,
    estimatedVisits: archiveSnapshot ? 0 : 2,
    estimatedUrls: archiveSnapshot ? 0 : 1,
    estimatedDownloads: archiveSnapshot ? 0 : 0,
    warnings: archiveSnapshot
      ? [
          'This snapshot is a full archive safety copy. PathKeep currently automates restore only for saved browser source checkpoints; keep this file for manual recovery review.',
        ]
      : [
          'Snapshot restore replays the saved browser checkpoint into the current archive. Existing visible archive facts stay in place and duplicate rows are skipped.',
        ],
  }
}

/**
 * Builds mock retention preview.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockRetentionPreview(state: MockBackendState): RetentionPreview {
  const dashboard = buildMockDashboardSnapshot(state)
  return {
    buckets: [
      {
        id: 'snapshots',
        bytes: dashboard.storage.snapshotBytes,
        itemCount: 3,
        paths: [state.snapshot.directories.rawSnapshotsDir],
      },
      {
        id: 'exports',
        bytes: dashboard.storage.exportBytes,
        itemCount: 2,
        paths: [state.snapshot.directories.exportsDir],
      },
      {
        id: 'staging',
        bytes: dashboard.storage.stagingBytes,
        itemCount: Math.sign(dashboard.storage.stagingBytes),
        paths: [state.snapshot.directories.stagingDir],
      },
      {
        id: 'quarantine',
        bytes: dashboard.storage.quarantineBytes,
        itemCount: Math.sign(dashboard.storage.quarantineBytes),
        paths: [state.snapshot.directories.quarantineDir],
      },
    ],
    warnings: [
      'Pruning snapshots removes saved restore checkpoints from future Audit review. Manifest and run summaries stay in place.',
      'Export pruning only removes local files under the PathKeep data directory. Remote objects are unchanged.',
    ],
  }
}

/**
 * Builds mock takeout inspection.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function buildMockTakeoutInspection(
  state: MockBackendState,
  sourcePath: string,
  dryRun: boolean,
): TakeoutInspection {
  const previewEntries = [
    {
      sourcePath: `${sourcePath}/Takeout/Chrome/BrowserHistory.json`,
      url: 'https://example.org/archive/trust-ui',
      title: 'PathKeep trust UX notes',
      visitedAt: new Date(Date.now() - 86_400_000).toISOString(),
      sourceVisitId: 41,
      status: dryRun ? 'preview' : 'imported',
    },
    {
      sourcePath: `${sourcePath}/Takeout/Chrome/BrowserHistory.json`,
      url: 'https://example.org/archive/linux-timer',
      title: 'systemd timer notes',
      visitedAt: new Date(Date.now() - 43_200_000).toISOString(),
      sourceVisitId: 42,
      status: dryRun ? 'preview' : 'imported',
    },
  ]
  const recognizedFiles = [
    {
      path: `${sourcePath}/Takeout/Chrome/BrowserHistory.json`,
      kind: 'browser-history',
      status: dryRun ? 'preview' : 'imported',
      records: previewEntries.length,
    },
  ]
  const quarantinedFiles = [
    {
      path: `${sourcePath}/Takeout/Chrome/unsupported.csv`,
      kind: 'unknown',
      status: 'quarantined',
      records: 1,
    },
  ]
  const notes = dryRun
    ? [
        'Preview includes recognized BrowserHistory rows and quarantined unsupported files.',
      ]
    : [
        'Import wrote a local batch and kept unsupported files quarantined for audit review.',
      ]

  if (dryRun) {
    return {
      dryRun: true,
      sourcePath,
      recognizedFiles,
      quarantinedFiles,
      previewEntries,
      candidateItems: 2,
      importedItems: 0,
      duplicateItems: 1,
      notes,
      importBatch: null,
    }
  }

  const batchId = state.nextImportBatchId
  state.nextImportBatchId += 1

  const importedAt = new Date().toISOString()
  const importBatch: ImportBatchOverview = {
    id: batchId,
    sourceKind: 'takeout',
    sourcePath,
    profileId: 'takeout::browser-history',
    createdAt: new Date().toISOString(),
    importedAt,
    revertedAt: null,
    status: 'imported',
    candidateItems: 2,
    importedItems: 2,
    duplicateItems: 1,
    visibleItems: 2,
    auditPath: `${state.snapshot.directories.quarantineDir}/import-batch-${batchId}.json`,
    gitCommit: null,
  }

  state.snapshot.recentImportBatches = [
    importBatch,
    ...state.snapshot.recentImportBatches,
  ]
  prependMockRun(state, {
    id: (state.snapshot.recentRuns[0]?.id ?? batchId) + 1,
    startedAt: importedAt,
    finishedAt: importedAt,
    status: 'success',
    runType: 'import',
    trigger: 'manual',
    profileScope: [importBatch.profileId],
    manifestHash: null,
    profilesProcessed: 1,
    newVisits: importBatch.importedItems,
    newUrls: 0,
    newDownloads: 0,
  })
  state.importBatchDetails[batchId] = {
    batch: importBatch,
    previewEntries,
    recognizedFiles,
    quarantinedFiles,
    notes,
  }

  return {
    dryRun: false,
    sourcePath,
    recognizedFiles,
    quarantinedFiles,
    previewEntries,
    candidateItems: 2,
    importedItems: 2,
    duplicateItems: 1,
    notes,
    importBatch,
  }
}

/**
 * Explains how mutate import batch works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function mutateImportBatch(
  state: MockBackendState,
  batchId: number,
  action: 'revert' | 'restore',
): ImportBatchDetail {
  const detail = state.importBatchDetails[batchId]
  if (!detail) {
    throw new Error(`Mock backend does not know import batch ${batchId}`)
  }

  const updatedBatch: ImportBatchOverview = {
    ...detail.batch,
    status: action === 'revert' ? 'reverted' : 'imported',
    revertedAt: action === 'revert' ? new Date().toISOString() : null,
    visibleItems: action === 'revert' ? 0 : detail.batch.importedItems,
  }

  const updatedDetail: ImportBatchDetail = {
    ...detail,
    batch: updatedBatch,
    previewEntries: detail.previewEntries.map((entry) => ({
      ...entry,
      status: action === 'revert' ? 'reverted' : 'imported',
    })),
    notes: [
      action === 'revert'
        ? 'Import batch was reverted from the live archive view.'
        : 'Import batch was restored into the live archive view.',
    ],
  }

  state.importBatchDetails[batchId] = updatedDetail
  state.snapshot.recentImportBatches = state.snapshot.recentImportBatches.map(
    (batch) => (batch.id === batchId ? updatedBatch : batch),
  )
  prependMockRun(state, {
    id: (state.snapshot.recentRuns[0]?.id ?? batchId) + 1,
    startedAt: new Date().toISOString(),
    finishedAt: new Date().toISOString(),
    status: 'success',
    runType: action === 'revert' ? 'rollback' : 'restore',
    trigger: 'manual',
    profileScope: [updatedBatch.profileId],
    manifestHash: null,
    profilesProcessed: 1,
    newVisits: updatedBatch.importedItems,
    newUrls: 0,
    newDownloads: 0,
  })
  return updatedDetail
}

/**
 * Explains how filter mock history works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function filterMockHistory(
  state: MockBackendState,
  query: HistoryQuery | undefined,
): HistoryQueryResponse {
  const rawQuery = query?.q?.trim() ?? ''
  const q = rawQuery.toLowerCase()
  const domain = query?.domain?.trim().toLowerCase() ?? ''
  const profileId = query?.profileId ?? null
  const browserKind = query?.browserKind ?? null
  const startTimeMs = query?.startTimeMs ?? null
  const endTimeMs = query?.endTimeMs ?? null
  const sort = query?.sort ?? 'newest'
  const limit = Math.max(1, Math.min(query?.limit ?? 150, 1000))
  const requestedPage = Math.max(1, Math.floor(query?.page ?? 1))
  const cursor = parseMockHistoryCursor(query?.cursor)
  const regex = query?.regexMode && rawQuery ? new RegExp(rawQuery, 'i') : null

  const filteredItems = [...state.history.items]
    .filter((item) => !profileId || item.profileId === profileId)
    .filter(
      (item) =>
        !browserKind ||
        browserKindFromProfileId(item.profileId) === browserKind,
    )
    .filter(
      (item) =>
        !q ||
        (regex
          ? regex.test(item.url) || regex.test(item.title ?? '')
          : item.url.toLowerCase().includes(q) ||
            (item.title ?? '').toLowerCase().includes(q)),
    )
    .filter((item) => !domain || item.domain.toLowerCase().includes(domain))
    .filter((item) => !startTimeMs || item.visitTime >= startTimeMs)
    .filter((item) => !endTimeMs || item.visitTime <= endTimeMs)
    .sort((left, right) =>
      sort === 'oldest'
        ? left.visitTime - right.visitTime
        : right.visitTime - left.visitTime,
    )

  const pageCount = Math.max(1, Math.ceil(filteredItems.length / limit))
  const cursorStartIndex = (() => {
    if (!cursor) return 0
    const nextIndex = filteredItems.findIndex((item) => {
      if (sort === 'oldest') {
        return (
          item.visitTime > cursor.visitTime ||
          (item.visitTime === cursor.visitTime && item.id > cursor.id)
        )
      }
      return (
        item.visitTime < cursor.visitTime ||
        (item.visitTime === cursor.visitTime && item.id < cursor.id)
      )
    })
    return nextIndex === -1 ? filteredItems.length : nextIndex
  })()
  const page =
    query?.page != null
      ? Math.min(requestedPage, pageCount)
      : Math.max(1, Math.floor(cursorStartIndex / limit) + 1)
  const startIndex =
    query?.page != null ? (page - 1) * limit : Math.max(0, cursorStartIndex)
  const items = filteredItems.slice(startIndex, startIndex + limit)
  const hasNext = startIndex + limit < filteredItems.length
  const hasPrevious = startIndex > 0

  return {
    total: filteredItems.length,
    items,
    page,
    pageSize: limit,
    pageCount,
    hasPrevious,
    hasNext,
    nextCursor:
      hasNext && items.length > 0
        ? encodeMockHistoryCursor(items[items.length - 1])
        : null,
  }
}

/**
 * Parses mock history cursor into the shape this surface expects.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function parseMockHistoryCursor(cursor?: string | null) {
  if (!cursor) return null
  const [visitTime, id] = cursor.split('|')
  const parsedVisitTime = Number(visitTime)
  const parsedId = Number(id)
  if (!Number.isFinite(parsedVisitTime) || !Number.isFinite(parsedId)) {
    return null
  }
  return {
    visitTime: parsedVisitTime,
    id: parsedId,
  }
}

/**
 * Explains how encode mock history cursor works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function encodeMockHistoryCursor(item: HistoryQueryResponse['items'][number]) {
  return `${item.visitTime}|${item.id}`
}

/**
 * Explains how paginate mock ai search works.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function paginateMockAiSearch(
  state: MockBackendState,
  request?: AiSearchRequest,
): AiSearchResponse {
  const limit = Math.max(1, Math.min(request?.limit ?? 24, 50))
  const offset = Math.max(0, Number.parseInt(request?.cursor ?? '0', 10) || 0)
  const items = state.history.items.map((item, index) => ({
    historyId: item.id,
    profileId: item.profileId,
    url: item.url,
    title: item.title,
    domain: item.domain,
    visitedAt: item.visitedAt,
    score: 0.8 - index * 0.1,
    matchReason: 'Browser preview lexical fixture',
  }))
  const pagedItems = items.slice(offset, offset + limit)
  const nextOffset = offset + pagedItems.length

  return {
    total: items.length,
    providerId: 'lexical-fallback',
    model: 'none',
    items: pagedItems,
    notes: ['Semantic retrieval is unavailable in browser preview mode.'],
    nextCursor: nextOffset < items.length ? String(nextOffset) : null,
  }
}

/**
 * Creates mock state.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
 */
function createMockState(): MockBackendState {
  const state: MockBackendState = {
    snapshot: structuredClone(mockSnapshot),
    history: structuredClone(mockHistory),
    keyringSecret: null,
    s3Credentials: null,
    appLockPasscode: null,
    appLockRecoveryHint: null,
    biometricState: 'unsupported',
    importBatchDetails: {},
    schedulePlanOverrides: {},
    scheduleStatusOverrides: {},
    intelligenceRuntime: structuredClone(mockIntelligenceRuntime),
    queueJobs: [
      {
        id: 2,
        jobType: 'index-build',
        state: 'failed',
        priority: 70,
        attempt: 1,
        maxAttempts: 3,
        runId: null,
        summary: 'Preview queue fixture needs a replay.',
        queuedAt: new Date(Date.now() - 120_000).toISOString(),
        availableAt: new Date(Date.now() - 60_000).toISOString(),
        startedAt: new Date(Date.now() - 110_000).toISOString(),
        finishedAt: new Date(Date.now() - 100_000).toISOString(),
        heartbeatAt: new Date(Date.now() - 105_000).toISOString(),
        errorCode: 'network-error',
        errorMessage: 'Preview transport timed out.',
      },
      {
        id: 1,
        jobType: 'assistant',
        state: 'queued',
        priority: 100,
        attempt: 0,
        maxAttempts: 1,
        runId: null,
        summary: 'What did I read about LanceDB?',
        queuedAt: new Date(Date.now() - 30_000).toISOString(),
        availableAt: new Date(Date.now() - 30_000).toISOString(),
        startedAt: null,
        finishedAt: null,
        heartbeatAt: null,
        errorCode: null,
        errorMessage: null,
      },
    ],
    nextAiJobId: 3,
    nextImportBatchId: 1,
    lastRemoteBundlePath: null,
    derivedStateCleared: false,
  }
  state.snapshot.config = normalizeMockConfig(
    state.snapshot.config,
    state.s3Credentials,
  )
  syncMockAppLockState(state)
  syncMockAiStatus(state)
  syncMockIntelligenceRuntime(state)
  return state
}

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
    case 'get_digest_summary':
      return {
        dateRange: { start: '', end: '' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
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
 * Exposes the legacy preview-aware backend facade consumed by older routes and tests.
 *
 * The browser-preview backend is intentionally deterministic and testable, so named declarations help keep preview-fixture behavior honest instead of magical.
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
