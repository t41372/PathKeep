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
  AuditRunDetail,
  BackupReport,
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
  KeyringStatusReport,
  RekeyPreview,
  RekeyRequest,
  RemoteBackupPreview,
  RemoteBackupResult,
  RunInsightsReport,
  RunInsightsRequest,
  SchedulePlan,
  ScheduleStatus,
  SecurityStatus,
  S3CredentialInput,
  TakeoutInspection,
  TakeoutRequest,
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
    appRoot: '~/Library/Application Support/PathKeep',
    configPath: '~/Library/Application Support/PathKeep/config.json',
    archiveDatabasePath:
      '~/Library/Application Support/PathKeep/archive/history-vault.sqlite',
    auditRepoPath: '~/Library/Application Support/PathKeep/audit',
    manifestsDir: '~/Library/Application Support/PathKeep/audit/manifests',
    exportsDir: '~/Library/Application Support/PathKeep/exports',
    rawSnapshotsDir: '~/Library/Application Support/PathKeep/raw-snapshots',
    stagingDir: '~/Library/Application Support/PathKeep/staging',
    quarantineDir: '~/Library/Application Support/PathKeep/quarantine',
    scheduleDir: '~/Library/Application Support/PathKeep/schedule',
    strongholdPath: '~/Library/Application Support/PathKeep/vault.hold',
    strongholdSaltPath:
      '~/Library/Application Support/PathKeep/stronghold-salt.txt',
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
      prefix: 'pathkeep',
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
      '~/Library/Application Support/PathKeep/archive/history-vault.sqlite',
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

interface MockBackendState {
  snapshot: AppSnapshot
  history: HistoryQueryResponse
  keyringSecret: string | null
}

function browserKindFromProfileId(profileId: string) {
  return profileId.split(':')[0] ?? profileId
}

function uniqueUrlCount(items: HistoryQueryResponse['items']) {
  return new Set(items.map((item) => item.url)).size
}

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
        manifestBytes: 0,
        snapshotBytes: 0,
        exportBytes: 0,
        stagingBytes: 0,
        quarantineBytes: 0,
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
      manifestBytes: 384_000,
      snapshotBytes: 1_228_800,
      exportBytes: 96_000,
      stagingBytes: 0,
      quarantineBytes: 0,
    },
    nextAction:
      state.snapshot.recentRuns.length === 0
        ? 'Run the first manual backup to write the manifest chain and explorer index.'
        : null,
  }
}

function buildMockAuditRunDetail(
  state: MockBackendState,
  runId: number,
): AuditRunDetail {
  const run = state.snapshot.recentRuns.find((item) => item.id === runId)
  if (!run) {
    throw new Error(`Mock backend does not know audit run ${runId}`)
  }

  return {
    run,
    trigger: 'manual',
    timezone: 'America/Phoenix',
    dueOnly: false,
    profileScope: state.snapshot.config.selectedProfileIds,
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
        kind: 'snapshot',
        path: `${state.snapshot.directories.rawSnapshotsDir}/run-${run.id}`,
        checksum: `snapshot-${run.id}`,
        sizeBytes: 4096,
        createdAt: run.finishedAt ?? run.startedAt,
        reason: 'periodic-checkpoint',
      },
    ],
  }
}

function buildMockScheduleStatus(state: MockBackendState): ScheduleStatus {
  return {
    platform: 'macos',
    label: 'dev.codex.pathkeep.backup',
    dueAfterHours: state.snapshot.config.dueAfterHours,
    checkIntervalHours: state.snapshot.config.scheduleCheckIntervalHours,
    applySupported: false,
    installState: 'manual-review',
    detectedFiles: [],
    manualSteps: [
      'Browser preview mode cannot inspect the installed native schedule state.',
      'Open the desktop build to verify the LaunchAgent artifact and install status.',
    ],
    auditPath: null,
    lastSuccessfulBackupAt: state.snapshot.archiveStatus.lastSuccessfulBackupAt,
    warnings: [
      'Browser preview mode keeps schedule verification read-only. Use the desktop app for the real platform status.',
    ],
  }
}

function buildMockSecurityStatus(state: MockBackendState): SecurityStatus {
  const warnings = state.snapshot.archiveStatus.warning
    ? [state.snapshot.archiveStatus.warning]
    : []

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
    keyringStatus: structuredClone(state.snapshot.keyringStatus),
    warnings,
  }
}

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

function filterMockHistory(
  state: MockBackendState,
  query: HistoryQuery | undefined,
): HistoryQueryResponse {
  const q = query?.q?.trim().toLowerCase() ?? ''
  const domain = query?.domain?.trim().toLowerCase() ?? ''
  const profileId = query?.profileId ?? null
  const browserKind = query?.browserKind ?? null
  const startTimeMs = query?.startTimeMs ?? null
  const endTimeMs = query?.endTimeMs ?? null
  const sort = query?.sort ?? 'newest'
  const limit = Math.max(1, Math.min(query?.limit ?? 150, 1000))

  const items = [...state.history.items]
    .filter((item) => !profileId || item.profileId === profileId)
    .filter(
      (item) =>
        !browserKind ||
        browserKindFromProfileId(item.profileId) === browserKind,
    )
    .filter(
      (item) =>
        !q ||
        item.url.toLowerCase().includes(q) ||
        (item.title ?? '').toLowerCase().includes(q),
    )
    .filter((item) => !domain || item.domain.toLowerCase().includes(domain))
    .filter((item) => !startTimeMs || item.visitTime >= startTimeMs)
    .filter((item) => !endTimeMs || item.visitTime <= endTimeMs)
    .sort((left, right) =>
      sort === 'oldest'
        ? left.visitTime - right.visitTime
        : right.visitTime - left.visitTime,
    )
    .slice(0, limit)

  return {
    total: items.length,
    items,
  }
}

function createMockState(): MockBackendState {
  return {
    snapshot: structuredClone(mockSnapshot),
    history: structuredClone(mockHistory),
    keyringSecret: null,
  }
}

let mockState = createMockState()
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
      return structuredClone(mockState.snapshot) as T
    case 'save_config': {
      const nextConfig = structuredClone(args?.config as AppConfig)
      mockState.snapshot.config = nextConfig
      mockState.snapshot.archiveStatus.encrypted =
        nextConfig.archiveMode === 'Encrypted'
      return structuredClone(mockState.snapshot) as T
    }
    case 'initialize_archive': {
      const nextConfig = structuredClone(args?.config as AppConfig)
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
      return structuredClone(mockState.snapshot) as T
    }
    case 'rekey_archive': {
      const request = args?.request as RekeyRequest
      mockState.snapshot.config.archiveMode = request.newMode
      mockState.snapshot.archiveStatus.encrypted =
        request.newMode === 'Encrypted'
      mockState.snapshot.archiveStatus.unlocked =
        request.newMode === 'Plaintext' ||
        Boolean(request.newKey && request.newKey.trim())
      return structuredClone(mockState.snapshot) as T
    }
    case 'preview_rekey_archive':
      return buildMockRekeyPreview(
        mockState,
        structuredClone(args?.request as RekeyRequest),
      ) as T
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
        manifestHash: `preview-manifest-${nextRunId}`,
        profilesProcessed: mockState.snapshot.config.selectedProfileIds.filter(
          (profileId) => profileId.startsWith('chrome:'),
        ).length,
        newVisits: mockState.history.items.length,
        newUrls: uniqueUrlCount(mockState.history.items),
        newDownloads: 1,
      }
      mockState.snapshot.recentRuns = [run, ...mockState.snapshot.recentRuns]
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
            rawRows: 4,
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
    case 'restore_import_batch':
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
        label: 'dev.codex.pathkeep.backup',
        executablePath: '/Applications/PathKeep.app',
        generatedFiles: [],
        manualSteps: ['Tauri is not available in browser preview mode.'],
        applyCommands: [],
        rollbackCommands: [],
        applySupported: false,
      } as T
    case 'schedule_status':
      return buildMockScheduleStatus(mockState) as T
    case 'doctor_report':
      return {
        generatedAt: new Date().toISOString(),
        checks: [],
      } as T
    case 'repair_health':
      return {
        runId: 1,
        repairedImportAudits: 0,
        repairedVisibilityRows: 0,
        clearedDerivedRows: 0,
        notes: ['Tauri is not available in browser preview mode.'],
      } as T
    case 'preview_remote_backup':
      return {
        bundlePath: '/tmp/pathkeep-remote.zip',
        objectKey: 'pathkeep/pathkeep-remote.zip',
        uploadUrl:
          'https://s3.us-east-1.amazonaws.com/example-bucket/pathkeep/pathkeep-remote.zip',
        previewCommand:
          'curl --fail --show-error --aws-sigv4 "aws:amz:us-east-1:s3" --user "$S3_ACCESS_KEY_ID:$S3_SECRET_ACCESS_KEY" -T \'/tmp/pathkeep-remote.zip\' \'https://s3.us-east-1.amazonaws.com/example-bucket/pathkeep/pathkeep-remote.zip\'',
        manualSteps: ['Browser preview mode cannot generate the real bundle.'],
        warnings: [],
      } as T
    case 'run_remote_backup':
      return {
        uploaded: false,
        bundlePath: '/tmp/pathkeep-remote.zip',
        objectKey: 'pathkeep/pathkeep-remote.zip',
        uploadUrl:
          'https://s3.us-east-1.amazonaws.com/example-bucket/pathkeep/pathkeep-remote.zip',
        message: 'Remote backup upload is only available in the desktop app.',
      } as T
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
    case 'clear_s3_credentials':
      return {
        available: true,
        backend: 'Mock keyring',
        storedSecret: false,
      } as T
    case 'store_ai_provider_api_key':
    case 'clear_ai_provider_api_key':
      return structuredClone(mockState.snapshot) as T
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
        total: mockState.history.items.length,
        providerId: 'lexical-fallback',
        model: 'none',
        items: mockState.history.items.map((item, index) => ({
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
    case 'preview_ai_integrations':
      return {
        mcpCommand: '/Applications/PathKeep.app --worker mcp-server',
        manualSteps: [
          'Enable MCP or Skill integration in Settings first.',
          'Store the database key in the native keyring if the archive is encrypted.',
          'Copy the generated MCP JSON into your MCP client configuration.',
        ],
        generatedFiles: [
          {
            relativePath: 'integrations/pathkeep-mcp.json',
            absolutePath:
              '~/Library/Application Support/PathKeep/integrations/pathkeep-mcp.json',
            purpose: 'PathKeep MCP client snippet',
            contents: '{\n  "mcpServers": {}\n}',
          },
        ],
        warnings: [],
      } as T
    case 'export_history':
      return {
        format: (args?.request as ExportRequest)?.format ?? 'jsonl',
        path: `/tmp/pathkeep-export-${new Date()
          .toISOString()
          .replaceAll(
            ':',
            '-',
          )}.${((args?.request as ExportRequest)?.format ?? 'jsonl').replace('markdown', 'md')}`,
        count: filterMockHistory(
          mockState,
          (args?.request as ExportRequest)?.query,
        ).items.length,
      } as T
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
      throw new Error(`Mock backend does not implement ${command}`)
  }
}

export const backendTestHarness = {
  call,
  reset: () => {
    mockState = createMockState()
  },
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
  previewRekeyArchive: (request: RekeyRequest) =>
    call<RekeyPreview>('preview_rekey_archive', { request }),
  setSessionDatabaseKey: (databaseKey: string) =>
    call<void>('set_session_database_key', { databaseKey }),
  clearSessionDatabaseKey: () => call<void>('clear_session_database_key'),
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
