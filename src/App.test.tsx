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
    previewAiIntegrations: vi.fn(),
    resetLocalSecretVault: vi.fn(),
    openPathInFileManager: vi.fn(),
  },
  strongholdMocks: {
    readDatabaseKeyStronghold: vi.fn().mockResolvedValue(null),
    storeDatabaseKeyStronghold: vi.fn().mockResolvedValue(undefined),
  },
}))

vi.mock('./lib/backend', () => ({
  backend: mockBackend,
}))

vi.mock('./lib/stronghold', () => ({
  readDatabaseKeyStronghold: strongholdMocks.readDatabaseKeyStronghold,
  storeDatabaseKeyStronghold: strongholdMocks.storeDatabaseKeyStronghold,
}))

vi.mock('@tauri-apps/plugin-autostart', () => autostartMocks)

import App from './App'

const browserProfiles = [
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
  {
    profileId: 'safari:default',
    profileName: 'Safari',
    browserFamily: 'safari',
    browserName: 'Safari',
    userName: null,
    profilePath: '/Users/demo/Library/Safari',
    historyPath: '/Users/demo/Library/Safari/History.db',
    faviconsPath: null,
    historyExists: true,
    browserVersion: '18.0',
    historyFileName: 'History.db',
  },
]

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
      assistantSystemPrompt:
        'You are an audit-first history research assistant.',
      llmProviders: [llmProvider],
      embeddingProviders: [embeddingProvider],
    },
  },
  archiveStatus: {
    initialized: true,
    encrypted: true,
    unlocked: true,
    databasePath: '/tmp/browser-history-backup/archive.sqlite',
    lastSuccessfulBackupAt: null,
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
    warning: 'Preview mode uses lexical fallbacks.',
  },
  browserProfiles,
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
  productName: 'Browser History Backup',
  version: '0.1.0',
  gitCommitShort: 'abc12345',
  gitCommitFull: 'abc12345def67890',
  gitDirty: false,
}

function makeSnapshot(overrides: Record<string, unknown> = {}) {
  return {
    ...structuredClone(baseSnapshot),
    ...overrides,
  }
}

function getClosestSection(text: RegExp | string) {
  const section = screen.getByText(text).closest('section')
  expect(section).not.toBeNull()
  return section as HTMLElement
}

function getProviderPanel(title: string) {
  const panel = screen.getByText(title).closest('.providerPanel')
  expect(panel).not.toBeNull()
  return panel as HTMLElement
}

describe('App integration', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'confirm', {
      writable: true,
      value: vi.fn().mockReturnValue(true),
    })
    Object.defineProperty(window.navigator, 'clipboard', {
      writable: true,
      value: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    })

    Object.values(mockBackend).forEach((mock) => mock.mockReset())
    autostartMocks.disable.mockReset()
    autostartMocks.enable.mockReset()
    autostartMocks.isEnabled.mockReset().mockResolvedValue(false)
    strongholdMocks.readDatabaseKeyStronghold
      .mockReset()
      .mockResolvedValue(null)
    strongholdMocks.storeDatabaseKeyStronghold
      .mockReset()
      .mockResolvedValue(undefined)
    mockBackend.getAppSnapshot.mockReset().mockResolvedValue(makeSnapshot())
    mockBackend.getAppBuildInfo.mockResolvedValue(buildInfo)
    mockBackend.keyringGetDatabaseKey.mockResolvedValue(null)
    mockBackend.queryHistory.mockResolvedValue({
      total: 2,
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
        {
          id: 2,
          profileId: 'safari:default',
          url: 'https://fallback.example/untitled',
          title: null,
          domain: 'fallback.example',
          visitedAt: '2026-04-03T13:00:00.000Z',
          visitTime: 2,
          durationMs: null,
          transition: null,
          sourceVisitId: 2,
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
      label: 'dev.example.browser-history-backup.backup',
      executablePath: '/Applications/Browser History Backup.app',
      generatedFiles: [
        {
          relativePath:
            'launchd/dev.example.browser-history-backup.backup.plist',
          absolutePath:
            '/tmp/browser-history-backup/schedule/dev.example.browser-history-backup.backup.plist',
          purpose: 'LaunchAgent',
          contents: '<plist />',
        },
      ],
      manualSteps: [
        'Copy the plist into ~/Library/LaunchAgents.',
        'Load it with launchctl bootstrap gui/$UID.',
      ],
      applyCommands: [['launchctl', 'bootstrap', 'gui/$UID', 'file.plist']],
      rollbackCommands: [['launchctl', 'bootout', 'gui/$UID', 'file.plist']],
      applySupported: true,
    })
    mockBackend.applySchedule.mockResolvedValue({
      applied: true,
      platform: 'macos',
      files: [
        '/tmp/browser-history-backup/schedule/dev.example.browser-history-backup.backup.plist',
      ],
      auditPath: '/tmp/browser-history-backup/audit/schedule.json',
      message: 'Schedule applied.',
    })
    mockBackend.exportHistory.mockResolvedValue({
      format: 'jsonl',
      path: '/tmp/history-export.jsonl',
      count: 1,
    })
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: false,
      reason: null,
      run: makeSnapshot().recentRuns[0],
      profiles: [],
      manifestPath: '/tmp/browser-history-backup/audit/manifests/run-3.json',
      gitCommit: 'abc123',
      warnings: [],
      remoteBackup: null,
    })
    mockBackend.doctor.mockResolvedValue({
      generatedAt: '2026-04-03T12:10:00.000Z',
      checks: [],
    })
    mockBackend.previewRemoteBackup.mockResolvedValue({
      bundlePath: '/tmp/browser-history-backup-remote.zip',
      objectKey: 'browser-history-backup/remote.zip',
      uploadUrl: 'https://s3.example.com/browser-history-backup/remote.zip',
      previewCommand: 'curl -T bundle.zip https://s3.example.com/upload',
      manualSteps: ['Review the bundle.', 'Upload it manually if preferred.'],
      warnings: [],
    })
    mockBackend.runRemoteBackup.mockResolvedValue({
      uploaded: true,
      bundlePath: '/tmp/browser-history-backup-remote.zip',
      objectKey: 'browser-history-backup/remote.zip',
      uploadUrl: 'https://s3.example.com/browser-history-backup/remote.zip',
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
    mockBackend.buildAiIndex.mockResolvedValue({
      providerId: 'embedding-preview',
      model: 'text-embedding-3-large',
      indexedItems: 8,
      updatedItems: 2,
      skippedItems: 0,
      removedItems: 0,
      lastIndexedAt: '2026-04-03T12:20:00.000Z',
      notes: ['Index built.'],
    })
    mockBackend.searchAiHistory.mockResolvedValue({
      total: 1,
      providerId: 'embedding-preview',
      model: 'text-embedding-3-large',
      items: [
        {
          historyId: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com',
          title: 'Example',
          domain: 'example.com',
          visitedAt: '2026-04-03T12:00:00.000Z',
          score: 0.88,
          matchReason: 'Semantic match',
        },
      ],
      notes: ['Preview semantic result.'],
    })
    mockBackend.askAiAssistant.mockResolvedValue({
      answer: 'You mostly read implementation details about history schemas.',
      providerId: 'llm-preview',
      embeddingProviderId: 'embedding-preview',
      citations: [
        {
          historyId: 1,
          profileId: 'chrome:Default',
          url: 'https://example.com',
          title: 'Example',
          visitedAt: '2026-04-03T12:00:00.000Z',
          score: 0.88,
        },
      ],
      notes: ['Grounded in preview data.'],
    })
    mockBackend.previewAiIntegrations.mockResolvedValue({
      mcpCommand:
        '/Applications/Browser History Backup.app --worker mcp-server',
      manualSteps: ['Enable MCP in Settings.', 'Copy the generated snippet.'],
      generatedFiles: [
        {
          relativePath: 'integrations/browser-history-backup-mcp.json',
          absolutePath:
            '/tmp/browser-history-backup/integrations/browser-history-backup-mcp.json',
          purpose: 'MCP snippet',
          contents: '{"mcpServers":{}}',
        },
      ],
      warnings: [],
    })
    mockBackend.resetLocalSecretVault.mockResolvedValue(undefined)
    mockBackend.openPathInFileManager.mockResolvedValue(
      baseSnapshot.directories.appRoot,
    )
  })

  test('loads import batches and lets the user revert a dirty import batch', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(makeSnapshot())
      .mockResolvedValueOnce(
        makeSnapshot({
          recentImportBatches: [revertedBatchDetail.batch],
        }),
      )

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Import' }))

    await screen.findByText('Takeout import and health checks')
    await waitFor(() =>
      expect(mockBackend.previewImportBatch).toHaveBeenCalledWith(7),
    )
    expect(screen.getByText('Recent import batches')).toBeInTheDocument()
    expect(screen.getByText('#7')).toBeInTheDocument()
    expect(screen.getByText('Looks good')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Revert batch' }))

    await waitFor(() =>
      expect(mockBackend.revertImportBatch).toHaveBeenCalledWith(7),
    )
    await screen.findByText(
      'The selected import batch was reverted. Live history rows were removed, and the raw audit trail was preserved.',
    )
  })

  test('shows build metadata and opens the app data root from settings', async () => {
    const user = userEvent.setup()

    render(<App />)

    await screen.findByText('History explorer')
    await screen.findByText('Version 0.1.0')
    await screen.findByText('Commit abc12345')

    await user.click(screen.getByRole('button', { name: 'Settings' }))
    await screen.findByText('Data and build info')

    expect(
      screen.getByDisplayValue('/tmp/browser-history-backup'),
    ).toBeInTheDocument()

    await user.click(
      screen.getByRole('button', { name: /open app data root/i }),
    )

    await waitFor(() =>
      expect(mockBackend.openPathInFileManager).toHaveBeenCalledWith(
        '/tmp/browser-history-backup',
      ),
    )
  })

  test('guides setup, previews the native schedule, and initializes the archive', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockReset().mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          initialized: false,
          rememberDatabaseKeyInKeyring: true,
          selectedProfileIds: [],
        },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          unlocked: false,
        },
      }),
    )
    mockBackend.initializeArchive.mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          initialized: true,
          rememberDatabaseKeyInKeyring: true,
        },
      }),
    )
    mockBackend.previewSchedule.mockResolvedValue({
      platform: 'macos',
      label: 'dev.example.browser-history-backup.backup',
      executablePath: '/Applications/Browser History Backup.app',
      generatedFiles: [
        {
          relativePath:
            'launchd/dev.example.browser-history-backup.backup.plist',
          absolutePath: null,
          purpose: 'LaunchAgent',
          contents: '<plist />',
        },
      ],
      manualSteps: [
        'Copy the plist into ~/Library/LaunchAgents.',
        'Load it with launchctl bootstrap gui/$UID.',
      ],
      applyCommands: [['launchctl', 'bootstrap', 'gui/$UID', 'file.plist']],
      rollbackCommands: [['launchctl', 'bootout', 'gui/$UID', 'file.plist']],
      applySupported: true,
    })

    render(<App />)

    await screen.findByText(
      'Choose which browser profiles should be included in backups.',
    )
    const checkboxes = screen.getAllByRole('checkbox')
    await user.click(checkboxes[1])
    await user.click(checkboxes[1])
    await user.click(screen.getByRole('button', { name: 'Plaintext' }))
    await user.click(screen.getByRole('button', { name: 'Encrypted' }))
    await user.type(
      screen.getAllByPlaceholderText('Store this in your password manager')[0],
      'vault-pass',
    )
    await user.type(
      screen.getAllByPlaceholderText('Store this in your password manager')[1],
      'vault-pass',
    )
    await user.click(
      screen.getByRole('button', { name: 'Preview native schedule' }),
    )
    await screen.findByText('Scheduler preview is ready.')
    expect(
      screen.getAllByText(
        'launchd/dev.example.browser-history-backup.backup.plist',
      ).length,
    ).toBeGreaterThan(0)
    await user.click(
      screen.getAllByRole('button', { name: 'Apply preview' })[0],
    )
    await waitFor(() =>
      expect(screen.getAllByText('Schedule applied.').length).toBeGreaterThan(
        0,
      ),
    )
    await user.click(screen.getByRole('button', { name: 'Create archive' }))

    await waitFor(() =>
      expect(mockBackend.initializeArchive).toHaveBeenCalledWith(
        expect.objectContaining({
          initialized: true,
          rememberDatabaseKeyInKeyring: true,
        }),
        expect.any(String),
      ),
    )
    expect(strongholdMocks.storeDatabaseKeyStronghold).toHaveBeenCalled()
  })

  test('shows settings fallbacks when metadata paths are unavailable', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockResolvedValue(
      makeSnapshot({
        directories: {
          ...structuredClone(baseSnapshot.directories),
          appRoot: '',
          archiveDatabasePath: '',
          auditRepoPath: '',
        },
        keyringStatus: {
          ...structuredClone(baseSnapshot.keyringStatus),
          backend: '',
        },
      }),
    )
    mockBackend.getAppBuildInfo.mockResolvedValue(null)

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Settings' }))
    expect(
      screen.getAllByDisplayValue('Not available').length,
    ).toBeGreaterThanOrEqual(3)
    await user.click(
      screen.getByRole('button', { name: /Open .*App data root/i }),
    )
    expect(mockBackend.openPathInFileManager).not.toHaveBeenCalled()

    const securitySection = getClosestSection(
      'Manage the encrypted archive state, unlock path, remembered key, and rekey operations.',
    )
    expect(
      within(securitySection).getByText('Not available'),
    ).toBeInTheDocument()
  })

  test('shows import batch fallbacks when imported metadata is missing', async () => {
    const user = userEvent.setup()
    mockBackend.previewImportBatch.mockResolvedValue({
      ...batchDetail,
      batch: {
        ...batchDetail.batch,
        importedAt: null,
        auditPath: null,
      },
      previewEntries: [],
      recognizedFiles: [],
      quarantinedFiles: [],
      notes: [],
    })

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Import' }))
    await waitFor(() =>
      expect(mockBackend.previewImportBatch).toHaveBeenCalledWith(7),
    )
    await waitFor(() =>
      expect(
        screen.getAllByText('Not available').length,
      ).toBeGreaterThanOrEqual(2),
    )
    await screen.findByText(
      'No preview rows are available for this selection yet.',
    )
    expect(screen.queryByText('Looks good')).not.toBeInTheDocument()
  })

  test('supports explorer exports, AI workbench actions, and backup review flows', async () => {
    const user = userEvent.setup()

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'HTML' }))
    await waitFor(() =>
      expect(mockBackend.exportHistory).toHaveBeenCalledWith(
        expect.objectContaining({ format: 'html' }),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'Analysis' }))
    await screen.findByText('Add LLM provider')
    await user.click(screen.getByRole('button', { name: 'Save AI settings' }))
    await waitFor(() => expect(mockBackend.saveConfig).toHaveBeenCalled())
    await user.click(screen.getByRole('button', { name: 'Build index' }))
    await waitFor(() => expect(mockBackend.buildAiIndex).toHaveBeenCalled())
    await user.type(screen.getByLabelText('Semantic search'), 'schema')
    await user.click(
      screen.getByRole('button', { name: 'Run semantic search' }),
    )
    await waitFor(() => expect(mockBackend.searchAiHistory).toHaveBeenCalled())
    await user.type(
      screen.getByLabelText('Question for the assistant'),
      'What did I research?',
    )
    await user.click(screen.getByRole('button', { name: 'Ask assistant' }))
    await waitFor(() => expect(mockBackend.askAiAssistant).toHaveBeenCalled())
    await user.click(
      screen.getByRole('button', { name: 'Preview MCP and skill files' }),
    )
    await waitFor(() =>
      expect(mockBackend.previewAiIntegrations).toHaveBeenCalled(),
    )
    expect(
      screen.getByText(
        '/Applications/Browser History Backup.app --worker mcp-server',
      ),
    ).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Backups' }))
    await screen.findByRole('heading', { name: 'Backups' })
    await user.click(screen.getByRole('button', { name: 'Preview upload' }))
    await waitFor(() =>
      expect(mockBackend.previewRemoteBackup).toHaveBeenCalled(),
    )
    await user.click(screen.getByRole('button', { name: 'Upload now' }))
    await waitFor(() => expect(mockBackend.runRemoteBackup).toHaveBeenCalled())
    await user.click(
      screen.getAllByRole('button', { name: /Run backup now/ })[0],
    )
    await waitFor(() => expect(mockBackend.runBackupNow).toHaveBeenCalled())
  })

  test('supports dry-run and full import previews', async () => {
    const user = userEvent.setup()
    mockBackend.inspectTakeout.mockResolvedValue({
      dryRun: true,
      sourcePath: '/tmp/takeout.zip',
      recognizedFiles: batchDetail.recognizedFiles,
      quarantinedFiles: [],
      previewEntries: batchDetail.previewEntries,
      candidateItems: 12,
      importedItems: 0,
      duplicateItems: 0,
      notes: ['Dry-run only'],
      importBatch: null,
    })
    mockBackend.importTakeout.mockResolvedValue({
      dryRun: false,
      sourcePath: '/tmp/takeout.zip',
      recognizedFiles: batchDetail.recognizedFiles,
      quarantinedFiles: [],
      previewEntries: batchDetail.previewEntries,
      candidateItems: 12,
      importedItems: 10,
      duplicateItems: 2,
      notes: ['Imported'],
      importBatch: batchDetail.batch,
    })

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Import' }))
    await user.type(
      screen.getByPlaceholderText('/Users/you/Downloads/takeout.zip'),
      '/tmp/takeout.zip',
    )
    await user.click(screen.getAllByRole('button', { name: 'Dry-run' })[0])
    await waitFor(() =>
      expect(mockBackend.inspectTakeout).toHaveBeenCalledWith({
        sourcePath: '/tmp/takeout.zip',
        dryRun: true,
      }),
    )
    expect(screen.getByText('Dry-run only')).toBeInTheDocument()
    await user.click(
      screen.getAllByRole('button', { name: 'Import supported files' })[0],
    )
    await waitFor(() =>
      expect(mockBackend.importTakeout).toHaveBeenCalledWith({
        sourcePath: '/tmp/takeout.zip',
        dryRun: false,
      }),
    )
    await waitFor(() =>
      expect(
        screen.getAllByText('Takeout import wrote 10 records.').length,
      ).toBeGreaterThan(0),
    )
  })

  test('supports initialized setup edits, scheduler workflow checks, and due-skip backup reviews', async () => {
    const user = userEvent.setup()
    mockBackend.saveConfig.mockImplementation((config) =>
      Promise.resolve(
        makeSnapshot({
          config: structuredClone(config),
        }),
      ),
    )
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: true,
      reason: 'Backup skipped because the current archive is already fresh.',
      run: makeSnapshot().recentRuns[0],
      profiles: [],
      manifestPath: null,
      gitCommit: null,
      warnings: [],
      remoteBackup: null,
    })

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Setup' }))
    await screen.findByText(
      'Choose which browser profiles should be included in backups.',
    )
    await user.click(screen.getByRole('button', { name: 'Apply preview' }))
    await screen.findByText('Generate a schedule preview first.')

    const dueAfterInput = screen.getByRole('spinbutton', {
      name: 'Back up only when at least this many hours have passed',
    })
    await user.clear(dueAfterInput)
    await user.type(dueAfterInput, '96')

    const intervalInput = screen.getByRole('spinbutton', {
      name: 'Wake-up check interval (hours)',
    })
    await user.clear(intervalInput)
    await user.type(intervalInput, '12')

    await user.click(
      screen.getByLabelText('Capture favicons alongside history snapshots'),
    )
    await user.click(
      screen.getByLabelText(
        'Commit manifests and audit artifacts into the local git repository',
      ),
    )
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Target platform' }),
      'windows',
    )

    await user.click(
      screen.getByRole('button', { name: 'Preview native schedule' }),
    )
    await screen.findByText('Scheduler preview is ready.')
    for (const button of screen.getAllByRole('button', {
      name: 'Mark complete',
    })) {
      await user.click(button)
    }

    const manualAlternative = screen
      .getByText('Manual alternative')
      .closest('.subsection')
    expect(manualAlternative).not.toBeNull()
    await user.click(
      within(manualAlternative as HTMLElement).getByRole('button', {
        name: 'Preview command',
      }),
    )

    await user.click(screen.getByRole('button', { name: 'Save setup' }))
    await waitFor(() =>
      expect(mockBackend.saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          dueAfterHours: 96,
          scheduleCheckIntervalHours: 12,
          captureFavicons: false,
          gitEnabled: false,
        }),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'Run backup now' }))
    await screen.findByText(
      'Backup skipped because the current archive is already fresh.',
    )
  })

  test('validates encrypted setup and supports plaintext initialization', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockReset().mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          initialized: false,
          archiveMode: 'Encrypted',
          selectedProfileIds: [],
        },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          encrypted: true,
          unlocked: false,
        },
      }),
    )
    mockBackend.initializeArchive.mockImplementation((config) =>
      Promise.resolve(
        makeSnapshot({
          config: structuredClone(config),
          archiveStatus: {
            ...structuredClone(baseSnapshot.archiveStatus),
            initialized: true,
            encrypted: config.archiveMode === 'Encrypted',
            unlocked: config.archiveMode !== 'Encrypted',
          },
        }),
      ),
    )

    render(<App />)

    await screen.findByText(
      'Choose which browser profiles should be included in backups.',
    )
    const profileCheckboxes = screen.getAllByRole('checkbox')
    await user.click(profileCheckboxes[1])

    await user.type(
      screen.getAllByPlaceholderText('Store this in your password manager')[0],
      'one-password',
    )
    await user.type(
      screen.getAllByPlaceholderText('Store this in your password manager')[1],
      'another-password',
    )
    await user.click(screen.getByRole('button', { name: 'Create archive' }))
    await screen.findByText(
      'Encrypted mode requires matching master passwords before initialization.',
    )

    await user.click(screen.getByRole('button', { name: 'Plaintext' }))
    await user.click(screen.getByRole('button', { name: 'Create archive' }))

    await waitFor(() =>
      expect(mockBackend.initializeArchive).toHaveBeenCalledWith(
        expect.objectContaining({
          archiveMode: 'Plaintext',
          initialized: true,
        }),
        null,
      ),
    )
    expect(mockBackend.resetLocalSecretVault).toHaveBeenCalled()
    expect(mockBackend.keyringClearDatabaseKey).toHaveBeenCalled()
  })

  test('initializes encrypted archives without remembering the key when keyring storage is disabled', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockReset().mockResolvedValue(
      makeSnapshot({
        config: {
          ...structuredClone(baseSnapshot.config),
          initialized: false,
          archiveMode: 'Encrypted',
          rememberDatabaseKeyInKeyring: false,
          selectedProfileIds: [],
        },
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          initialized: false,
          encrypted: true,
          unlocked: false,
        },
      }),
    )

    render(<App />)

    await screen.findByText(
      'Choose which browser profiles should be included in backups.',
    )
    const profileCheckboxes = screen.getAllByRole('checkbox')
    await user.click(profileCheckboxes[1])
    await user.type(
      screen.getAllByPlaceholderText('Store this in your password manager')[0],
      'matching-password',
    )
    await user.type(
      screen.getAllByPlaceholderText('Store this in your password manager')[1],
      'matching-password',
    )
    await user.click(screen.getByRole('button', { name: 'Create archive' }))

    await waitFor(() =>
      expect(mockBackend.keyringClearDatabaseKey).toHaveBeenCalled(),
    )
  })

  test('supports explorer filters, multiple export formats, backup selection, and warning summaries', async () => {
    const user = userEvent.setup()
    mockBackend.runBackupNow.mockResolvedValue({
      dueSkipped: false,
      reason: null,
      run: makeSnapshot().recentRuns[0],
      profiles: [],
      manifestPath: '/tmp/browser-history-backup/audit/manifests/run-4.json',
      gitCommit: 'def456',
      warnings: ['A locked profile was skipped.'],
      remoteBackup: null,
    })

    render(<App />)

    await screen.findByText('History explorer')
    await user.type(screen.getByLabelText('Search'), 'example')
    await user.type(screen.getByLabelText('Domain'), 'example.com')
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Profile' }),
      'chrome:Default',
    )

    await user.click(screen.getByRole('button', { name: 'Markdown' }))
    await waitFor(() =>
      expect(mockBackend.exportHistory).toHaveBeenCalledWith(
        expect.objectContaining({
          format: 'markdown',
          query: expect.objectContaining({
            domain: 'example.com',
            profileId: 'chrome:Default',
          }),
        }),
      ),
    )
    await user.click(screen.getByRole('button', { name: 'Text' }))
    await user.click(screen.getByRole('button', { name: 'JSONL' }))
    await user.click(
      screen.getByRole('button', {
        name: /Example https:\/\/example\.com/,
      }),
    )

    await user.click(screen.getByRole('button', { name: 'Backups' }))
    await screen.findByRole('heading', { name: 'Backups' })
    await user.click(screen.getByRole('button', { name: /#3 .*run-3-hash/ }))
    await user.click(
      screen.getAllByRole('button', { name: /Run backup now/ })[0],
    )

    await waitFor(() => expect(mockBackend.runBackupNow).toHaveBeenCalled())
    await screen.findByText('A locked profile was skipped.')
  })

  test('covers analysis toggles, provider editing, validation, and integration previews', async () => {
    const user = userEvent.setup()
    mockBackend.previewAiIntegrations.mockResolvedValue({
      mcpCommand:
        '/Applications/Browser History Backup.app --worker mcp-server',
      manualSteps: ['Enable MCP in Settings.', 'Copy the generated snippet.'],
      generatedFiles: [
        {
          relativePath: 'integrations/browser-history-backup-mcp.json',
          absolutePath:
            '/tmp/browser-history-backup/integrations/browser-history-backup-mcp.json',
          purpose: 'MCP snippet',
          contents: '{"mcpServers":{"browser-history-backup":{}}}',
        },
      ],
      warnings: ['Update your MCP client configuration after saving.'],
    })
    mockBackend.buildAiIndex
      .mockRejectedValueOnce(new Error('Index rebuild failed.'))
      .mockResolvedValue({
        providerId: 'embedding-preview',
        model: 'text-embedding-3-large',
        indexedItems: 8,
        updatedItems: 2,
        skippedItems: 0,
        removedItems: 0,
        lastIndexedAt: '2026-04-03T12:20:00.000Z',
        notes: ['Index built.'],
      })

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Analysis' }))
    await screen.findByText('Add LLM provider')

    await user.click(
      screen.getByLabelText('Enable optional AI analysis features'),
    )
    await user.click(screen.getByLabelText('Enable the assistant'))
    await user.click(screen.getByLabelText('Enable semantic indexing'))
    await user.click(
      screen.getByLabelText(
        'Refresh the semantic index after successful backups',
      ),
    )
    await user.click(screen.getByLabelText('Enable MCP integration'))
    await user.click(screen.getByLabelText('Enable skill export guidance'))
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Default LLM provider' }),
      '',
    )
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Default embedding provider' }),
      '',
    )
    const topKInput = screen.getByRole('spinbutton', {
      name: 'Retrieval top-k',
    })
    await user.clear(topKInput)
    await user.type(topKInput, '12')
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Analysis profile filter' }),
      'chrome:Default',
    )
    await user.clear(
      screen.getByRole('textbox', { name: 'Assistant system prompt' }),
    )
    await user.type(
      screen.getByRole('textbox', { name: 'Assistant system prompt' }),
      'Focus on auditability and browser provenance.',
    )

    await user.click(
      screen.getByRole('button', { name: 'Run semantic search' }),
    )
    await screen.findByText('Enter a semantic search query first.')
    await user.click(screen.getByRole('button', { name: 'Ask assistant' }))
    await screen.findByText('Enter a question for the assistant first.')

    const llmPanel = getProviderPanel('LLM providers')
    await user.click(within(llmPanel).getByRole('button', { name: 'Save key' }))
    await screen.findByText('Enter an API key for the selected provider first.')
    await user.type(
      within(llmPanel).getByPlaceholderText('sk-...'),
      'llm-secret',
    )
    await user.click(within(llmPanel).getByRole('button', { name: 'Save key' }))
    await waitFor(() =>
      expect(mockBackend.storeAiProviderApiKey).toHaveBeenCalledWith({
        providerId: 'llm-preview',
        apiKey: 'llm-secret',
      }),
    )
    await user.click(
      within(llmPanel).getByRole('button', { name: 'Clear key' }),
    )
    await waitFor(() =>
      expect(mockBackend.clearAiProviderApiKey).toHaveBeenCalledWith(
        'llm-preview',
      ),
    )
    await user.click(
      within(llmPanel).getByRole('button', { name: 'Add LLM provider' }),
    )
    const llmRadios = within(llmPanel).getAllByRole('radio')
    await user.click(llmRadios[1])
    await user.type(
      within(llmPanel).getAllByRole('textbox', { name: 'Provider name' })[1],
      ' Local',
    )
    await user.click(
      within(llmPanel).getAllByRole('button', { name: 'Remove provider' })[1],
    )

    const embeddingPanel = getProviderPanel('Embedding providers')
    await user.type(
      within(embeddingPanel).getByPlaceholderText('sk-...'),
      'embed-secret',
    )
    await user.click(
      within(embeddingPanel).getByRole('button', { name: 'Save key' }),
    )
    await user.click(
      within(embeddingPanel).getByRole('button', { name: 'Clear key' }),
    )
    await user.click(
      within(embeddingPanel).getByRole('button', {
        name: 'Add embedding provider',
      }),
    )
    const embeddingRadios = within(embeddingPanel).getAllByRole('radio')
    await user.click(embeddingRadios[1])
    await user.clear(
      within(embeddingPanel).getAllByRole('spinbutton', {
        name: 'Embedding dimensions',
      })[1],
    )
    await user.type(
      within(embeddingPanel).getAllByRole('spinbutton', {
        name: 'Embedding dimensions',
      })[1],
      '3072',
    )
    await user.click(
      within(embeddingPanel).getAllByRole('button', {
        name: 'Remove provider',
      })[1],
    )

    await user.type(screen.getByLabelText('Domain'), 'example.com')
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Profile' }),
      'chrome:Default',
    )
    await user.click(screen.getByRole('button', { name: 'Full rebuild' }))
    await screen.findByText('Index rebuild failed.')
    await user.click(screen.getByRole('button', { name: 'Build index' }))
    await waitFor(() =>
      expect(mockBackend.buildAiIndex).toHaveBeenCalledWith({
        providerId: null,
        fullRebuild: false,
        limit: null,
      }),
    )
    await user.click(
      screen.getByRole('button', { name: 'Preview MCP and skill files' }),
    )
    await screen.findByText(
      'Update your MCP client configuration after saving.',
    )

    const integrationSection = screen
      .getByText('Enable MCP in Settings.', { selector: 'li' })
      .closest('section')
    if (!integrationSection) {
      throw new Error('Expected MCP integration section to exist')
    }
    for (const button of within(integrationSection).getAllByRole('button', {
      name: 'Preview command',
    })) {
      await user.click(button)
    }
  })

  test('covers import workflow controls, preview failures, quarantined files, and doctor output', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockReset().mockResolvedValue(
      makeSnapshot({
        recentImportBatches: [
          {
            ...structuredClone(baseSnapshot.recentImportBatches[0]),
            id: 7,
            status: 'running',
          },
          {
            ...structuredClone(baseSnapshot.recentImportBatches[0]),
            id: 8,
            status: 'pending' as never,
            sourcePath: '/tmp/archive-2.zip',
          },
        ],
      }),
    )
    mockBackend.previewImportBatch
      .mockRejectedValueOnce(new Error('Batch preview failed.'))
      .mockResolvedValue({
        ...batchDetail,
        batch: {
          ...batchDetail.batch,
          id: 8,
          status: 'pending' as never,
          sourcePath: '/tmp/archive-2.zip',
        },
        quarantinedFiles: [
          { path: 'quarantine.csv', kind: 'csv', status: 'quarantined' },
        ],
      })
    mockBackend.inspectTakeout.mockResolvedValue({
      dryRun: true,
      sourcePath: '/tmp/takeout.zip',
      recognizedFiles: batchDetail.recognizedFiles,
      quarantinedFiles: [
        { path: 'quarantine.csv', kind: 'csv', status: 'quarantined' },
      ],
      previewEntries: [],
      candidateItems: 12,
      importedItems: 0,
      duplicateItems: 0,
      notes: ['Needs review'],
      importBatch: null,
    })
    mockBackend.importTakeout.mockResolvedValue({
      dryRun: false,
      sourcePath: '/tmp/takeout.zip',
      recognizedFiles: batchDetail.recognizedFiles,
      quarantinedFiles: [
        { path: 'quarantine.csv', kind: 'csv', status: 'quarantined' },
      ],
      previewEntries: batchDetail.previewEntries,
      candidateItems: 12,
      importedItems: 10,
      duplicateItems: 2,
      notes: ['Imported'],
      importBatch: batchDetail.batch,
    })
    mockBackend.doctor.mockResolvedValue({
      generatedAt: '2026-04-03T12:10:00.000Z',
      checks: [
        {
          name: 'Archive writable',
          status: 'ok',
          message: 'App root is writable.',
        },
        {
          name: 'Schedule audit',
          status: 'warn',
          message: 'No recent schedule audit found.',
        },
      ],
    })

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Import' }))
    await screen.findByText('Takeout import and health checks')
    await screen.findByText('Batch preview failed.')
    expect(screen.getByText('Running')).toBeInTheDocument()
    expect(screen.getAllByText('pending').length).toBeGreaterThan(0)

    await user.click(screen.getAllByRole('button', { name: 'Dry-run' })[0])
    await screen.findByText(
      'Enter a Google Takeout zip path or an extracted folder path first.',
    )
    await user.type(
      screen.getByPlaceholderText('/Users/you/Downloads/takeout.zip'),
      '/tmp/takeout.zip',
    )

    const workflowSection = getClosestSection(
      'Review every import step before it changes the archive',
    )
    await user.click(
      within(workflowSection).getByRole('button', { name: 'Dry-run' }),
    )
    await screen.findByText('Needs review')
    await screen.findByText(
      'No preview rows are available for this selection yet.',
    )
    await waitFor(() =>
      expect(screen.getAllByText('quarantine.csv').length).toBeGreaterThan(0),
    )
    await user.click(
      within(workflowSection).getAllByRole('button', {
        name: 'Mark complete',
      })[0],
    )
    await user.click(
      within(workflowSection).getByRole('button', {
        name: 'Import supported files',
      }),
    )
    await waitFor(() =>
      expect(mockBackend.importTakeout).toHaveBeenCalledWith({
        sourcePath: '/tmp/takeout.zip',
        dryRun: false,
      }),
    )
    await user.click(
      within(workflowSection).getAllByRole('button', {
        name: 'Mark complete',
      })[0],
    )
    await user.click(screen.getByRole('button', { name: /#8/ }))
    await waitFor(() =>
      expect(mockBackend.previewImportBatch).toHaveBeenCalledWith(8),
    )
    ;(window.confirm as ReturnType<typeof vi.fn>).mockReturnValueOnce(false)
    await user.click(screen.getByRole('button', { name: 'Revert batch' }))
    expect(mockBackend.revertImportBatch).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: 'Run doctor' }))
    await screen.findByText('Archive writable')
    await screen.findByText('No recent schedule audit found.')
  })

  test('supports manual unlock, remote backup settings, path actions, and autostart sync', async () => {
    const user = userEvent.setup()
    const lockedSnapshot = makeSnapshot({
      config: {
        ...structuredClone(baseSnapshot.config),
        rememberDatabaseKeyInKeyring: true,
        appAutostart: false,
      },
      archiveStatus: {
        ...structuredClone(baseSnapshot.archiveStatus),
        unlocked: false,
      },
    })
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(lockedSnapshot)
      .mockResolvedValue(
        makeSnapshot({
          config: {
            ...structuredClone(baseSnapshot.config),
            rememberDatabaseKeyInKeyring: true,
            appAutostart: false,
          },
        }),
      )
    mockBackend.saveConfig.mockImplementation((config) =>
      Promise.resolve(
        makeSnapshot({
          config: structuredClone(config),
        }),
      ),
    )
    strongholdMocks.readDatabaseKeyStronghold.mockResolvedValue(
      'manual-session-key',
    )
    autostartMocks.isEnabled
      .mockReset()
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce(true)
      .mockResolvedValue(false)

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Settings' }))
    await screen.findByText('Data and build info')

    await user.click(screen.getByRole('button', { name: 'Unlock archive' }))
    await screen.findByText(
      'Enter the master password you stored in your password manager.',
    )
    await user.type(
      screen.getByPlaceholderText('Store this in your password manager'),
      'manual-password',
    )
    await user.click(screen.getByRole('button', { name: 'Unlock archive' }))
    await screen.findByText('Encrypted archive unlocked for this session.')

    await user.click(screen.getByRole('button', { name: 'Copy App data root' }))
    await user.click(
      screen.getByRole('button', { name: 'Copy Archive database' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'Open Audit repository' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'Copy Audit repository' }),
    )

    await user.click(screen.getByLabelText('Enable remote backup'))
    await user.click(screen.getByLabelText('Use path-style requests'))
    await user.click(
      screen.getByLabelText(
        'Upload automatically after each successful backup',
      ),
    )
    await user.type(screen.getByLabelText('Bucket'), 'history-bucket')
    await user.clear(screen.getByLabelText('Region'))
    await user.type(screen.getByLabelText('Region'), 'us-west-2')
    await user.clear(screen.getByLabelText('Object key prefix'))
    await user.type(
      screen.getByLabelText('Object key prefix'),
      'browser-history-backup/archive',
    )
    await user.type(
      screen.getByPlaceholderText('https://s3.example.com'),
      'https://s3.example.com',
    )
    await user.clear(screen.getByPlaceholderText('https://s3.example.com'))

    await user.click(screen.getByRole('button', { name: 'Save credentials' }))
    await screen.findByText(
      'Enter the S3 access key ID and secret access key before saving credentials.',
    )
    await user.type(screen.getByLabelText('Access key ID'), 'AKIA456')
    await user.type(screen.getByLabelText('Secret access key'), 'secret456')
    await user.click(screen.getByRole('button', { name: 'Save credentials' }))
    await waitFor(() =>
      expect(mockBackend.storeS3Credentials).toHaveBeenCalledWith({
        accessKeyId: 'AKIA456',
        secretAccessKey: 'secret456',
      }),
    )

    await user.click(screen.getByRole('button', { name: 'Preview upload' }))
    await screen.findByText('Review the bundle.')
    await user.click(screen.getByRole('button', { name: 'Preview command' }))

    await user.click(
      screen.getByLabelText('Launch the app automatically at login'),
    )
    await user.click(screen.getByRole('button', { name: 'Save settings' }))
    await waitFor(() => expect(autostartMocks.enable).toHaveBeenCalled())

    await user.click(
      screen.getByLabelText('Launch the app automatically at login'),
    )
    await user.click(screen.getByRole('button', { name: 'Save settings' }))
    await waitFor(() => expect(autostartMocks.disable).toHaveBeenCalled())

    await user.click(screen.getByRole('button', { name: 'Rotate archive key' }))
    await screen.findByText(
      'Enter a new master password before rotating the encrypted archive.',
    )
    await user.type(
      screen.getByLabelText('New master password'),
      'rotated-password',
    )
    await user.click(screen.getByRole('button', { name: 'Rotate archive key' }))
    await waitFor(() =>
      expect(mockBackend.keyringStoreDatabaseKey).toHaveBeenCalled(),
    )

    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Interface language' }),
      'zh-TW',
    )
  })

  test('guards security actions until a session key is available', async () => {
    const user = userEvent.setup()

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Settings' }))
    await screen.findByText('Data and build info')

    await user.click(
      screen.getByRole('button', { name: 'Remember current key' }),
    )
    await screen.findByText(
      'Unlock the archive before saving the current key into the system keyring.',
    )
    await user.click(screen.getByRole('button', { name: 'Rotate archive key' }))
    await screen.findByText(
      'Unlock the archive first so the app can rotate the encryption key.',
    )
  })

  test('surfaces unlock errors when the stronghold snapshot has no database key', async () => {
    const user = userEvent.setup()
    mockBackend.getAppSnapshot.mockReset().mockResolvedValue(
      makeSnapshot({
        archiveStatus: {
          ...structuredClone(baseSnapshot.archiveStatus),
          unlocked: false,
        },
      }),
    )
    strongholdMocks.readDatabaseKeyStronghold.mockResolvedValue(null)

    render(<App />)

    await screen.findByText('History explorer')
    await user.click(screen.getByRole('button', { name: 'Settings' }))
    await screen.findByText('Data and build info')
    await user.type(
      screen.getByPlaceholderText('Store this in your password manager'),
      'missing-stronghold-key',
    )
    await user.click(screen.getByRole('button', { name: 'Unlock archive' }))
    await screen.findByText(
      'No database key was found in the Stronghold snapshot for that password.',
    )
  })

  test('auto-unlocks with a remembered key and supports settings security actions', async () => {
    const user = userEvent.setup()
    const lockedSnapshot = makeSnapshot({
      archiveStatus: {
        ...structuredClone(baseSnapshot.archiveStatus),
        unlocked: false,
      },
    })
    mockBackend.keyringGetDatabaseKey.mockResolvedValue('remembered-key')
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(lockedSnapshot)
      .mockResolvedValue(makeSnapshot())

    render(<App />)

    await screen.findByText('Unlocked with the remembered database key.')
    await waitFor(() =>
      expect(mockBackend.setSessionDatabaseKey).toHaveBeenCalledWith(
        'remembered-key',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'Settings' }))
    await screen.findByText('Data and build info')
    await user.type(
      screen.getByLabelText('New master password'),
      'new-master-password',
    )
    await user.click(
      screen.getByRole('button', { name: 'Remember current key' }),
    )
    await waitFor(() =>
      expect(mockBackend.keyringStoreDatabaseKey).toHaveBeenCalledWith(
        'remembered-key',
      ),
    )
    await user.click(
      screen.getByRole('button', { name: 'Clear remembered key' }),
    )
    await waitFor(() =>
      expect(mockBackend.keyringClearDatabaseKey).toHaveBeenCalled(),
    )
    await user.click(screen.getByRole('button', { name: 'Rotate archive key' }))
    await waitFor(() => expect(mockBackend.rekeyArchive).toHaveBeenCalled())
    await user.click(
      screen.getByRole('button', { name: 'Convert to plaintext' }),
    )
    await waitFor(() =>
      expect(mockBackend.clearSessionDatabaseKey).toHaveBeenCalled(),
    )
    await user.type(screen.getByLabelText('Access key ID'), 'AKIA123')
    await user.type(screen.getByLabelText('Secret access key'), 'secret123')
    await user.click(screen.getByRole('button', { name: 'Save credentials' }))
    await waitFor(() =>
      expect(mockBackend.storeS3Credentials).toHaveBeenCalledWith({
        accessKeyId: 'AKIA123',
        secretAccessKey: 'secret123',
      }),
    )
    await user.click(screen.getByRole('button', { name: 'Clear credentials' }))
    await waitFor(() =>
      expect(mockBackend.clearS3Credentials).toHaveBeenCalled(),
    )
    await user.click(
      screen.getByRole('button', { name: /open archive database/i }),
    )
    await waitFor(() =>
      expect(mockBackend.openPathInFileManager).toHaveBeenCalledWith(
        '/tmp/browser-history-backup/archive.sqlite',
      ),
    )
  })

  test('auto-unlocks an uninitialized archive and keeps the empty setup review visible', async () => {
    const emptySetupSnapshot = makeSnapshot({
      config: {
        ...structuredClone(baseSnapshot.config),
        initialized: false,
        rememberDatabaseKeyInKeyring: true,
        selectedProfileIds: [],
      },
      archiveStatus: {
        ...structuredClone(baseSnapshot.archiveStatus),
        initialized: false,
        unlocked: false,
      },
      browserProfiles: [],
      recentRuns: [],
      recentImportBatches: [],
    })
    const unlockedSetupSnapshot = makeSnapshot({
      ...structuredClone(emptySetupSnapshot),
      archiveStatus: {
        ...structuredClone(emptySetupSnapshot.archiveStatus),
        unlocked: true,
      },
    })

    mockBackend.keyringGetDatabaseKey.mockResolvedValue('bootstrap-key')
    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(emptySetupSnapshot)
      .mockResolvedValue(unlockedSetupSnapshot)

    render(<App />)

    await screen.findByText('Unlocked with the remembered database key.')
    await waitFor(() =>
      expect(mockBackend.setSessionDatabaseKey).toHaveBeenCalledWith(
        'bootstrap-key',
      ),
    )
    expect(
      screen.getAllByText(
        'Select at least one browser profile to see the paths and manual commands.',
      ).length,
    ).toBeGreaterThanOrEqual(2)

    const reviewSection = getClosestSection(
      'Confirm the local paths and the password-recovery implications before continuing.',
    )
    expect(
      within(reviewSection).queryByRole('button', { name: 'Run backup now' }),
    ).not.toBeInTheDocument()

    const scheduleSection = getClosestSection(
      'Preview the OS-native scheduler artifact, inspect the commands, then apply it only if you want the app to do it for you.',
    )
    expect(
      within(scheduleSection).getByText(
        'Generate a schedule preview to inspect the file contents, commands, and rollback path.',
      ),
    ).toBeInTheDocument()

    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: 'Settings' }))
    await screen.findByText(
      'Initialize the archive first. Encryption changes and key rotation become available after that.',
    )
  })

  test('covers sparse fallback states across setup, analysis, backups, import, and settings views', async () => {
    const user = userEvent.setup()
    const sparseSnapshot = makeSnapshot({
      config: {
        ...structuredClone(baseSnapshot.config),
        archiveMode: 'Plaintext',
        preferredLanguage: 'system',
        selectedProfileIds: [],
        appAutostart: true,
        remoteBackup: {
          ...structuredClone(baseSnapshot.config.remoteBackup),
          enabled: true,
          bucket: 'history-bucket',
          region: 'us-west-2',
          endpoint: 'https://s3.example.com',
          prefix: 'browser-history-backup/archive',
          pathStyle: false,
          uploadAfterBackup: true,
          credentialsSaved: true,
          lastUploadedAt: '2026-04-02T18:30:00.000Z',
          lastUploadedObjectKey: 'browser-history-backup/archive/latest.zip',
          lastError: 'Last upload expired before completion.',
        },
        ai: {
          ...structuredClone(baseSnapshot.config.ai),
          enabled: false,
          assistantEnabled: false,
          semanticIndexEnabled: false,
          mcpEnabled: false,
          skillEnabled: false,
          autoIndexAfterBackup: false,
          llmProviderId: null,
          embeddingProviderId: null,
          assistantSystemPrompt: '',
          llmProviders: [
            {
              ...structuredClone(llmProvider),
              id: 'llm-empty',
              name: '',
              apiKeySaved: false,
            },
          ],
          embeddingProviders: [
            {
              ...structuredClone(embeddingProvider),
              id: 'embed-empty',
              name: '',
            },
          ],
        },
      },
      archiveStatus: {
        ...structuredClone(baseSnapshot.archiveStatus),
        encrypted: false,
        unlocked: true,
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
      browserProfiles: [
        {
          profileId: 'firefox:offline',
          profileName: 'Offline',
          browserFamily: 'firefox',
          browserName: 'Firefox',
          userName: null,
          profilePath:
            '/Users/demo/Library/Application Support/Firefox/Profiles/offline',
          historyPath: null,
          faviconsPath: null,
          historyExists: false,
          browserVersion: null,
          historyFileName: 'places.sqlite',
        },
      ],
      recentRuns: [],
      recentImportBatches: [],
    })
    const runningSnapshot = makeSnapshot({
      ...structuredClone(sparseSnapshot),
      recentRuns: [
        {
          id: 11,
          startedAt: '2026-04-03T12:30:00.000Z',
          finishedAt: null,
          status: 'running',
          manifestHash: null,
          profilesProcessed: 0,
          newVisits: 0,
          newUrls: 0,
          newDownloads: 0,
        },
        {
          id: 10,
          startedAt: '2026-04-03T11:30:00.000Z',
          finishedAt: '2026-04-03T11:31:00.000Z',
          status: 'completed',
          manifestHash: 'older-run',
          profilesProcessed: 1,
          newVisits: 2,
          newUrls: 1,
          newDownloads: 0,
        },
      ],
    })

    mockBackend.getAppSnapshot
      .mockReset()
      .mockResolvedValueOnce(sparseSnapshot)
      .mockResolvedValue(runningSnapshot)
    mockBackend.getAppBuildInfo.mockResolvedValue({
      ...buildInfo,
      gitDirty: true,
    })
    mockBackend.queryHistory.mockResolvedValue({
      total: 0,
      items: [],
    })
    mockBackend.searchAiHistory.mockResolvedValue({
      total: 1,
      providerId: 'embed-empty',
      model: 'text-embedding-3-large',
      items: [
        {
          historyId: 91,
          profileId: 'firefox:offline',
          url: 'https://fallback.example/search',
          title: '',
          domain: 'fallback.example',
          visitedAt: '2026-04-03T12:00:00.000Z',
          score: 0.42,
          matchReason: 'Sparse-state preview',
        },
      ],
      notes: [],
    })
    mockBackend.askAiAssistant
      .mockReset()
      .mockResolvedValueOnce({
        answer: 'No citations were attached to this preview response.',
        providerId: 'llm-empty',
        embeddingProviderId: 'embed-empty',
        citations: [],
        notes: [],
      })
      .mockResolvedValueOnce({
        answer: 'The sparse test response cites the URL directly.',
        providerId: 'llm-empty',
        embeddingProviderId: 'embed-empty',
        citations: [
          {
            historyId: 91,
            profileId: 'firefox:offline',
            url: 'https://fallback.example/search',
            title: '',
            visitedAt: '2026-04-03T12:00:00.000Z',
            score: 0.42,
          },
        ],
        notes: [],
      })
    mockBackend.previewAiIntegrations.mockResolvedValue({
      mcpCommand: 'browser-history-backup --worker mcp-server',
      manualSteps: [],
      generatedFiles: [],
      warnings: [],
    })
    mockBackend.runBackupNow
      .mockReset()
      .mockResolvedValueOnce({
        dueSkipped: true,
        reason: null,
        run: null,
        profiles: [],
        manifestPath: null,
        gitCommit: null,
        warnings: [],
        remoteBackup: null,
      })
      .mockResolvedValueOnce({
        dueSkipped: false,
        reason: null,
        run: runningSnapshot.recentRuns[0],
        profiles: [],
        manifestPath: null,
        gitCommit: null,
        warnings: [],
        remoteBackup: {
          uploaded: true,
          bundlePath: '/tmp/browser-history-backup-remote.zip',
          objectKey: 'browser-history-backup/archive/latest.zip',
          uploadUrl:
            'https://s3.example.com/browser-history-backup/archive/latest.zip',
          message: 'Remote upload finished.',
        },
      })
    mockBackend.previewRemoteBackup.mockResolvedValue({
      bundlePath: '/tmp/browser-history-backup-remote.zip',
      objectKey: 'browser-history-backup/archive/latest.zip',
      uploadUrl:
        'https://s3.example.com/browser-history-backup/archive/latest.zip',
      previewCommand: 'curl -T bundle.zip https://s3.example.com/upload',
      manualSteps: [],
      warnings: [],
    })
    mockBackend.importTakeout.mockResolvedValue({
      dryRun: false,
      sourcePath: '/tmp/sparse-takeout.zip',
      recognizedFiles: [],
      quarantinedFiles: [],
      previewEntries: [],
      candidateItems: 0,
      importedItems: 0,
      duplicateItems: 0,
      notes: [],
      importBatch: null,
    })

    render(<App />)

    await screen.findByText('History explorer')
    expect(screen.getByText('Dirty')).toBeInTheDocument()
    await screen.findByText(
      'No matching rows yet. Try a broader search or run a backup.',
    )

    await user.click(screen.getByRole('button', { name: 'Setup' }))
    await screen.findByText('History database not found')
    await screen.findByText('No signed-in user metadata')
    await screen.findByText('Version unavailable')

    await user.click(screen.getByRole('button', { name: 'Analysis' }))
    await screen.findByText(
      'Ask a question after enabling AI analysis and selecting providers.',
    )
    expect(
      screen.getByRole('option', { name: 'llm-empty' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('option', { name: 'embed-empty' }),
    ).toBeInTheDocument()
    await user.type(screen.getByLabelText('Semantic search'), 'fallback')
    await user.click(
      screen.getByRole('button', { name: 'Run semantic search' }),
    )
    await waitFor(() =>
      expect(
        screen.getAllByText('https://fallback.example/search').length,
      ).toBeGreaterThanOrEqual(2),
    )
    await user.type(
      screen.getByLabelText('Question for the assistant'),
      'What did the sparse test find?',
    )
    await user.click(screen.getByRole('button', { name: 'Ask assistant' }))
    await screen.findByText(
      'No citations were attached to this preview response.',
    )
    await user.click(screen.getByRole('button', { name: 'Ask assistant' }))
    await screen.findByText('The sparse test response cites the URL directly.')
    await user.click(
      screen.getByRole('button', { name: 'Preview MCP and skill files' }),
    )
    await screen.findByText('browser-history-backup --worker mcp-server')

    await user.click(screen.getByRole('button', { name: 'Backups' }))
    await waitFor(() =>
      expect(
        screen.getAllByText(
          'No completed runs yet. Run the first backup to start the audit chain.',
        ).length,
      ).toBeGreaterThanOrEqual(3),
    )
    await user.click(
      screen.getAllByRole('button', { name: /Run backup now/ })[0],
    )
    await screen.findByText('Backup completed.')
    await user.click(screen.getByRole('button', { name: 'Preview upload' }))
    await screen.findByText('browser-history-backup/archive/latest.zip')
    await user.click(
      screen.getAllByRole('button', { name: /Run backup now/ })[0],
    )
    await screen.findByText('Remote upload finished.')
    await screen.findByText('Still running')
    await screen.findByText('Pending')

    await user.click(screen.getByRole('button', { name: 'Import' }))
    await waitFor(() =>
      expect(
        screen.getAllByText(
          'Run a dry-run first to see recognized files, quarantined files, and notes.',
        ).length,
      ).toBeGreaterThanOrEqual(2),
    )
    const importWorkflowSection = getClosestSection(
      'Review every import step before it changes the archive',
    )
    await user.click(
      within(importWorkflowSection).getAllByRole('button', {
        name: 'Mark complete',
      })[1],
    )
    await screen.findByText('No import batches have been recorded yet.')
    await user.type(
      screen.getByPlaceholderText('/Users/you/Downloads/takeout.zip'),
      '/tmp/sparse-takeout.zip',
    )
    await user.click(
      screen.getAllByRole('button', { name: 'Import supported files' })[0],
    )
    await screen.findByText('Takeout import wrote 0 records.')
    await screen.findByText('No directly importable files detected yet.')
    await screen.findByText('No quarantined files in this run.')
    expect(
      screen.getAllByText(
        'No preview rows are available for this selection yet.',
      ).length,
    ).toBeGreaterThanOrEqual(1)

    await user.click(screen.getByRole('button', { name: 'Settings' }))
    await screen.findByText('Data and build info')
    expect(
      screen.getByDisplayValue('/tmp/browser-history-backup'),
    ).toBeInTheDocument()
    expect(
      screen.getByLabelText('Launch the app automatically at login'),
    ).toBeChecked()
    expect(screen.getAllByText('Yes').length).toBeGreaterThan(0)
    expect(
      screen.getAllByText('browser-history-backup/archive/latest.zip').length,
    ).toBeGreaterThanOrEqual(1)
  })
})
