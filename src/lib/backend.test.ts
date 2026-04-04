import { beforeEach, describe, expect, test, vi } from 'vitest'

const { invoke, isTauri } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

import { backend, backendTestHarness } from './backend'
import type { AppConfig, SchedulePlan } from './types'

const config: AppConfig = {
  initialized: false,
  archiveMode: 'Encrypted',
  preferredLanguage: 'system',
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
}

const schedulePlan: SchedulePlan = {
  platform: 'macos',
  label: 'dev.example.browser-history-backup.backup',
  executablePath: '/Applications/Browser History Backup.app',
  generatedFiles: [],
  manualSteps: [],
  applyCommands: [],
  rollbackCommands: [],
  applySupported: false,
}

describe('backend facade', () => {
  beforeEach(() => {
    isTauri.mockReturnValue(false)
    invoke.mockReset()
  })

  test('covers browser preview commands with deterministic mock data', async () => {
    await expect(backend.getAppSnapshot()).resolves.toMatchObject({
      config: expect.objectContaining({ archiveMode: 'Encrypted' }),
    })
    await expect(backend.saveConfig(config)).resolves.toMatchObject({
      config: expect.objectContaining({ archiveMode: 'Encrypted' }),
    })
    await expect(
      backend.initializeArchive(config, 'key'),
    ).resolves.toMatchObject({
      config: expect.objectContaining({ archiveMode: 'Encrypted' }),
    })
    await expect(
      backend.rekeyArchive({ newMode: 'Plaintext', newKey: null }),
    ).resolves.toMatchObject({
      config: expect.objectContaining({ archiveMode: 'Encrypted' }),
    })
    await expect(
      backend.setSessionDatabaseKey('session-key'),
    ).resolves.toBeUndefined()
    await expect(backend.clearSessionDatabaseKey()).resolves.toBeUndefined()
    await expect(backend.runBackupNow()).resolves.toMatchObject({
      dueSkipped: false,
    })
    await expect(
      backend.queryHistory({
        q: 'sqlite',
        domain: null,
        profileId: null,
        limit: 10,
      }),
    ).resolves.toMatchObject({ total: 2 })
    await expect(
      backend.exportHistory({ query: { q: 'sqlite' }, format: 'jsonl' }),
    ).resolves.toMatchObject({ format: 'jsonl' })
    await expect(backend.previewRemoteBackup()).resolves.toMatchObject({
      bundlePath: expect.stringContaining('browser-history-backup-remote.zip'),
    })
    await expect(backend.runRemoteBackup()).resolves.toMatchObject({
      uploaded: false,
    })
    await expect(
      backend.inspectTakeout({ sourcePath: '/tmp/takeout', dryRun: true }),
    ).resolves.toMatchObject({
      dryRun: true,
      notes: ['Tauri is not available in browser preview mode.'],
    })
    await expect(
      backend.importTakeout({ sourcePath: '/tmp/takeout', dryRun: false }),
    ).resolves.toMatchObject({
      dryRun: true,
      notes: ['Tauri is not available in browser preview mode.'],
    })
    await expect(backend.previewImportBatch(7)).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'imported' }),
    })
    await expect(backend.revertImportBatch(7)).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'reverted' }),
    })
    await expect(backend.previewSchedule()).resolves.toMatchObject({
      platform: 'macos',
      applySupported: false,
    })
    await expect(backend.applySchedule(schedulePlan)).resolves.toMatchObject({
      applied: false,
    })
    await expect(backend.doctor()).resolves.toMatchObject({
      checks: [],
    })
    await expect(backend.keyringStatus()).resolves.toMatchObject({
      available: true,
      backend: 'Mock keyring',
    })
    await expect(backend.keyringGetDatabaseKey()).resolves.toBeNull()
    await expect(
      backend.keyringStoreDatabaseKey('secret'),
    ).resolves.toMatchObject({
      storedSecret: true,
    })
    await expect(backend.keyringClearDatabaseKey()).resolves.toMatchObject({
      storedSecret: false,
    })
    await expect(
      backend.storeS3Credentials({
        accessKeyId: 'key',
        secretAccessKey: 'secret',
      }),
    ).resolves.toMatchObject({
      storedSecret: false,
    })
    await expect(backend.clearS3Credentials()).resolves.toMatchObject({
      storedSecret: false,
    })
    await expect(backend.resetLocalSecretVault()).resolves.toBeUndefined()
  })

  test('delegates to Tauri invoke when running inside the desktop shell', async () => {
    isTauri.mockReturnValue(true)
    invoke.mockResolvedValue({ ok: true })

    await expect(backend.getAppSnapshot()).resolves.toEqual({ ok: true })
    expect(invoke).toHaveBeenCalledWith('app_snapshot', undefined)
  })

  test('throws when a mock command is not implemented in browser preview mode', async () => {
    await expect(
      backendTestHarness.call('inspect_takeout'),
    ).resolves.toMatchObject({
      sourcePath: '/tmp/takeout.zip',
      dryRun: true,
    })
    await expect(
      backendTestHarness.call('totally_unknown_command'),
    ).rejects.toThrow('Mock backend does not implement totally_unknown_command')
  })
})
