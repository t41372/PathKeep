import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'

const { autostartMocks, mockBackend } = vi.hoisted(() => ({
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
}))

vi.mock('./lib/backend', () => ({
  backend: mockBackend,
}))

vi.mock('./lib/stronghold', () => ({
  readDatabaseKeyStronghold: vi.fn().mockResolvedValue(null),
  storeDatabaseKeyStronghold: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('@tauri-apps/plugin-autostart', () => autostartMocks)

import App from './App'

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
        'You are an audit-first history research assistant.',
      llmProviders: [],
      embeddingProviders: [],
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
  browserProfiles: [],
  recentRuns: [],
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

describe('App integration', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'confirm', {
      writable: true,
      value: vi.fn().mockReturnValue(true),
    })

    mockBackend.getAppSnapshot.mockReset()
    mockBackend.getAppBuildInfo.mockResolvedValue(buildInfo)
    mockBackend.getAppSnapshot
      .mockResolvedValueOnce(baseSnapshot)
      .mockResolvedValueOnce({
        ...baseSnapshot,
        recentImportBatches: [revertedBatchDetail.batch],
      })
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
    mockBackend.doctor.mockResolvedValue({
      generatedAt: '2026-04-03T12:10:00.000Z',
      checks: [],
    })
    mockBackend.openPathInFileManager.mockResolvedValue(
      baseSnapshot.directories.appRoot,
    )
  })

  test('loads import batches and lets the user revert a dirty import batch', async () => {
    const user = userEvent.setup()

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
})
