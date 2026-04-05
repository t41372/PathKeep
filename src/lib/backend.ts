import { invoke, isTauri } from '@tauri-apps/api/core'
import type {
  AiAssistantRequest,
  AiAssistantResponse,
  AiIndexReport,
  AiIndexRequest,
  AiIntegrationPreview,
  AiProviderSecretInput,
  AiSearchRequest,
  AiSearchResponse,
  AppBuildInfo,
  AppConfig,
  AppSnapshot,
  ApplyResult,
  BackupReport,
  ExplainInsightRequest,
  ExportRequest,
  ExportResult,
  HealthReport,
  HistoryQuery,
  HistoryQueryResponse,
  ImportBatchDetail,
  InsightExplanation,
  InsightSnapshot,
  InsightThreadDetail,
  KeyringStatusReport,
  RekeyRequest,
  RemoteBackupPreview,
  RemoteBackupResult,
  RunInsightsReport,
  RunInsightsRequest,
  SchedulePlan,
  S3CredentialInput,
  TakeoutInspection,
  TakeoutRequest,
} from './types'

// Stryker disable all: browser-preview fixtures are static reference data, not behavior.
const mockBuildInfo: AppBuildInfo = {
  productName: 'Browser History Backup',
  version: '0.1.0',
  gitCommitShort: 'preview',
  gitCommitFull: 'preview-build',
  gitDirty: true,
}

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
    selectedProfileIds: [
      'chrome:Default',
      'chrome:Profile 2',
      'safari:default',
    ],
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
    ai: {
      enabled: false,
      assistantEnabled: false,
      semanticIndexEnabled: false,
      mcpEnabled: false,
      skillEnabled: false,
      autoIndexAfterBackup: false,
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
      '~/Library/Application Support/Browser History Backup/archive/history-vault.sqlite',
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
    ready: false,
    indexedItems: 0,
    lastIndexedAt: null,
    llmProviderId: null,
    embeddingProviderId: null,
    warning: null,
  },
  insightStatus: {
    ready: true,
    lastRunAt: new Date().toISOString(),
    runs: 4,
    cards: 4,
    topics: 3,
    threads: 2,
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
    },
  ],
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

const mockInsightSnapshot: InsightSnapshot = {
  generatedAt: new Date().toISOString(),
  windowDays: 30,
  profileId: 'chrome:Default',
  status: structuredClone(mockSnapshot.insightStatus),
  cards: [
    {
      cardId: 'card-rising-topic-1',
      kind: 'rising-topic',
      title: 'Rising topic: archive tooling',
      summary:
        'Archive tooling is gaining momentum across docs, repo issues, and comparison pages.',
      windowDays: 30,
      profileId: 'chrome:Default',
      score: 0.82,
      chromiumEnhanced: true,
      evidence: [
        {
          historyId: 1,
          profileId: 'chrome:Default',
          url: mockHistory.items[0].url,
          title: mockHistory.items[0].title,
          visitedAt: mockHistory.items[0].visitedAt,
          note: 'Topic momentum increased this week.',
        },
      ],
    },
    {
      cardId: 'card-open-loop-thread-1',
      kind: 'open-loop',
      title: 'Open loop: archive tool compare',
      summary:
        'This thread reopened twice and still leans on compare/docs/forum patterns.',
      windowDays: 30,
      profileId: 'chrome:Default',
      score: 2.15,
      chromiumEnhanced: true,
      evidence: [
        {
          historyId: 2,
          profileId: 'chrome:Default',
          url: mockHistory.items[1].url,
          title: mockHistory.items[1].title,
          visitedAt: mockHistory.items[1].visitedAt,
          note: 'Repeated revisit signal.',
        },
      ],
    },
  ],
  topics: [
    {
      topicId: 'topic-001',
      label: 'Archive tooling',
      profileScope: 'chrome:Default',
      windowDays: 30,
      firstSeenAt: '2026-04-01T12:00:00.000Z',
      lastSeenAt: '2026-04-03T16:00:00.000Z',
      visitCount: 7,
      revisitCount: 2,
      trendSlope: 0.82,
      burstScore: 2.4,
      evidence: [
        {
          historyId: 1,
          profileId: 'chrome:Default',
          url: mockHistory.items[0].url,
          title: mockHistory.items[0].title,
          visitedAt: mockHistory.items[0].visitedAt,
          note: 'Representative evidence',
        },
      ],
    },
    {
      topicId: 'topic-002',
      label: 'Schema spelunking',
      profileScope: 'chrome:Default',
      windowDays: 30,
      firstSeenAt: '2026-03-29T12:00:00.000Z',
      lastSeenAt: '2026-04-03T12:30:00.000Z',
      visitCount: 5,
      revisitCount: 2,
      trendSlope: 0.33,
      burstScore: 1.3,
      evidence: [],
    },
  ],
  threads: [
    {
      threadId: 'thread-001',
      profileId: 'chrome:Default',
      title: 'archive tool compare',
      status: 'open-loop',
      firstSeenAt: '2026-04-01T12:00:00.000Z',
      lastSeenAt: '2026-04-03T16:00:00.000Z',
      visitCount: 6,
      reopenCount: 2,
      openLoopScore: 2.15,
      dominantTopicId: 'topic-001',
      chromiumEnhanced: true,
      evidence: [
        {
          historyId: 1,
          profileId: 'chrome:Default',
          url: mockHistory.items[0].url,
          title: mockHistory.items[0].title,
          visitedAt: mockHistory.items[0].visitedAt,
          note: 'Docs revisit',
        },
      ],
    },
  ],
  queryLadders: [
    {
      rootTerm: 'archive tool',
      profileId: 'chrome:Default',
      steps: [
        'archive tool',
        'archive tool compare',
        'archive tool compare github',
      ],
      stages: ['broad', 'compare', 'site-restrict'],
      count: 3,
      chromiumOnly: true,
    },
  ],
  workflowMap: {
    profileId: 'chrome:Default',
    roles: [
      { role: 'search', count: 5 },
      { role: 'docs', count: 4 },
      { role: 'repo', count: 3 },
      { role: 'forum', count: 2 },
    ],
    edges: [
      { fromRole: 'search', toRole: 'docs', count: 3 },
      { fromRole: 'docs', toRole: 'repo', count: 2 },
      { fromRole: 'repo', toRole: 'forum', count: 1 },
    ],
    chromiumEnhanced: true,
  },
  profileFacets: [
    {
      key: 'explore-exploit',
      label: 'Explore vs exploit',
      value: 'Exploit-heavy',
      confidence: 0.71,
      evidence: [],
    },
    {
      key: 'source-preference',
      label: 'Source preference',
      value: 'Docs-first',
      confidence: 0.68,
      evidence: [],
    },
  ],
  notes: ['Browser preview mode shows a deterministic insight fixture.'],
}

const mockInsightThreadDetail: InsightThreadDetail = {
  summary: structuredClone(mockInsightSnapshot.threads[0]),
  visits: [
    {
      historyId: 1,
      profileId: 'chrome:Default',
      url: mockHistory.items[0].url,
      title: mockHistory.items[0].title,
      visitedAt: mockHistory.items[0].visitedAt,
      note: 'Docs revisit',
    },
    {
      historyId: 2,
      profileId: 'chrome:Default',
      url: mockHistory.items[1].url,
      title: mockHistory.items[1].title,
      visitedAt: mockHistory.items[1].visitedAt,
      note: 'Issue follow-up',
    },
  ],
}

const mockInsightRunReport: RunInsightsReport = {
  runId: 12,
  processedVisits: 24,
  enrichedVisits: 8,
  failedEnrichments: 1,
  topicCount: mockInsightSnapshot.topics.length,
  threadCount: mockInsightSnapshot.threads.length,
  cardCount: mockInsightSnapshot.cards.length,
  contentCoverage: 0.64,
  lastRunAt: new Date().toISOString(),
  notes: ['Insight run used preview fixtures and local heuristics.'],
}

const mockInsightExplanation: InsightExplanation = {
  explanation:
    'This insight is based on repeated revisits to archive-related docs, repository issues, and search refinements within the selected window.',
  usedLlm: false,
  citations: structuredClone(mockInsightThreadDetail.visits),
  notes: ['Browser preview mode explains insights from static evidence only.'],
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
    case 'app_build_info':
      return mockBuildInfo as T
    case 'app_snapshot':
    case 'save_config':
    case 'initialize_archive':
    case 'rekey_archive':
      return structuredClone(mockSnapshot) as T
    case 'set_session_database_key':
    case 'clear_session_database_key':
    case 'reset_local_secret_vault':
      return undefined as T
    case 'open_path_in_file_manager':
      return (
        typeof args?.path === 'string'
          ? args.path
          : mockSnapshot.directories.appRoot
      ) as T
    case 'run_backup_now':
      return {
        dueSkipped: false,
        profiles: [],
        warnings: [],
        remoteBackup: null,
      } as T
    case 'query_history':
      return mockHistory as T
    case 'run_insights_now':
      return mockInsightRunReport as T
    case 'load_insights':
      return mockInsightSnapshot as T
    case 'load_thread_detail':
      return mockInsightThreadDetail as T
    case 'explain_insight':
      return mockInsightExplanation as T
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
    case 'store_ai_provider_api_key':
    case 'clear_ai_provider_api_key':
      return structuredClone(mockSnapshot) as T
    case 'build_ai_index':
      return {
        providerId: 'mock-embedding',
        model: 'text-embedding-3-large',
        indexedItems: 2,
        updatedItems: 0,
        skippedItems: 0,
        removedItems: 0,
        lastIndexedAt: new Date().toISOString(),
        notes: ['Browser preview mode uses a static AI index fixture.'],
      } as T
    case 'search_ai_history':
      return {
        total: mockHistory.items.length,
        providerId: 'lexical-fallback',
        model: 'none',
        items: mockHistory.items.map((item, index) => ({
          historyId: item.id,
          profileId: item.profileId,
          url: item.url,
          title: item.title,
          domain: item.domain,
          visitedAt: item.visitedAt,
          score: 0.8 - index * 0.1,
          matchReason: 'Browser preview lexical fixture',
        })),
        notes: ['Semantic retrieval is unavailable in browser preview mode.'],
      } as T
    case 'ask_ai_assistant':
      return {
        answer:
          'Browser preview mode can show the assistant layout, but real LLM answers only run in the desktop app.',
        providerId: 'preview-llm',
        embeddingProviderId: 'lexical-fallback',
        citations: mockHistory.items.map((item) => ({
          historyId: item.id,
          profileId: item.profileId,
          url: item.url,
          title: item.title,
          visitedAt: item.visitedAt,
          score: 0.8,
        })),
        notes: ['Open the desktop build to run real agentic history analysis.'],
      } as T
    case 'preview_ai_integrations':
      return {
        mcpCommand:
          '/Applications/Browser History Backup.app --worker mcp-server',
        manualSteps: [
          'Enable MCP or Skill integration in Settings first.',
          'Store the database key in the native keyring if the archive is encrypted.',
          'Copy the generated MCP JSON into your MCP client configuration.',
        ],
        generatedFiles: [
          {
            relativePath: 'integrations/browser-history-backup-mcp.json',
            absolutePath:
              '~/Library/Application Support/Browser History Backup/integrations/browser-history-backup-mcp.json',
            purpose: 'Browser History Backup MCP client snippet',
            contents: '{\n  "mcpServers": {}\n}',
          },
        ],
        warnings: [],
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
  getAppBuildInfo: () => call<AppBuildInfo>('app_build_info'),
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
  storeAiProviderApiKey: (input: AiProviderSecretInput) =>
    call<AppSnapshot>('store_ai_provider_api_key', { input }),
  clearAiProviderApiKey: (providerId: string) =>
    call<AppSnapshot>('clear_ai_provider_api_key', { providerId }),
  buildAiIndex: (request: AiIndexRequest) =>
    call<AiIndexReport>('build_ai_index', { request }),
  searchAiHistory: (request: AiSearchRequest) =>
    call<AiSearchResponse>('search_ai_history', { request }),
  askAiAssistant: (request: AiAssistantRequest) =>
    call<AiAssistantResponse>('ask_ai_assistant', { request }),
  runInsightsNow: (request: RunInsightsRequest) =>
    call<RunInsightsReport>('run_insights_now', { request }),
  loadInsights: (request: RunInsightsRequest) =>
    call<InsightSnapshot>('load_insights', { request }),
  loadThreadDetail: (threadId: string) =>
    call<InsightThreadDetail>('load_thread_detail', { threadId }),
  explainInsight: (request: ExplainInsightRequest) =>
    call<InsightExplanation>('explain_insight', { request }),
  previewAiIntegrations: () =>
    call<AiIntegrationPreview>('preview_ai_integrations'),
  resetLocalSecretVault: () => call<void>('reset_local_secret_vault'),
  openPathInFileManager: (path: string) =>
    call<string>('open_path_in_file_manager', { path }),
}
