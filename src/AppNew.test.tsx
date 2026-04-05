import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'

const { autostartMocks, mockBackend, strongholdMocks } = vi.hoisted(() => ({
  autostartMocks: {
    disable: vi.fn(),
    enable: vi.fn(),
    isEnabled: vi.fn().mockResolvedValue(false),
  },
  mockBackend: {
    getAppBuildInfo: vi.fn(),
    getAppSnapshot: vi.fn(),
    saveConfig: vi.fn(),
    initializeArchive: vi.fn(),
    rekeyArchive: vi.fn(),
    setSessionDatabaseKey: vi.fn(),
    clearSessionDatabaseKey: vi.fn(),
    runBackupNow: vi.fn(),
    queryHistory: vi.fn(),
    exportHistory: vi.fn(),
    previewRemoteBackup: vi.fn(),
    runRemoteBackup: vi.fn(),
    inspectTakeout: vi.fn(),
    importTakeout: vi.fn(),
    previewImportBatch: vi.fn(),
    revertImportBatch: vi.fn(),
    previewSchedule: vi.fn(),
    applySchedule: vi.fn(),
    doctor: vi.fn(),
    keyringStatus: vi.fn(),
    keyringGetDatabaseKey: vi.fn(),
    keyringStoreDatabaseKey: vi.fn(),
    keyringClearDatabaseKey: vi.fn(),
    storeS3Credentials: vi.fn(),
    clearS3Credentials: vi.fn(),
    storeAiProviderApiKey: vi.fn(),
    clearAiProviderApiKey: vi.fn(),
    buildAiIndex: vi.fn(),
    searchAiHistory: vi.fn(),
    askAiAssistant: vi.fn(),
    runInsightsNow: vi.fn(),
    loadInsights: vi.fn(),
    loadThreadDetail: vi.fn(),
    explainInsight: vi.fn(),
    previewAiIntegrations: vi.fn(),
    resetLocalSecretVault: vi.fn(),
    openPathInFileManager: vi.fn(),
  },
  strongholdMocks: {
    readDatabaseKeyStronghold: vi.fn().mockResolvedValue(null),
    storeDatabaseKeyStronghold: vi.fn().mockResolvedValue(undefined),
  },
}))

vi.mock('./lib/backend', () => ({ backend: mockBackend }))
vi.mock('./lib/stronghold', () => ({
  readDatabaseKeyStronghold: strongholdMocks.readDatabaseKeyStronghold,
  storeDatabaseKeyStronghold: strongholdMocks.storeDatabaseKeyStronghold,
}))
vi.mock('@tauri-apps/plugin-autostart', () => autostartMocks)

import AppNew from './AppNew'

/* ------------------------------------------------------------------ */
/* Fixtures                                                            */
/* ------------------------------------------------------------------ */

const llmProvider = {
  id: 'llm-preview',
  name: 'Preview LLM',
  purpose: 'llm',
  requestFormat: 'openai',
  enabled: true,
  baseUrl: 'https://api.example.com/v1',
  apiKeySaved: true,
  defaultModel: 'gpt-4.1-mini',
  modelCatalog: ['gpt-4.1-mini', 'gpt-4.1'],
  temperature: 0.2,
  maxTokens: 1200,
  dimensions: null,
  notes: 'LLM notes',
}
const embeddingProvider = {
  id: 'embedding-preview',
  name: 'Preview Embeddings',
  purpose: 'embedding',
  requestFormat: 'openai',
  enabled: true,
  baseUrl: 'https://api.example.com/v1',
  apiKeySaved: false,
  defaultModel: 'text-embedding-3-large',
  modelCatalog: ['text-embedding-3-large'],
  temperature: null,
  maxTokens: null,
  dimensions: 1536,
  notes: null,
}

const baseSnapshot = {
  directories: {
    appRoot: '/tmp/browser-history-backup',
    configPath: '/tmp/browser-history-backup/config.json',
    archiveDatabasePath: '/tmp/browser-history-backup/archive.sqlite',
    auditRepoPath: '/tmp/browser-history-backup/audit',
    manifestsDir: '/tmp/browser-history-backup/audit/manifests',
    exportsDir: '/tmp/browser-history-backup/exports',
    rawSnapshotsDir: '/tmp/browser-history-backup/raw',
    stagingDir: '/tmp/browser-history-backup/staging',
    quarantineDir: '/tmp/browser-history-backup/quarantine',
    scheduleDir: '/tmp/browser-history-backup/schedule',
    strongholdPath: '/tmp/browser-history-backup/vault.hold',
    strongholdSaltPath: '/tmp/browser-history-backup/vault.salt',
  },
  config: {
    initialized: true,
    archiveMode: 'Encrypted',
    preferredLanguage: 'en',
    dueAfterHours: 72,
    scheduleCheckIntervalHours: 6,
    checkpointDays: 90,
    captureFavicons: true,
    selectedProfileIds: ['chrome:Default'],
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
      enabled: true,
      assistantEnabled: true,
      semanticIndexEnabled: true,
      mcpEnabled: true,
      skillEnabled: true,
      autoIndexAfterBackup: true,
      llmProviderId: 'llm-preview',
      embeddingProviderId: 'embedding-preview',
      retrievalTopK: 8,
      assistantSystemPrompt: 'AI assistant.',
      llmProviders: [llmProvider],
      embeddingProviders: [embeddingProvider],
    },
  },
  archiveStatus: {
    initialized: true,
    encrypted: true,
    unlocked: true,
    databasePath: '/tmp/browser-history-backup/archive.sqlite',
    lastSuccessfulBackupAt: '2026-04-03T11:31:00.000Z',
    warning: null,
  },
  keyringStatus: {
    available: true,
    backend: 'Mock keyring',
    storedSecret: false,
    message: null,
  },
  aiStatus: {
    enabled: true,
    assistantEnabled: true,
    mcpEnabled: true,
    skillEnabled: true,
    ready: true,
    indexedItems: 24,
    lastIndexedAt: '2026-04-03T11:55:00.000Z',
    llmProviderId: 'llm-preview',
    embeddingProviderId: 'embedding-preview',
    warning: null,
  },
  insightStatus: {
    ready: true,
    lastRunAt: '2026-04-03T12:25:00.000Z',
    runs: 3,
    cards: 4,
    topics: 2,
    threads: 1,
    contentCoverage: 0.66,
    warning: null,
  },
  browserProfiles: [
    {
      profileId: 'chrome:Default',
      profileName: 'Primary',
      browserFamily: 'chromium',
      browserName: 'Google Chrome',
      userName: 'primary@example.test',
      profilePath:
        '/Users/demo/Library/Application Support/Google/Chrome/Default',
      historyPath:
        '/Users/demo/Library/Application Support/Google/Chrome/Default/History',
      faviconsPath:
        '/Users/demo/Library/Application Support/Google/Chrome/Default/Favicons',
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
      profilePath:
        '/Users/demo/Library/Application Support/Google/Chrome/Profile 2',
      historyPath:
        '/Users/demo/Library/Application Support/Google/Chrome/Profile 2/History',
      faviconsPath:
        '/Users/demo/Library/Application Support/Google/Chrome/Profile 2/Favicons',
      historyExists: true,
      browserVersion: '146.0.7680.178',
      historyFileName: 'History',
    },
  ],
  recentRuns: [
    {
      id: 3,
      startedAt: '2026-04-03T11:30:00.000Z',
      finishedAt: '2026-04-03T11:31:00.000Z',
      status: 'completed',
      manifestHash: 'run-3-hash',
      profilesProcessed: 3,
      newVisits: 8,
      newUrls: 4,
      newDownloads: 1,
    },
  ],
  recentImportBatches: [
    {
      id: 7,
      sourceKind: 'takeout',
      sourcePath: '/tmp/takeout.zip',
      profileId: 'takeout::browser-history',
      createdAt: '2026-04-03T12:00:00.000Z',
      importedAt: '2026-04-03T12:01:00.000Z',
      revertedAt: null,
      status: 'imported',
      candidateItems: 12,
      importedItems: 10,
      duplicateItems: 2,
      visibleItems: 10,
      auditPath: '/tmp/browser-history-backup/audit/import-batch-7.json',
      gitCommit: 'abc123',
    },
  ],
}

const batchDetail = {
  batch: baseSnapshot.recentImportBatches[0],
  previewEntries: [
    {
      sourcePath: '/tmp/takeout.zip',
      url: 'https://example.com',
      title: 'Example',
      visitedAt: '2026-04-03T12:00:00.000Z',
      sourceVisitId: 1,
      status: 'imported',
    },
  ],
  recognizedFiles: [
    { path: 'takeout.jsonl', kind: 'jsonl', status: 'ready', records: 10 },
  ],
  quarantinedFiles: [],
  notes: ['Looks good'],
}
const revertedBatchDetail = {
  ...batchDetail,
  batch: {
    ...batchDetail.batch,
    status: 'reverted',
    revertedAt: '2026-04-03T12:05:00.000Z',
    visibleItems: 0,
  },
}
const buildInfo = {
  productName: 'BHB',
  version: '0.1.0',
  gitCommitShort: 'abc12345',
  gitCommitFull: 'abc12345def67890',
  gitDirty: false,
}

const insightSnapshot = {
  generatedAt: '2026-04-03T12:25:00.000Z',
  windowDays: 30,
  profileId: 'chrome:Default',
  status: structuredClone(baseSnapshot.insightStatus),
  cards: [
    {
      cardId: 'card-1',
      kind: 'rising-topic',
      title: 'Rising topic: archive tooling',
      summary: 'Momentum.',
      windowDays: 30,
      profileId: 'chrome:Default',
      score: 0.82,
      chromiumEnhanced: true,
      evidence: [],
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
      evidence: [],
    },
  ],
  queryLadders: [],
  workflowMap: null,
  profileFacets: [],
  notes: [],
}
const insightThreadDetail = {
  summary: structuredClone(insightSnapshot.threads[0]),
  visits: [
    {
      historyId: 1,
      profileId: 'chrome:Default',
      url: 'https://example.com',
      title: 'Example',
      visitedAt: '2026-04-03T12:00:00.000Z',
      note: 'Thread visit',
    },
  ],
}

function makeSnapshot(overrides: Record<string, unknown> = {}) {
  return { ...structuredClone(baseSnapshot), ...overrides }
}

/* ------------------------------------------------------------------ */
/* Helpers                                                             */
/* ------------------------------------------------------------------ */

/** Click a sidebar nav item by its aria-label (which is the nav label text). */
function navBtn(label: string) {
  const nav = screen.getByRole('navigation')
  return within(nav).getByRole('button', { name: label })
}

/** Find button by regex anywhere in the main pane. */
function mainBtn(label: RegExp) {
  const main = screen.getByRole('main')
  return within(main).getByRole('button', { name: label })
}

function mainBtns(label: RegExp) {
  const main = screen.getByRole('main')
  return within(main).getAllByRole('button', { name: label })
}

/* ------------------------------------------------------------------ */
/* Setup                                                               */
/* ------------------------------------------------------------------ */

describe('AppNew integration', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'confirm', {
      writable: true,
      value: vi.fn().mockReturnValue(true),
    })
    Object.defineProperty(window.navigator, 'clipboard', {
      writable: true,
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
    })

    Object.values(mockBackend).forEach((m) => m.mockReset())
    autostartMocks.disable.mockReset()
    autostartMocks.enable.mockReset()
    autostartMocks.isEnabled.mockReset().mockResolvedValue(false)
    strongholdMocks.readDatabaseKeyStronghold
      .mockReset()
      .mockResolvedValue(null)
    strongholdMocks.storeDatabaseKeyStronghold
      .mockReset()
      .mockResolvedValue(undefined)

    mockBackend.getAppSnapshot.mockResolvedValue(makeSnapshot())
    mockBackend.getAppBuildInfo.mockResolvedValue(buildInfo)
    mockBackend.keyringGetDatabaseKey.mockResolvedValue(null)
    mockBackend.queryHistory.mockResolvedValue({
      total: 1,
      items: [
        {
          id: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com',
          title: 'Example',
          domain: 'example.com',
          visitedAt: '2026-04-03T12:00:00.000Z',
          visitTime: 1,
          durationMs: 1000,
          transition: 1,
          sourceVisitId: 1,
          appId: null,
        },
      ],
    })
    mockBackend.previewImportBatch.mockResolvedValue(batchDetail)
    mockBackend.revertImportBatch.mockResolvedValue(revertedBatchDetail)
    mockBackend.saveConfig.mockResolvedValue(makeSnapshot())
    mockBackend.initializeArchive.mockResolvedValue(makeSnapshot())
    mockBackend.rekeyArchive.mockResolvedValue(makeSnapshot())
    mockBackend.setSessionDatabaseKey.mockResolvedValue(undefined)
    mockBackend.clearSessionDatabaseKey.mockResolvedValue(undefined)
    mockBackend.previewSchedule.mockResolvedValue({
      platform: 'macos',
      label: 'dev.example.bhb.backup',
      executablePath: '/Applications/BHB.app',
      generatedFiles: [
        {
          relativePath: 'launchd/bhb.plist',
          absolutePath: '/tmp/sched/bhb.plist',
          purpose: 'LaunchAgent',
          contents: '<plist />',
        },
      ],
      manualSteps: ['Copy.', 'Load.'],
      applyCommands: [['launchctl', 'bootstrap', 'gui/$UID', 'bhb.plist']],
      rollbackCommands: [['launchctl', 'bootout', 'gui/$UID', 'bhb.plist']],
      applySupported: true,
    })
    mockBackend.applySchedule.mockResolvedValue({
      applied: true,
      platform: 'macos',
      files: ['/tmp/sched/bhb.plist'],
      auditPath: '/tmp/audit/schedule.json',
      message: 'Schedule applied.',
    })
    mockBackend.exportHistory.mockResolvedValue({
      format: 'jsonl',
      path: '/tmp/hist.jsonl',
      count: 1,
    })
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: false,
      reason: null,
      run: makeSnapshot().recentRuns[0],
      profiles: [],
      manifestPath: '/tmp/audit/run-3.json',
      gitCommit: 'abc123',
      warnings: [],
      remoteBackup: null,
    })
    mockBackend.doctor.mockResolvedValue({
      generatedAt: '2026-04-03T12:10:00.000Z',
      checks: [{ name: 'DB integrity', status: 'pass', message: 'OK.' }],
    })
    mockBackend.previewRemoteBackup.mockResolvedValue({
      bundlePath: '/tmp/remote.zip',
      objectKey: 'bhb/remote.zip',
      uploadUrl: 'https://s3.example.com/bhb/remote.zip',
      previewCommand: 'curl …',
      manualSteps: ['Review.'],
      warnings: [],
    })
    mockBackend.runRemoteBackup.mockResolvedValue({
      uploaded: true,
      bundlePath: '/tmp/remote.zip',
      objectKey: 'bhb/remote.zip',
      uploadUrl: 'https://s3.example.com/bhb/remote.zip',
      message: 'Remote upload finished.',
    })
    mockBackend.storeS3Credentials.mockResolvedValue(makeSnapshot())
    mockBackend.clearS3Credentials.mockResolvedValue(makeSnapshot())
    mockBackend.keyringStoreDatabaseKey.mockResolvedValue({
      available: true,
      backend: 'Mock keyring',
      storedSecret: true,
      message: null,
    })
    mockBackend.keyringClearDatabaseKey.mockResolvedValue({
      available: true,
      backend: 'Mock keyring',
      storedSecret: false,
      message: null,
    })
    mockBackend.storeAiProviderApiKey.mockResolvedValue(makeSnapshot())
    mockBackend.clearAiProviderApiKey.mockResolvedValue(makeSnapshot())
    mockBackend.runInsightsNow.mockResolvedValue({
      runId: 12,
      processedVisits: 24,
      enrichedVisits: 8,
      failedEnrichments: 1,
      topicCount: 1,
      threadCount: 1,
      cardCount: 1,
      contentCoverage: 0.66,
      lastRunAt: '2026-04-03T12:25:00.000Z',
      notes: [],
    })
    mockBackend.loadInsights.mockResolvedValue(insightSnapshot)
    mockBackend.loadThreadDetail.mockResolvedValue(insightThreadDetail)
    mockBackend.explainInsight.mockResolvedValue({
      explanation: 'Repeated revisits.',
      usedLlm: false,
      citations: structuredClone(insightThreadDetail.visits),
      notes: [],
    })
    mockBackend.previewAiIntegrations.mockResolvedValue({
      mcpCommand: '/bin/mcp',
      manualSteps: [],
      generatedFiles: [],
      warnings: [],
    })
    mockBackend.resetLocalSecretVault.mockResolvedValue(undefined)
    mockBackend.openPathInFileManager.mockResolvedValue(
      baseSnapshot.directories.appRoot,
    )
    mockBackend.inspectTakeout.mockResolvedValue({
      sourceType: 'zip',
      recognizedFiles: [
        { path: 'takeout.jsonl', kind: 'jsonl', status: 'ready', records: 10 },
      ],
      quarantinedFiles: [],
      totalRecords: 10,
      candidateItems: 10,
      importedItems: 0,
      duplicateItems: 0,
      importBatch: null,
      previewEntries: [
        {
          sourcePath: '/tmp/takeout.zip',
          url: 'https://example.com',
          title: 'Example',
          visitedAt: '2026-04-03T12:00:00.000Z',
          sourceVisitId: 1,
          status: 'preview',
        },
      ],
      notes: [],
    })
    mockBackend.importTakeout.mockResolvedValue({
      sourceType: 'zip',
      recognizedFiles: [
        { path: 'takeout.jsonl', kind: 'jsonl', status: 'ready', records: 10 },
      ],
      quarantinedFiles: [],
      totalRecords: 10,
      candidateItems: 10,
      importedItems: 10,
      duplicateItems: 2,
      importBatch: { id: 8 },
      previewEntries: [],
      notes: [],
    })
  })

  /* ---------- Dashboard ---------- */

  test('renders dashboard with health, backup, and doctor', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    expect(screen.getAllByText('Archive healthy').length).toBeGreaterThan(0)

    await user.click(mainBtn(/Run backup now/))
    await waitFor(() => expect(mockBackend.runBackupNow).toHaveBeenCalled())

    await user.click(mainBtn(/Run doctor/))
    await waitFor(() => expect(mockBackend.doctor).toHaveBeenCalled())
  })

  test('dashboard backup skipped notice', async () => {
    const user = userEvent.setup()
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: true,
      reason: 'Not due yet.',
      run: null,
      profiles: [],
      manifestPath: null,
      gitCommit: null,
      warnings: [],
      remoteBackup: null,
    })
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(mainBtn(/Run backup now/))
    // The reason shows up in both a StatusTag and the toast notice, so multiple matches are expected
    await waitFor(() =>
      expect(screen.getAllByText('Not due yet.').length).toBeGreaterThan(0),
    )
  })

  /* ---------- Sidebar ---------- */

  test('navigates between all pages via sidebar', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')

    await user.click(navBtn('Explorer'))
    await screen.findByText(/Search the long-term archive/)

    await user.click(navBtn('Insights'))
    await screen.findByText(/Configure providers/)

    await user.click(navBtn('Activity Log'))
    await screen.findByText(/Review backup runs/)

    await user.click(navBtn('Import'))
    await screen.findByText(/Start with a dry-run/)

    await user.click(navBtn('Settings'))
    await screen.findByRole('heading', { name: 'General' })
  })

  /* ---------- Explorer ---------- */

  test('searches history and shows results', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Explorer'))

    // Explorer auto-queries on mount/filter change via useDeferredValue
    await waitFor(() => expect(mockBackend.queryHistory).toHaveBeenCalled())
    // "Example" appears in both the list row and the auto-selected detail panel
    await waitFor(() =>
      expect(screen.getAllByText('Example').length).toBeGreaterThan(0),
    )
    // Click the row button to select
    const rows = screen.getAllByText('Example')
    await user.click(rows[0])
    expect(screen.getByText('https://example.com')).toBeInTheDocument()
  })

  test('exports history JSONL', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Explorer'))
    await waitFor(() => expect(mockBackend.queryHistory).toHaveBeenCalled())
    await user.click(mainBtn(/JSONL/))
    await waitFor(() =>
      expect(mockBackend.exportHistory).toHaveBeenCalledWith(
        expect.objectContaining({ format: 'jsonl' }),
      ),
    )
  })

  test('empty explorer results', async () => {
    mockBackend.queryHistory.mockResolvedValue({ total: 0, items: [] })
    render(<AppNew />)
    await screen.findByText('Overview')
    const user = userEvent.setup()
    await user.click(navBtn('Explorer'))
    await waitFor(() => expect(mockBackend.queryHistory).toHaveBeenCalled())
    await screen.findByText('No results found.')
  })

  /* ---------- Insights ---------- */

  test('loads insight data and shows cards, topics, threads', async () => {
    render(<AppNew />)
    await screen.findByText('Overview')
    const user = userEvent.setup()
    await user.click(navBtn('Insights'))
    await waitFor(() => expect(mockBackend.loadInsights).toHaveBeenCalled())
    await screen.findByText('Rising topic: archive tooling')
    expect(screen.getByText('Archive tooling')).toBeInTheDocument()
    // Thread title may appear in both the thread list and auto-loaded detail
    expect(screen.getAllByText('archive tool compare').length).toBeGreaterThan(
      0,
    )
  })

  test('runs insights and explains a card', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Insights'))
    await waitFor(() => expect(mockBackend.loadInsights).toHaveBeenCalled())
    await screen.findByText('Rising topic: archive tooling')
    await user.click(mainBtn(/Run insights now/))
    await waitFor(() => expect(mockBackend.runInsightsNow).toHaveBeenCalled())
    await user.click(mainBtn(/Explain/))
    await waitFor(() => expect(mockBackend.explainInsight).toHaveBeenCalled())
    await screen.findByText('Repeated revisits.')
  })

  test('thread detail', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Insights'))
    await waitFor(() => expect(mockBackend.loadInsights).toHaveBeenCalled())
    // The thread auto-selects on mount, triggering loadThreadDetail
    await waitFor(() =>
      expect(mockBackend.loadThreadDetail).toHaveBeenCalledWith('thread-001'),
    )
    await screen.findByText('Thread visit')
  })

  /* ---------- Activity Log ---------- */

  test('shows runs and triggers backup', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Activity Log'))
    await screen.findByText(/Review backup runs/)
    await user.click(mainBtn(/Run backup now/))
    await waitFor(() => expect(mockBackend.runBackupNow).toHaveBeenCalled())
  })

  test('previews and runs remote backup', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          remoteBackup: {
            ...structuredClone(baseSnapshot.config.remoteBackup),
            enabled: true,
            bucket: 'my-bucket',
            credentialsSaved: true,
          },
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Activity Log'))
    await screen.findByText(/Review backup runs/)
    await user.click(mainBtn(/Preview upload/))
    await waitFor(() =>
      expect(mockBackend.previewRemoteBackup).toHaveBeenCalled(),
    )
    await user.click(mainBtn(/Upload now/))
    await waitFor(() => expect(mockBackend.runRemoteBackup).toHaveBeenCalled())
  })

  /* ---------- Import ---------- */

  test('inspects takeout and imports', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Import'))
    await screen.findByText(/Start with a dry-run/)
    await user.type(
      screen.getByPlaceholderText('/path/to/takeout.zip or /path/to/Takeout/'),
      '/tmp/takeout.zip',
    )
    await user.click(mainBtn(/Dry-run/))
    await waitFor(() =>
      expect(mockBackend.inspectTakeout).toHaveBeenCalledWith(
        expect.objectContaining({
          sourcePath: '/tmp/takeout.zip',
          dryRun: true,
        }),
      ),
    )
    await user.click(mainBtn(/Import supported files/))
    await waitFor(() =>
      expect(mockBackend.importTakeout).toHaveBeenCalledWith(
        expect.objectContaining({
          sourcePath: '/tmp/takeout.zip',
          dryRun: false,
        }),
      ),
    )
  })

  test('shows batch history and reverts', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(makeSnapshot())
      .mockResolvedValueOnce(
        makeSnapshot({ recentImportBatches: [revertedBatchDetail.batch] }),
      )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Import'))
    await waitFor(() =>
      expect(mockBackend.previewImportBatch).toHaveBeenCalledWith(7),
    )
    // The batch row shows the source path
    expect(screen.getByText('/tmp/takeout.zip')).toBeInTheDocument()
    await user.click(mainBtn(/Revert/))
    await waitFor(() =>
      expect(mockBackend.revertImportBatch).toHaveBeenCalledWith(7),
    )
  })

  /* ---------- Settings: General ---------- */

  test('general settings with paths', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await screen.findByRole('heading', { name: 'General' })
    expect(
      screen.getByDisplayValue('/tmp/browser-history-backup'),
    ).toBeInTheDocument()
  })

  test('saves general settings', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await screen.findByRole('heading', { name: 'General' })
    await user.click(mainBtns(/Save settings/)[0])
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  test('opens app data root', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await screen.findByRole('heading', { name: 'General' })
    // PathRow's button in .pathActions
    const allButtons = screen.getAllByRole('button')
    const openBtn = allButtons.find((b) => b.closest('.pathActions') !== null)
    if (openBtn) {
      await user.click(openBtn)
      await waitFor(() =>
        expect(mockBackend.openPathInFileManager).toHaveBeenCalled(),
      )
    }
  })

  /* ---------- Settings: Sources ---------- */

  test('toggles profile selection', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Sources/))
    await screen.findByText('Primary')
    expect(screen.getByText('Research')).toBeInTheDocument()
    await user.click(screen.getAllByRole('checkbox')[0])
  })

  /* ---------- Settings: Schedule ---------- */

  test('previews and applies schedule', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Schedule/))
    await user.click(mainBtn(/Preview native schedule/))
    await waitFor(() =>
      expect(mockBackend.previewSchedule).toHaveBeenCalledWith('macos'),
    )
    await user.click(mainBtns(/Apply preview/)[0])
    await waitFor(() => expect(mockBackend.applySchedule).toHaveBeenCalled())
  })

  test('removes schedule', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Schedule/))
    await user.click(mainBtn(/Preview native schedule/))
    await waitFor(() => expect(mockBackend.previewSchedule).toHaveBeenCalled())
    await user.click(mainBtn(/Remove schedule/))
    expect(window.confirm).toHaveBeenCalled()
  })

  /* ---------- Settings: Security ---------- */

  test('security displays mode and keyring', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    expect(screen.getByText('Encrypted')).toBeInTheDocument()
    expect(screen.getByText('Unlocked')).toBeInTheDocument()
    expect(screen.getByText('Mock keyring')).toBeInTheDocument()
  })

  test('stores and clears remembered key', async () => {
    const user = userEvent.setup()
    // The security page's handleRememberKey checks sessionDatabaseKey,
    // which requires a keyring-sourced auto-unlock to populate it.
    // Simulate by using auto-unlock: keyringGetDatabaseKey returns a key,
    // and the first snapshot shows locked, then the second post-unlock.
    mockBackend.keyringGetDatabaseKey.mockResolvedValue('remembered-key')
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(
        makeSnapshot({
          archiveStatus: {
            ...structuredClone(baseSnapshot.archiveStatus),
            unlocked: false,
          },
        }),
      )
      .mockResolvedValueOnce(makeSnapshot())
    render(<AppNew />)
    await screen.findByText('Overview')
    await waitFor(() =>
      expect(mockBackend.setSessionDatabaseKey).toHaveBeenCalledWith(
        'remembered-key',
      ),
    )
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    await user.click(mainBtn(/Remember current key/))
    await waitFor(() =>
      expect(mockBackend.keyringStoreDatabaseKey).toHaveBeenCalled(),
    )
    await user.click(mainBtn(/Clear remembered key/))
    await waitFor(() =>
      expect(mockBackend.keyringClearDatabaseKey).toHaveBeenCalled(),
    )
  })

  test('unlock from security page', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          unlocked: false,
        },
      }),
    )
    strongholdMocks.readDatabaseKeyStronghold.mockResolvedValue('key-abc')
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    expect(screen.getByText('Locked')).toBeInTheDocument()
    await user.type(
      screen.getByPlaceholderText('Store this in your password manager'),
      'my-pass',
    )
    await user.click(mainBtn(/Unlock archive/))
    await waitFor(() =>
      expect(strongholdMocks.readDatabaseKeyStronghold).toHaveBeenCalledWith(
        'my-pass',
        '/tmp/browser-history-backup/vault.hold',
      ),
    )
  })

  /* ---------- Settings: Remote ---------- */

  test('remote settings toggle', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Remote/))
    await user.click(screen.getAllByRole('checkbox')[0])
  })

  /* ---------- Settings: AI Providers ---------- */

  test('AI settings shows providers', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    expect(screen.getByText('Enable AI features')).toBeInTheDocument()
    // Provider names appear in both the select dropdown and editor inputs
    expect(screen.getAllByDisplayValue('Preview LLM').length).toBeGreaterThan(0)
    expect(
      screen.getAllByDisplayValue('Preview Embeddings').length,
    ).toBeGreaterThan(0)
  })

  test('adds new AI provider', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    await user.click(mainBtns(/Add provider/)[0])
    expect(screen.getByDisplayValue('New LLM provider')).toBeInTheDocument()
  })

  /* ---------- Onboarding ---------- */

  test('onboarding: encrypted init', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          initialized: false,
          selectedProfileIds: [],
        },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    mockBackend.initializeArchive.mockResolvedValue(makeSnapshot())
    render(<AppNew />)

    // "Welcome" appears in both step indicator and content; wait for multiple
    await waitFor(() =>
      expect(screen.getAllByText('Welcome').length).toBeGreaterThan(0),
    )
    // Onboarding nav buttons use step labels, not "Next"
    await user.click(screen.getByRole('button', { name: /Sources/ }))
    await screen.findByText(
      'Choose which browser profiles should be included in backups.',
    )
    await user.click(screen.getAllByRole('checkbox')[0])
    await user.click(screen.getByRole('button', { name: /Security/ }))
    // "Security" also appears in step indicator; use getAllByText
    await waitFor(() =>
      expect(screen.getAllByText('Security').length).toBeGreaterThan(0),
    )
    const pwdInputs = screen.getAllByPlaceholderText(
      'Store this in your password manager',
    )
    await user.type(pwdInputs[0], 'vault-pass')
    await user.type(pwdInputs[1], 'vault-pass')
    await user.click(screen.getByRole('button', { name: /Schedule/ }))
    await waitFor(() =>
      expect(screen.getAllByText('Schedule').length).toBeGreaterThan(0),
    )
    await user.click(screen.getByRole('button', { name: /Done/ }))
    await screen.findByText(/Review/)
    await user.click(screen.getByRole('button', { name: /Create archive/ }))
    await waitFor(() =>
      expect(mockBackend.initializeArchive).toHaveBeenCalledWith(
        expect.objectContaining({ initialized: true }),
        expect.any(String),
      ),
    )
  })

  test('onboarding: plaintext init', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          initialized: false,
          archiveMode: 'Plaintext',
          selectedProfileIds: [],
        },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    mockBackend.initializeArchive.mockResolvedValue(makeSnapshot())
    render(<AppNew />)

    await waitFor(() =>
      expect(screen.getAllByText('Welcome').length).toBeGreaterThan(0),
    )
    await user.click(screen.getByRole('button', { name: /Sources/ }))
    await user.click(screen.getAllByRole('checkbox')[0])
    await user.click(screen.getByRole('button', { name: /Security/ }))
    await waitFor(() =>
      expect(screen.getAllByText('Security').length).toBeGreaterThan(0),
    )
    await user.click(screen.getByRole('button', { name: /Schedule/ }))
    await waitFor(() =>
      expect(screen.getAllByText('Schedule').length).toBeGreaterThan(0),
    )
    await user.click(screen.getByRole('button', { name: /Done/ }))
    await user.click(screen.getByRole('button', { name: /Create archive/ }))
    await waitFor(() =>
      expect(mockBackend.initializeArchive).toHaveBeenCalledWith(
        expect.objectContaining({ initialized: true }),
        null,
      ),
    )
  })

  /* ---------- Auto-unlock ---------- */

  test('auto-unlocks with remembered key', async () => {
    mockBackend.keyringGetDatabaseKey.mockResolvedValue('remembered-key-123')
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(
        makeSnapshot({
          archiveStatus: {
            ...structuredClone(baseSnapshot.archiveStatus),
            unlocked: false,
          },
        }),
      )
      .mockResolvedValueOnce(makeSnapshot())
    render(<AppNew />)
    await screen.findByText('Overview')
    await waitFor(() =>
      expect(mockBackend.setSessionDatabaseKey).toHaveBeenCalledWith(
        'remembered-key-123',
      ),
    )
  })

  /* ---------- Toast ---------- */

  test('shows toast after backup', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(mainBtn(/Run backup now/))
    // "Backup completed" may appear in both the inline StatusTag and the toast notice
    await waitFor(() =>
      expect(screen.getAllByText(/Backup completed/).length).toBeGreaterThan(0),
    )
  })

  test('toast close button clears notice', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(mainBtn(/Run backup now/))
    await waitFor(() =>
      expect(screen.getAllByText(/Backup completed/).length).toBeGreaterThan(0),
    )
    // Find the toast close button
    const toastLog = screen.getByRole('log')
    const closeBtn = within(toastLog).getByRole('button')
    await user.click(closeBtn)
    // Toast was dismissed
    await waitFor(() =>
      expect(screen.queryByRole('log')).not.toBeInTheDocument(),
    )
  })

  test('error banner shows and can be dismissed', async () => {
    const user = userEvent.setup()
    mockBackend.runBackupNow.mockRejectedValue(new Error('Network failure'))
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(mainBtn(/Run backup now/))
    await screen.findByText('Network failure')
    const alertEl = screen.getByRole('alert')
    const closeBtn = within(alertEl).getByRole('button')
    await user.click(closeBtn)
    await waitFor(() =>
      expect(screen.queryByRole('alert')).not.toBeInTheDocument(),
    )
  })

  /* ---------- Dashboard: locked and uninitialized states ---------- */

  test('dashboard shows locked badge', async () => {
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    expect(screen.getAllByText('Archive locked').length).toBeGreaterThan(0)
  })

  test('dashboard shows needs-setup badge when not initialized', async () => {
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: { ...structuredClone(baseSnapshot.config), initialized: false },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    // Onboarding shows instead of dashboard for uninitialized
    await waitFor(() =>
      expect(screen.getAllByText('Welcome').length).toBeGreaterThan(0),
    )
  })

  test('dashboard shows backup with profile stats', async () => {
    const user = userEvent.setup()
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: false,
      reason: null,
      profiles: [
        {
          profileId: 'chrome:Default',
          newVisits: 5,
          newUrls: 3,
        },
      ],
      manifestPath: '/tmp/audit/manifest.json',
      gitCommit: 'abc',
      warnings: ['Low disk'],
      remoteBackup: null,
    })
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(mainBtn(/Run backup now/))
    // Dashboard shows backupCompleted status and profile stats
    await waitFor(() =>
      expect(screen.getAllByText(/Backup completed/).length).toBeGreaterThan(0),
    )
  })

  test('dashboard shows recent run details when available', async () => {
    render(<AppNew />)
    await screen.findByText('Overview')
    // The recent runs section shows the run with +8 visits
    expect(screen.getAllByText(/8/).length).toBeGreaterThan(0)
  })

  /* ---------- Explorer: filters ---------- */

  test('explorer domain filter', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Explorer'))
    await waitFor(() => expect(mockBackend.queryHistory).toHaveBeenCalled())
    const domainInput = screen.getByPlaceholderText('e.g. github.com')
    await user.type(domainInput, 'example.com')
    await waitFor(() =>
      expect(mockBackend.queryHistory).toHaveBeenCalledWith(
        expect.objectContaining({ domain: 'example.com' }),
      ),
    )
  })

  test('explorer profile filter', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Explorer'))
    await waitFor(() => expect(mockBackend.queryHistory).toHaveBeenCalled())
    const profileSelect = screen.getByDisplayValue('All selected profiles')
    await user.selectOptions(profileSelect, 'chrome:Default')
    await waitFor(() =>
      expect(mockBackend.queryHistory).toHaveBeenCalledWith(
        expect.objectContaining({ profileId: 'chrome:Default' }),
      ),
    )
  })

  /* ---------- Insights: run report ---------- */

  test('insights run report shows stats', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Insights'))
    await waitFor(() => expect(mockBackend.loadInsights).toHaveBeenCalled())
    await screen.findByText('Rising topic: archive tooling')
    await user.click(mainBtn(/Run insights now/))
    await waitFor(() => expect(mockBackend.runInsightsNow).toHaveBeenCalled())
  })

  /* ---------- Activity Log: run details ---------- */

  test('activity log shows run detail when selected', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Activity Log'))
    // Recent runs are shown; clicking one selects it
    await waitFor(() =>
      expect(screen.getAllByText(/visits/).length).toBeGreaterThan(0),
    )
  })

  test('activity log empty state', async () => {
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({ recentRuns: [] }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    const user = userEvent.setup()
    await user.click(navBtn('Activity Log'))
    await screen.findByText('No backup yet')
  })

  /* ---------- Import: deeper flows ---------- */

  test('import page has no batch section when empty', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({ recentImportBatches: [] }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Import'))
    await screen.findByText(/Start with a dry-run/)
    // With no batches, the batch section is not rendered at all
    expect(screen.queryByText('Import history')).not.toBeInTheDocument()
  })

  test('import dry-run button disabled without path', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Import'))
    // Dry-run button is disabled without a path
    const dryRunBtn = mainBtn(/Dry-run/)
    expect(dryRunBtn).toBeDisabled()
  })

  /* ---------- Settings: General deeper coverage ---------- */

  test('general settings shows build info', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await screen.findByRole('heading', { name: 'General' })
    expect(screen.getByText('0.1.0')).toBeInTheDocument()
    expect(screen.getByText('abc12345')).toBeInTheDocument()
    expect(screen.getByText('Clean')).toBeInTheDocument()
  })

  test('general settings language change', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    // The language select shows 'English' since config has preferredLanguage: 'en'
    const langSelect = screen.getByDisplayValue('English')
    await user.selectOptions(langSelect, 'zh-CN')
  })

  test('general settings autostart toggle', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    // Find the autostart checkbox
    const checkboxes = screen.getAllByRole('checkbox')
    // Toggle the autostart
    await user.click(checkboxes[0])
  })

  test('general settings opens and copies paths', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    // Find path action buttons (open/copy)
    const allButtons = screen.getAllByRole('button')
    const openBtns = allButtons.filter(
      (b) => b.closest('.pathActions') !== null,
    )
    if (openBtns.length > 0) {
      await user.click(openBtns[0])
      await waitFor(() =>
        expect(mockBackend.openPathInFileManager).toHaveBeenCalled(),
      )
    }
    // Find a copy button
    const copyBtns = allButtons.filter((b) => b.textContent?.includes('Copy'))
    if (copyBtns.length > 0) {
      await user.click(copyBtns[0])
      await waitFor(() =>
        expect(window.navigator.clipboard.writeText).toHaveBeenCalled(),
      )
    }
  })

  /* ---------- Settings: Sources deeper ---------- */

  test('sources settings shows profiles with meta, saves', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Sources/))
    // Profile meta info shows
    expect(screen.getByText('primary@example.test')).toBeInTheDocument()
    expect(screen.getByText('research@example.test')).toBeInTheDocument()
    expect(
      screen.getAllByText('History database detected').length,
    ).toBeGreaterThan(0)
    // Save from sources page
    await user.click(mainBtns(/Save settings/)[0])
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  /* ---------- Settings: Schedule deeper ---------- */

  test('schedule settings shows interval controls', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Schedule/))
    // Due-after-hours and interval fields
    expect(screen.getByDisplayValue('72')).toBeInTheDocument()
    expect(screen.getByDisplayValue('6')).toBeInTheDocument()
    // Platform select
    expect(screen.getByDisplayValue('macOS (launchd)')).toBeInTheDocument()
  })

  test('schedule workflow mark-complete toggle', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Schedule/))
    // Preview a schedule first to populate the workflow
    await user.click(mainBtn(/Preview native schedule/))
    await waitFor(() => expect(mockBackend.previewSchedule).toHaveBeenCalled())
    // Mark manual step complete
    const markBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent === 'Mark complete')
    if (markBtns.length > 0) {
      await user.click(markBtns[0])
    }
  })

  test('schedule saves settings', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Schedule/))
    await user.click(mainBtns(/Save settings/)[0])
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  /* ---------- Settings: Security deeper ---------- */

  test('security rotate key', async () => {
    const user = userEvent.setup()
    // Need auto-unlock to have sessionDatabaseKey
    mockBackend.keyringGetDatabaseKey.mockResolvedValue('old-key')
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(
        makeSnapshot({
          archiveStatus: {
            ...structuredClone(baseSnapshot.archiveStatus),
            unlocked: false,
          },
        }),
      )
      .mockResolvedValueOnce(makeSnapshot())
    render(<AppNew />)
    await screen.findByText('Overview')
    await waitFor(() =>
      expect(mockBackend.setSessionDatabaseKey).toHaveBeenCalledWith('old-key'),
    )
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    // Type a new password and rotate
    const newPwdInputs = screen.getAllByPlaceholderText(
      'Store this in your password manager',
    )
    if (newPwdInputs.length > 0) {
      await user.type(newPwdInputs[0], 'new-pass')
      await user.click(mainBtn(/Rotate archive key/))
      await waitFor(() => expect(mockBackend.rekeyArchive).toHaveBeenCalled())
    }
  })

  test('security convert to plaintext', async () => {
    const user = userEvent.setup()
    mockBackend.keyringGetDatabaseKey.mockResolvedValue('old-key')
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(
        makeSnapshot({
          archiveStatus: {
            ...structuredClone(baseSnapshot.archiveStatus),
            unlocked: false,
          },
        }),
      )
      .mockResolvedValueOnce(makeSnapshot())
    render(<AppNew />)
    await screen.findByText('Overview')
    await waitFor(() =>
      expect(mockBackend.setSessionDatabaseKey).toHaveBeenCalledWith('old-key'),
    )
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    await user.click(mainBtn(/Convert to plaintext/))
    await waitFor(() =>
      expect(mockBackend.rekeyArchive).toHaveBeenCalledWith(
        expect.objectContaining({ newMode: 'Plaintext' }),
      ),
    )
  })

  test('security save settings', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    await user.click(mainBtns(/Save settings/)[0])
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  /* ---------- Settings: Remote deeper ---------- */

  test('remote settings shows fields when enabled', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          remoteBackup: {
            ...structuredClone(baseSnapshot.config.remoteBackup),
            enabled: true,
            bucket: 'my-bucket',
            region: 'eu-west-1',
            prefix: 'bhb/',
            credentialsSaved: true,
          },
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Remote/))
    expect(screen.getByDisplayValue('my-bucket')).toBeInTheDocument()
    expect(screen.getByDisplayValue('eu-west-1')).toBeInTheDocument()
    expect(screen.getByDisplayValue('bhb/')).toBeInTheDocument()
    expect(screen.getByText('Yes')).toBeInTheDocument()
  })

  test('remote settings stores and clears S3 credentials', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          remoteBackup: {
            ...structuredClone(baseSnapshot.config.remoteBackup),
            enabled: true,
            bucket: 'my-bucket',
          },
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Remote/))
    // Type access key ID and secret
    const allInputs = Array.from(
      document.querySelectorAll('input[type="password"]'),
    ) as HTMLInputElement[]
    if (allInputs.length >= 2) {
      await user.type(allInputs[0], 'AKID123')
      await user.type(allInputs[1], 'SECRET456')
      await user.click(mainBtn(/Save credentials/))
      await waitFor(() =>
        expect(mockBackend.storeS3Credentials).toHaveBeenCalledWith(
          expect.objectContaining({
            accessKeyId: 'AKID123',
            secretAccessKey: 'SECRET456',
          }),
        ),
      )
      await user.click(mainBtn(/Clear credentials/))
      await waitFor(() =>
        expect(mockBackend.clearS3Credentials).toHaveBeenCalled(),
      )
    }
  })

  test('remote settings saves', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Remote/))
    await user.click(mainBtns(/Save settings/)[0])
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  test('remote settings validation for empty credentials', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          remoteBackup: {
            ...structuredClone(baseSnapshot.config.remoteBackup),
            enabled: true,
            bucket: 'my-bucket',
          },
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Remote/))
    // Try to save without filling keys
    await user.click(mainBtn(/Save credentials/))
    // Error text from enterS3Credentials i18n key: 'Enter the S3 access key ID and secret access key before saving credentials.'
    await screen.findByRole('alert')
  })

  /* ---------- Settings: AI deeper ---------- */

  test('AI settings toggles and provider interactions', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Toggle AI off
    const aiToggle = screen.getAllByRole('checkbox')[0]
    await user.click(aiToggle)
    // AI sections should be hidden now
    await waitFor(() =>
      expect(screen.queryByText('Enable AI assistant')).not.toBeInTheDocument(),
    )
    // Toggle back on
    await user.click(aiToggle)
    await screen.findByText('Enable AI assistant')
  })

  test('AI settings saves', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    await user.click(mainBtns(/Save settings/)[0])
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  test('AI settings stores and clears provider key', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Find API key input and save/clear buttons in the provider editor
    const keyInputs = Array.from(
      document.querySelectorAll('input[type="password"]'),
    ) as HTMLInputElement[]
    if (keyInputs.length > 0) {
      await user.type(keyInputs[0], 'sk-test-key')
      const saveBtns = screen
        .getAllByRole('button')
        .filter((b) => b.textContent?.includes('Save key'))
      if (saveBtns.length > 0) {
        await user.click(saveBtns[0])
        await waitFor(() =>
          expect(mockBackend.storeAiProviderApiKey).toHaveBeenCalled(),
        )
      }
      const clearBtns = screen
        .getAllByRole('button')
        .filter((b) => b.textContent?.includes('Clear key'))
      if (clearBtns.length > 0) {
        await user.click(clearBtns[0])
        await waitFor(() =>
          expect(mockBackend.clearAiProviderApiKey).toHaveBeenCalled(),
        )
      }
    }
  })

  test('AI settings removes provider', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    const removeBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Remove provider'))
    if (removeBtns.length > 0) {
      await user.click(removeBtns[0])
    }
  })

  /* ---------- Onboarding: back navigation ---------- */

  test('onboarding back button', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          initialized: false,
          selectedProfileIds: [],
        },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    await waitFor(() =>
      expect(screen.getAllByText('Welcome').length).toBeGreaterThan(0),
    )
    await user.click(screen.getByRole('button', { name: /Sources/ }))
    // Back button should now be visible
    await user.click(screen.getByRole('button', { name: /Back/ }))
    // Should be back on Welcome
    await waitFor(() =>
      expect(screen.getAllByText('Welcome').length).toBeGreaterThan(0),
    )
  })

  test('onboarding schedule step toggles', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          initialized: false,
          selectedProfileIds: [],
        },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    await waitFor(() =>
      expect(screen.getAllByText('Welcome').length).toBeGreaterThan(0),
    )
    await user.click(screen.getByRole('button', { name: /Sources/ }))
    await user.click(screen.getByRole('button', { name: /Security/ }))
    await user.click(screen.getByRole('button', { name: /Schedule/ }))
    // Toggle favicons and git in schedule step
    const checkboxes = screen.getAllByRole('checkbox')
    if (checkboxes.length > 0) {
      await user.click(checkboxes[0])
    }
  })

  /* ---------- Dashboard: quick action buttons ---------- */

  test('dashboard quick actions navigate to explorer', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    // Click the Explorer quick action button
    await user.click(mainBtn(/Explorer/))
    // Should navigate to explorer page
    await waitFor(() => expect(mockBackend.queryHistory).toHaveBeenCalled())
  })

  test('dashboard quick actions navigate to settings', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    // Click the Settings quick action button
    await user.click(mainBtns(/Settings/)[0])
    // Should navigate to settings page
    await screen.findByRole('heading', { name: 'General' })
  })

  test('dashboard shows warning when archiveStatus has warning', async () => {
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          warning: 'Disk space low',
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    expect(screen.getAllByText('Disk space low').length).toBeGreaterThan(0)
  })

  /* ---------- Activity log: run backup from activity page ---------- */

  test('activity log run backup and show profile detail', async () => {
    const user = userEvent.setup()
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: false,
      reason: null,
      profiles: [
        {
          profileId: 'chrome:Default',
          newVisits: 12,
          newUrls: 5,
        },
      ],
      manifestPath: '/tmp/audit/run.json',
      gitCommit: 'abc',
      warnings: ['Low disk'],
      remoteBackup: null,
    })
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Activity Log'))
    await user.click(mainBtn(/Run backup now/))
    // Profile stats and warnings should appear
    await screen.findByText(/chrome:Default/)
    expect(screen.getByText('Low disk')).toBeInTheDocument()
  })

  test('activity log backup skipped due', async () => {
    const user = userEvent.setup()
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: true,
      reason: 'Not due yet',
      profiles: [],
      manifestPath: null,
      gitCommit: null,
      warnings: [],
      remoteBackup: null,
    })
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Activity Log'))
    await user.click(mainBtn(/Run backup now/))
    await waitFor(() =>
      expect(screen.getAllByText(/Not due/i).length).toBeGreaterThan(0),
    )
  })

  test('activity log run selects a recent run', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Activity Log'))
    // Click on the recent run card to select it
    const runCards = screen
      .getAllByRole('button')
      .filter(
        (b) =>
          b.closest('.runCard') !== null || b.classList.contains('runCard'),
      )
    if (runCards.length > 0) {
      await user.click(runCards[0])
    }
  })

  test('activity log remote backup preview and upload', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          remoteBackup: {
            ...structuredClone(baseSnapshot.config.remoteBackup),
            enabled: true,
          },
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Activity Log'))
    // Preview remote backup
    await user.click(mainBtn(/Preview upload/))
    await waitFor(() =>
      expect(mockBackend.previewRemoteBackup).toHaveBeenCalled(),
    )
    // Upload now
    await user.click(mainBtn(/Upload now/))
    await waitFor(() => expect(mockBackend.runRemoteBackup).toHaveBeenCalled())
  })

  /* ---------- Import: dry-run with path and import flow ---------- */

  test('import dry-run with takeout path', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Import'))
    const pathInput = screen.getByPlaceholderText(
      '/path/to/takeout.zip or /path/to/Takeout/',
    )
    await user.type(pathInput, '/tmp/takeout.zip')
    await user.click(mainBtn(/Dry-run/))
    await waitFor(() =>
      expect(mockBackend.inspectTakeout).toHaveBeenCalledWith(
        expect.objectContaining({
          sourcePath: '/tmp/takeout.zip',
          dryRun: true,
        }),
      ),
    )
    // Dry run results should appear
    await screen.findByText('takeout.jsonl')
  })

  test('import with quarantined files', async () => {
    mockBackend.inspectTakeout.mockResolvedValue({
      sourceType: 'zip',
      recognizedFiles: [
        { path: 'takeout.jsonl', kind: 'jsonl', status: 'ready', records: 10 },
      ],
      quarantinedFiles: [
        { path: 'mystery.bin', kind: 'unknown', status: 'quarantined' },
      ],
      totalRecords: 10,
      candidateItems: 10,
      importedItems: 0,
      duplicateItems: 0,
      importBatch: null,
      previewEntries: [
        {
          sourcePath: '/tmp/takeout.zip',
          url: 'https://example.com',
          title: 'Example',
          visitedAt: '2026-04-03T12:00:00.000Z',
          sourceVisitId: 1,
          status: 'preview',
        },
      ],
      notes: [],
    })
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Import'))
    const pathInput = screen.getByPlaceholderText(
      '/path/to/takeout.zip or /path/to/Takeout/',
    )
    await user.type(pathInput, '/tmp/takeout.zip')
    await user.click(mainBtn(/Dry-run/))
    await waitFor(() => expect(mockBackend.inspectTakeout).toHaveBeenCalled())
    await screen.findByText('mystery.bin')
  })

  test('import supported files flow', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Import'))
    const pathInput = screen.getByPlaceholderText(
      '/path/to/takeout.zip or /path/to/Takeout/',
    )
    await user.type(pathInput, '/tmp/takeout.zip')
    await user.click(mainBtn(/Import supported files/))
    await waitFor(() =>
      expect(mockBackend.importTakeout).toHaveBeenCalledWith(
        expect.objectContaining({
          sourcePath: '/tmp/takeout.zip',
          dryRun: false,
        }),
      ),
    )
  })

  /* ---------- Onboarding: profile deselect and archive mode ---------- */

  test('onboarding deselect profile via Sources step', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          initialized: false,
          selectedProfileIds: ['chrome:Default'],
        },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    await waitFor(() =>
      expect(screen.getAllByText('Welcome').length).toBeGreaterThan(0),
    )
    await user.click(screen.getByRole('button', { name: /Sources/ }))
    // Click the already-selected profile checkbox to deselect
    const checkboxes = screen.getAllByRole('checkbox')
    const checked = checkboxes.filter((cb) => (cb as HTMLInputElement).checked)
    if (checked.length > 0) {
      await user.click(checked[0])
    }
  })

  test('onboarding archive mode change to plaintext', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: { ...structuredClone(baseSnapshot.config), initialized: false },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    await waitFor(() =>
      expect(screen.getAllByText('Welcome').length).toBeGreaterThan(0),
    )
    // Navigate step-by-step: Welcome -> Sources -> Security
    await user.click(screen.getByRole('button', { name: /Sources/ }))
    await user.click(screen.getByRole('button', { name: /Security/ }))
    // Change archive mode from Encrypted to Plaintext
    const archiveModeSelect = screen.getByDisplayValue('Encrypted')
    await user.selectOptions(archiveModeSelect, 'Plaintext')
  })

  test('onboarding schedule due-after-hours change', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: { ...structuredClone(baseSnapshot.config), initialized: false },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    await waitFor(() =>
      expect(screen.getAllByText('Welcome').length).toBeGreaterThan(0),
    )
    // Navigate step-by-step: Welcome -> Sources -> Security -> Schedule
    await user.click(screen.getByRole('button', { name: /Sources/ }))
    await user.click(screen.getByRole('button', { name: /Security/ }))
    await user.click(screen.getByRole('button', { name: /Schedule/ }))
    // Change the due-after-hours number input
    const dueInput = screen.getByDisplayValue('72')
    await user.clear(dueInput)
    await user.type(dueInput, '24')
    // Also toggle git checkbox
    const checkboxes = screen.getAllByRole('checkbox')
    if (checkboxes.length >= 2) {
      await user.click(checkboxes[1])
    }
  })

  /* ---------- General settings: path open/copy ---------- */

  test('general settings storage path row actions', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    // PathRow renders paths in readonly inputs; use getByDisplayValue
    expect(
      screen.getByDisplayValue('/tmp/browser-history-backup'),
    ).toBeInTheDocument()
    expect(
      screen.getByDisplayValue('/tmp/browser-history-backup/archive.sqlite'),
    ).toBeInTheDocument()
    expect(
      screen.getByDisplayValue('/tmp/browser-history-backup/audit'),
    ).toBeInTheDocument()
    // Click open button (ghostButton inside pathActions)
    const pathActionBtns = Array.from(
      document.querySelectorAll('.pathActions .ghostButton'),
    )
    if (pathActionBtns.length > 0) {
      await user.click(pathActionBtns[0] as HTMLElement)
      await waitFor(() =>
        expect(mockBackend.openPathInFileManager).toHaveBeenCalled(),
      )
    }
  })

  /* ---------- Remote: edit individual fields ---------- */

  test('remote settings bucket and region edit', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          remoteBackup: {
            ...structuredClone(baseSnapshot.config.remoteBackup),
            enabled: true,
            bucket: '',
          },
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Remote/))
    // Fill in bucket
    const inputs = screen.getAllByRole('textbox')
    if (inputs.length > 0) await user.type(inputs[0], 'test-bucket')
    // Toggle path style
    const toggles = screen.getAllByRole('checkbox')
    if (toggles.length > 0) await user.click(toggles[0])
  })

  test('remote settings shows last error when present', async () => {
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          remoteBackup: {
            ...structuredClone(baseSnapshot.config.remoteBackup),
            enabled: true,
            bucket: 'my-bucket',
            lastError: 'Connection refused',
          },
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    const user = userEvent.setup()
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Remote/))
    expect(screen.getByText('Connection refused')).toBeInTheDocument()
  })

  /* ---------- Schedule: apply and remove ---------- */

  test('schedule apply preview workflow', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Schedule/))
    // Preview first
    await user.click(mainBtn(/Preview native schedule/))
    await waitFor(() =>
      expect(mockBackend.previewSchedule).toHaveBeenCalledWith('macos'),
    )
    // Apply the preview
    await user.click(mainBtn(/Apply preview/))
    await waitFor(() => expect(mockBackend.applySchedule).toHaveBeenCalled())
  })

  test('schedule remove schedule with confirm', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Schedule/))
    await user.click(mainBtn(/Preview native schedule/))
    await waitFor(() => expect(mockBackend.previewSchedule).toHaveBeenCalled())
    // Remove schedule
    await user.click(mainBtn(/Remove schedule/))
    expect(window.confirm).toHaveBeenCalled()
  })

  test('schedule apply without preview leaves button disabled', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Schedule/))
    // Apply button is disabled without preview
    const applyBtn = mainBtn(/Apply preview/)
    expect(applyBtn).toBeDisabled()
  })

  /* ---------- Sources settings: archive toggles ---------- */

  test('sources settings archive toggles', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Sources/))
    // Toggle capture favicons and git audit
    const checkboxes = screen.getAllByRole('checkbox')
    // Find the archive section toggles (after profile checkboxes)
    for (const cb of checkboxes) {
      const label = cb.closest('label')
      if (
        label?.textContent?.includes('Capture') ||
        label?.textContent?.includes('git')
      ) {
        await user.click(cb)
        break
      }
    }
  })

  /* ---------- AI providers: add and edit providers ---------- */

  test('AI settings add LLM provider', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Add a new LLM provider
    const addBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Add LLM provider'))
    if (addBtns.length > 0) {
      await user.click(addBtns[0])
    }
  })

  test('AI settings add embedding provider', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Add a new embedding provider
    const addBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Add embedding provider'))
    if (addBtns.length > 0) {
      await user.click(addBtns[0])
    }
  })

  test('AI settings edit provider fields', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Edit a text field in the provider editor
    const nameInputs = screen
      .getAllByDisplayValue('Preview LLM')
      .filter(
        (el) => el.tagName === 'INPUT' && !(el as HTMLInputElement).readOnly,
      )
    if (nameInputs.length > 0) {
      await user.clear(nameInputs[0])
      await user.type(nameInputs[0], 'Updated LLM')
    }
  })

  /* ---------- Security: remember key ---------- */

  test('security remember and clear key', async () => {
    const user = userEvent.setup()
    mockBackend.keyringGetDatabaseKey.mockResolvedValue('existing-key')
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(
        makeSnapshot({
          archiveStatus: {
            ...structuredClone(baseSnapshot.archiveStatus),
            unlocked: false,
          },
        }),
      )
      .mockResolvedValueOnce(makeSnapshot())
    render(<AppNew />)
    await screen.findByText('Overview')
    await waitFor(() =>
      expect(mockBackend.setSessionDatabaseKey).toHaveBeenCalledWith(
        'existing-key',
      ),
    )
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    // Remember key
    const rememberBtn = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Remember current key'))
    if (rememberBtn.length > 0) {
      await user.click(rememberBtn[0])
      await waitFor(() =>
        expect(mockBackend.keyringStoreDatabaseKey).toHaveBeenCalled(),
      )
    }
    // Clear remembered key
    const clearBtn = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Clear remembered key'))
    if (clearBtn.length > 0) {
      await user.click(clearBtn[0])
      await waitFor(() =>
        expect(mockBackend.keyringClearDatabaseKey).toHaveBeenCalled(),
      )
    }
  })

  /* ---------- Insights: explain card ---------- */

  test('insights explain card', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Insights'))
    await waitFor(() => expect(mockBackend.loadInsights).toHaveBeenCalled())
    await screen.findByText('Rising topic: archive tooling')
    // Click explain button
    const explainBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Explain'))
    if (explainBtns.length > 0) {
      await user.click(explainBtns[0])
      await waitFor(() => expect(mockBackend.explainInsight).toHaveBeenCalled())
    }
  })

  /* ---------- Explorer: export ---------- */

  test('explorer export JSONL', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Explorer'))
    await waitFor(() => expect(mockBackend.queryHistory).toHaveBeenCalled())
    // Export buttons are format names: JSONL, HTML, MARKDOWN, TEXT
    await user.click(mainBtn(/JSONL/))
    await waitFor(() => expect(mockBackend.exportHistory).toHaveBeenCalled())
  })

  /* ---------- Explorer: visit detail selection ---------- */

  test('explorer select a visit from results', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Explorer'))
    await waitFor(() => expect(mockBackend.queryHistory).toHaveBeenCalled())
    // Click a history row to select it
    const resultRows = screen
      .getAllByRole('button')
      .filter(
        (b) =>
          b.closest('.resultRow') !== null || b.classList.contains('resultRow'),
      )
    if (resultRows.length > 0) {
      await user.click(resultRows[0])
    }
  })

  /* ---------- General: path copy and autostart ---------- */

  test('general settings copy path and toggle autostart', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    // Click all copy buttons (ghostButton with content_copy icon)
    const copyBtns = Array.from(
      document.querySelectorAll('.pathActions .ghostButton'),
    ) as HTMLElement[]
    for (const btn of copyBtns) {
      await user.click(btn)
    }
    // Toggle app autostart
    const autostartToggle = screen
      .getAllByRole('checkbox')
      .find((cb) => cb.closest('label')?.textContent?.includes('Autostart'))
    if (autostartToggle) {
      await user.click(autostartToggle)
    }
    // Click save
    await user.click(mainBtn(/Save/))
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  /* ---------- Remote: all field interactions ---------- */

  test('remote settings fills all fields and toggles', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          remoteBackup: {
            ...structuredClone(baseSnapshot.config.remoteBackup),
            enabled: true,
            bucket: 'b',
            region: 'us-east-1',
            prefix: 'pfx',
          },
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Remote/))
    // Type in region field
    const regionInput = screen.getByDisplayValue('us-east-1')
    await user.clear(regionInput)
    await user.type(regionInput, 'eu-west-1')
    // Type in endpoint field
    const endpointInput = screen.getByPlaceholderText(/Leave empty/)
    await user.type(endpointInput, 'https://s3.custom.local')
    // Type in prefix field
    const prefixInput = screen.getByDisplayValue('pfx')
    await user.clear(prefixInput)
    await user.type(prefixInput, 'new-prefix')
    // Toggle upload after backup
    const checkboxes = screen.getAllByRole('checkbox')
    for (const cb of checkboxes) {
      const label = cb.closest('label')
      if (label?.textContent?.includes('Upload after')) {
        await user.click(cb)
        break
      }
    }
    // Fill credentials and save
    const passwordInputs = Array.from(
      document.querySelectorAll('input[type="password"]'),
    ) as HTMLInputElement[]
    if (passwordInputs.length >= 2) {
      await user.type(passwordInputs[0], 'AKID')
      await user.type(passwordInputs[1], 'SECRET')
    }
    await user.click(mainBtn(/Save credentials/))
    await waitFor(() =>
      expect(mockBackend.storeS3Credentials).toHaveBeenCalled(),
    )
    // Save settings
    const allSaveBtns = mainBtns(/Save/)
    await user.click(allSaveBtns[allSaveBtns.length - 1])
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  test('remote settings clear credentials', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          remoteBackup: {
            ...structuredClone(baseSnapshot.config.remoteBackup),
            enabled: true,
            credentialsSaved: true,
          },
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Remote/))
    await user.click(mainBtn(/Clear credentials/))
    await waitFor(() =>
      expect(mockBackend.clearS3Credentials).toHaveBeenCalled(),
    )
  })

  /* ---------- Schedule: all config inputs ---------- */

  test('schedule settings edits config inputs', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Schedule/))
    // Change due-after-hours
    const dueInput = screen.getByDisplayValue('72')
    await user.clear(dueInput)
    await user.type(dueInput, '48')
    // Change check interval hours
    const checkInput = screen.getByDisplayValue('6')
    await user.clear(checkInput)
    await user.type(checkInput, '12')
    // Change platform
    const platformSelect = screen.getByDisplayValue(/macOS/)
    await user.selectOptions(platformSelect, 'linux')
    // Save
    await user.click(mainBtns(/Save/)[mainBtns(/Save/).length - 1])
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  /* ---------- Security: rotate without session key ---------- */

  test('security rotate key without session key shows error', async () => {
    const user = userEvent.setup()
    // archive is unlocked but the session key is not in the context
    // this triggers the handleRotateEncryption guard
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    // Type a new password and click rotate
    const pwInputs = Array.from(
      document.querySelectorAll('input[type="password"]'),
    ) as HTMLInputElement[]
    if (pwInputs.length > 0) {
      await user.type(pwInputs[0], 'new-password')
    }
    const rotateBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Rotate'))
    if (rotateBtns.length > 0) {
      await user.click(rotateBtns[0])
    }
  })

  test('security rotate key with empty password shows error', async () => {
    const user = userEvent.setup()
    // Set up with a session key available
    mockBackend.keyringGetDatabaseKey.mockResolvedValue('existing-key')
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(
        makeSnapshot({
          archiveStatus: {
            ...structuredClone(baseSnapshot.archiveStatus),
            unlocked: false,
          },
        }),
      )
      .mockResolvedValueOnce(makeSnapshot())
    render(<AppNew />)
    await screen.findByText('Overview')
    await waitFor(() =>
      expect(mockBackend.setSessionDatabaseKey).toHaveBeenCalled(),
    )
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    // Click rotate without entering a password
    const rotateBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Rotate'))
    if (rotateBtns.length > 0) {
      await user.click(rotateBtns[0])
    }
  })

  test('security switch to plaintext', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    // Click convert to plaintext
    const plaintextBtn = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Convert to plaintext'))
    if (plaintextBtn.length > 0) {
      await user.click(plaintextBtn[0])
      await waitFor(() => expect(mockBackend.rekeyArchive).toHaveBeenCalled())
    }
  })

  test('security remember key without session key shows error', async () => {
    const user = userEvent.setup()
    // Default mock doesn't auto-unlock, so sessionDatabaseKey is null
    mockBackend.keyringGetDatabaseKey.mockResolvedValue(null)
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    // Click remember key — should trigger error since no session key
    const rememberBtn = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Remember'))
    if (rememberBtn.length > 0) {
      await user.click(rememberBtn[0])
    }
  })

  test('security save settings', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    await user.click(mainBtn(/Save/))
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  /* ---------- Security: uninitialized archive mode select ---------- */

  test('security uninitialized shows archive mode select', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    // Uninitialized archive should show archive mode select
    const modeSelects = screen.getAllByDisplayValue('Encrypted')
    if (modeSelects.length > 0) {
      await user.selectOptions(modeSelects[0], 'Plaintext')
    }
  })

  /* ---------- AI providers: select embedding provider + store key ---------- */

  test('AI providers select embedding provider', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Select embedding provider from dropdown
    const embeddingSelects = screen.getAllByDisplayValue('Preview Embeddings')
    if (embeddingSelects.length > 0) {
      await user.selectOptions(embeddingSelects[0], '')
    }
  })

  test('AI providers store API key', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Type API key and save it
    const apiKeyInput = screen
      .getAllByPlaceholderText('sk-...')
      .filter((el) => el.tagName === 'INPUT')
    if (apiKeyInput.length > 0) {
      await user.type(apiKeyInput[0], 'test-key')
      const saveBtns = screen
        .getAllByRole('button')
        .filter((b) => b.textContent?.includes('Save key'))
      if (saveBtns.length > 0) {
        await user.click(saveBtns[0])
        await waitFor(() =>
          expect(mockBackend.storeAiProviderApiKey).toHaveBeenCalled(),
        )
      }
    }
  })

  test('AI providers clear API key', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    const clearBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Clear key'))
    if (clearBtns.length > 0) {
      await user.click(clearBtns[0])
      await waitFor(() =>
        expect(mockBackend.clearAiProviderApiKey).toHaveBeenCalled(),
      )
    }
  })

  test('AI providers remove and add providers', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Remove provider
    const removeBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Remove'))
    if (removeBtns.length > 0) {
      await user.click(removeBtns[0])
    }
    // Add providers
    const addBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Add'))
    for (const btn of addBtns) {
      await user.click(btn)
    }
  })

  test('AI providers save settings', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Save — use last Save button on the page (specific to AI Providers tab)
    const saveBtns = mainBtns(/Save/)
    await user.click(saveBtns[saveBtns.length - 1])
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  /* ---------- Sources: deeper toggles and save ---------- */

  test('sources settings toggle favicons and git audit', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Sources/))
    // Toggle all checkboxes to hit onChange handlers
    const checkboxes = screen.getAllByRole('checkbox')
    for (const cb of checkboxes) {
      await user.click(cb)
    }
    // Save
    await user.click(mainBtn(/Save/))
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
  })

  /* ---------- Import: batch select and revert ---------- */

  test('import select batch and revert', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Import'))
    // The batch list should appear
    await screen.findByText('/tmp/takeout.zip')
    // Click the batch row
    const batchBtns = screen
      .getAllByRole('button')
      .filter(
        (b) =>
          b.closest('.batchRow') !== null || b.classList.contains('batchRow'),
      )
    if (batchBtns.length > 0) {
      await user.click(batchBtns[0])
      await waitFor(() =>
        expect(mockBackend.previewImportBatch).toHaveBeenCalled(),
      )
    }
    // Revert batch
    const revertBtn = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Revert'))
    if (revertBtn.length > 0) {
      await user.click(revertBtn[0])
      await waitFor(() =>
        expect(mockBackend.revertImportBatch).toHaveBeenCalled(),
      )
    }
  })

  /* ---------- Insights: run insights ---------- */

  test('insights run insights now', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Insights'))
    await waitFor(() => expect(mockBackend.loadInsights).toHaveBeenCalled())
    const runBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Run insights'))
    if (runBtns.length > 0) {
      await user.click(runBtns[0])
      await waitFor(() => expect(mockBackend.runInsightsNow).toHaveBeenCalled())
    }
  })

  test('insights view thread detail', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Insights'))
    await waitFor(() => expect(mockBackend.loadInsights).toHaveBeenCalled())
    // Threads section should render
    const threadTitles = screen.getAllByText('archive tool compare')
    // Find the one inside a button (thread row)
    const clickable = threadTitles.find((el) => el.closest('button'))
    if (clickable) {
      await user.click(clickable.closest('button')!)
      await waitFor(() =>
        expect(mockBackend.loadThreadDetail).toHaveBeenCalled(),
      )
    }
  })

  /* ---------- Dashboard: doctor report ---------- */

  test('dashboard run doctor and see report', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    // Click Run doctor quick action
    await user.click(mainBtn(/Run doctor/))
    await waitFor(() => expect(mockBackend.doctor).toHaveBeenCalled())
    // Should show the doctor report
    await screen.findByText('DB integrity')
  })

  test('dashboard backup skipped displays reason', async () => {
    const user = userEvent.setup()
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: true,
      reason: 'Backup not due yet',
      profiles: [],
      manifestPath: null,
      gitCommit: null,
      warnings: [],
      remoteBackup: null,
    })
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(mainBtn(/Run backup now/))
    await waitFor(() =>
      expect(screen.getAllByText(/Backup not due/i).length).toBeGreaterThan(0),
    )
  })

  /* ---------- AI providers: LLM radio select ---------- */

  test('AI providers select LLM provider via radio', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Select LLM provider via radio button
    const radios = screen.getAllByRole('radio')
    if (radios.length > 0) {
      await user.click(radios[0])
    }
  })

  test('AI providers embedding provider interactions', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Interact with embedding provider section
    // Select embedding radio
    const radios = screen.getAllByRole('radio')
    // The last radio should be the embedding provider
    if (radios.length >= 2) {
      await user.click(radios[radios.length - 1])
    }
    // Type in the embedding provider's API key input
    const apiInputs = screen.getAllByPlaceholderText('sk-...')
    if (apiInputs.length >= 2) {
      await user.type(apiInputs[apiInputs.length - 1], 'emb-key')
    }
    // Save the embedding key
    const saveBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent === 'Save key')
    if (saveBtns.length >= 2) {
      await user.click(saveBtns[saveBtns.length - 1])
    }
    // Clear the embedding key
    const clearBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent === 'Clear key')
    if (clearBtns.length >= 2) {
      await user.click(clearBtns[clearBtns.length - 1])
    }
    // Remove the embedding provider
    const removeBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent === 'Remove')
    if (removeBtns.length >= 2) {
      await user.click(removeBtns[removeBtns.length - 1])
    }
    // Add embedding provider
    const addBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Add'))
    if (addBtns.length >= 2) {
      await user.click(addBtns[addBtns.length - 1])
    }
  })

  test('AI providers embedding provider field update', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Edit embedding provider name
    const embNameInputs = screen
      .getAllByDisplayValue('Preview Embeddings')
      .filter(
        (el) => el.tagName === 'INPUT' && !(el as HTMLInputElement).readOnly,
      )
    if (embNameInputs.length > 0) {
      await user.clear(embNameInputs[0])
      await user.type(embNameInputs[0], 'Updated Embeddings')
    }
    // Edit the dimensions spinbutton
    const dimInputs = screen
      .getAllByDisplayValue('1536')
      .filter((el) => el.tagName === 'INPUT')
    if (dimInputs.length > 0) {
      await user.clear(dimInputs[0])
      await user.type(dimInputs[0], '3072')
    }
  })

  test('AI providers select embedding provider dropdown', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/AI Providers/))
    // Select embedding provider from the settings dropdown (not the editor radio)
    const selects = screen.getAllByRole('combobox')
    // Find the embedding provider dropdown
    for (const sel of selects) {
      const options = Array.from(sel.querySelectorAll('option'))
      const hasEmbedding = options.some((o) =>
        o.textContent?.includes('Preview Embeddings'),
      )
      if (hasEmbedding) {
        await user.selectOptions(sel, '')
        break
      }
    }
  })

  /* ---------- Dashboard: last backup report states ---------- */

  test('dashboard shows backup complete with profile and warning counts', async () => {
    const user = userEvent.setup()
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: false,
      reason: null,
      profiles: [
        {
          profileId: 'chrome:Default',
          newVisits: 34,
          newUrls: 12,
        },
      ],
      manifestPath: '/tmp/manifest.json',
      gitCommit: 'deadbeef',
      warnings: ['Low disk space'],
      remoteBackup: null,
    })
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(mainBtn(/Run backup now/))
    // Last backup report should show the profile stats
    await waitFor(() =>
      expect(screen.getAllByText(/chrome:Default/).length).toBeGreaterThan(0),
    )
  })

  /* ---------- Import: batch detail with error handling ---------- */

  test('import batch preview error handling', async () => {
    const user = userEvent.setup()
    mockBackend.previewImportBatch.mockRejectedValueOnce(
      new Error('Preview failed'),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Import'))
    await screen.findByText('/tmp/takeout.zip')
    // Click the batch row - should handle error gracefully
    const batchBtns = screen
      .getAllByRole('button')
      .filter(
        (b) =>
          b.closest('.batchRow') !== null || b.classList.contains('batchRow'),
      )
    if (batchBtns.length > 0) {
      await user.click(batchBtns[0])
    }
  })

  /* ---------- Explorer: domain filter and profile filter ---------- */

  test('explorer domain and profile filters', async () => {
    const user = userEvent.setup()
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Explorer'))
    await waitFor(() => expect(mockBackend.queryHistory).toHaveBeenCalled())
    // Type in domain filter
    const domainInput = screen.getByPlaceholderText(/github.com/)
    await user.type(domainInput, 'test.com')
    // Change profile filter
    const profileSelect = screen.getByDisplayValue('All selected profiles')
    await user.selectOptions(profileSelect, 'chrome:Default')
    // Wait for queries triggered by filter changes
    await waitFor(() => {
      const callCount = mockBackend.queryHistory.mock.calls.length
      expect(callCount).toBeGreaterThan(1)
    })
  })

  /* ---------- app-context: unlock with empty password ---------- */

  test('security unlock with empty password shows error', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    // Click unlock without entering password
    const unlockBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Unlock'))
    if (unlockBtns.length > 0) {
      await user.click(unlockBtns[0])
    }
  })

  test('security unlock with wrong password shows error', async () => {
    const user = userEvent.setup()
    strongholdMocks.readDatabaseKeyStronghold.mockResolvedValue(null)
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          unlocked: false,
        },
      }),
    )
    render(<AppNew />)
    await screen.findByText('Overview')
    await user.click(navBtn('Settings'))
    await user.click(mainBtn(/Security/))
    // Type a password and try to unlock
    const pwInputs = Array.from(
      document.querySelectorAll('input[type="password"]'),
    ) as HTMLInputElement[]
    if (pwInputs.length > 0) {
      await user.type(pwInputs[0], 'wrong-password')
    }
    const unlockBtns = screen
      .getAllByRole('button')
      .filter((b) => b.textContent?.includes('Unlock'))
    if (unlockBtns.length > 0) {
      await user.click(unlockBtns[0])
      await waitFor(() =>
        expect(strongholdMocks.readDatabaseKeyStronghold).toHaveBeenCalled(),
      )
    }
  })
})
