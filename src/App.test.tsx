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
    resetLocalSecretVault: vi.fn(),
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
    appRoot: '/tmp/chrome-history-backup',
    configPath: '/tmp/chrome-history-backup/config.json',
    archiveDatabasePath: '/tmp/chrome-history-backup/archive.sqlite',
    auditRepoPath: '/tmp/chrome-history-backup/audit',
    manifestsDir: '/tmp/chrome-history-backup/audit/manifests',
    exportsDir: '/tmp/chrome-history-backup/exports',
    rawSnapshotsDir: '/tmp/chrome-history-backup/raw',
    stagingDir: '/tmp/chrome-history-backup/staging',
    quarantineDir: '/tmp/chrome-history-backup/quarantine',
    scheduleDir: '/tmp/chrome-history-backup/schedule',
    strongholdPath: '/tmp/chrome-history-backup/vault.hold',
    strongholdSaltPath: '/tmp/chrome-history-backup/vault.salt',
  },
  config: {
    initialized: true,
    archiveMode: 'Encrypted',
    preferredLanguage: 'en',
    dueAfterHours: 72,
    scheduleCheckIntervalHours: 6,
    checkpointDays: 90,
    captureFavicons: true,
    selectedProfileIds: ['Default'],
    gitEnabled: true,
    rememberDatabaseKeyInKeyring: false,
    appAutostart: false,
    remoteBackup: {
      enabled: false,
      bucket: '',
      region: 'us-east-1',
      endpoint: null,
      prefix: 'chrome-history-backup',
      pathStyle: true,
      uploadAfterBackup: false,
      credentialsSaved: false,
      lastUploadedAt: null,
      lastUploadedObjectKey: null,
      lastError: null,
    },
  },
  archiveStatus: {
    initialized: true,
    encrypted: true,
    unlocked: true,
    databasePath: '/tmp/chrome-history-backup/archive.sqlite',
    lastSuccessfulBackupAt: null,
    warning: null,
  },
  keyringStatus: {
    available: true,
    backend: 'Mock keyring',
    storedSecret: false,
    message: null,
  },
  chromeProfiles: [],
  recentRuns: [],
  recentImportBatches: [
    {
      id: 7,
      sourceKind: 'takeout',
      sourcePath: '/tmp/takeout.zip',
      profileId: 'takeout::takeout',
      createdAt: '2026-04-03T12:00:00.000Z',
      importedAt: '2026-04-03T12:01:00.000Z',
      revertedAt: null,
      status: 'imported',
      candidateItems: 12,
      importedItems: 10,
      duplicateItems: 2,
      visibleItems: 10,
      auditPath: '/tmp/chrome-history-backup/audit/import-batch-7.json',
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

describe('App integration', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'confirm', {
      writable: true,
      value: vi.fn().mockReturnValue(true),
    })

    mockBackend.getAppSnapshot.mockReset()
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
          profileId: 'Default',
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
})
