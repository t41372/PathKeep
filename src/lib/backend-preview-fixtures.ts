/**
 * @file backend-preview-fixtures.ts
 * @description Deterministic browser-preview fixture data reused by the preview backend facade and test harnesses.
 * @module lib/backend-preview-fixtures
 *
 * ## Responsibilities
 * - Hold the seeded browser-preview build info, app snapshot, history rows, and intelligence runtime fixtures.
 * - Keep preview-mode data deterministic so browser-preview and tests share one canonical baseline.
 * - Separate static fixture payloads from command handlers so `backend.ts` can shrink around real behavior.
 *
 * ## Not responsible for
 * - Routing commands, mutating preview state, or deciding how browser-preview commands behave.
 * - Defining typed desktop transport APIs; that remains with `backend-client/*` and `ipc/bridge.ts`.
 * - Owning route-level UI copy beyond the honesty strings already embedded in shared fixture payloads.
 *
 * ## Dependencies
 * - Depends on typed frontend contracts from `./types`.
 * - Reuses the canonical enrichment defaults so preview config matches the shipping config shape.
 *
 * ## Performance notes
 * - These fixtures stay static and clone-friendly so browser-preview reads avoid extra compute or hidden randomness.
 */

import { defaultEnrichmentSettings } from './enrichment'
import { defaultExplorerBackgroundPrefetchPages } from './explorer-preferences'
import type {
  AppBuildInfo,
  AppSnapshot,
  HistoryQueryResponse,
  IntelligenceRuntimeSnapshot,
} from './types'

/** Provides the stable build metadata shown whenever PathKeep runs in browser-preview mode. */
export const mockBuildInfo: AppBuildInfo = {
  productName: 'PathKeep',
  version: '0.1.0',
  gitCommitShort: 'preview',
  gitCommitFull: 'preview-build',
  gitDirty: true,
}

/** Seeds the canonical browser-preview app snapshot so shell surfaces and tests start from one shared state shape. */
export const mockSnapshot: AppSnapshot = {
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
    explorerBackgroundPrefetchPages: defaultExplorerBackgroundPrefetchPages,
    appLock: {
      enabled: false,
      idleTimeoutMinutes: 5,
      biometricEnabled: false,
      passcodeEnabled: true,
      passcodeConfigured: false,
      recoveryHint: null,
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

/** Seeds the deterministic preview history rows reused by Explorer, dashboard, and intelligence fixtures. */
export const mockHistory: HistoryQueryResponse = {
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

/** Seeds the deterministic queue/runtime fixture shown by browser-preview intelligence and jobs surfaces. */
export const mockIntelligenceRuntime: IntelligenceRuntimeSnapshot = {
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
