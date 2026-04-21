/**
 * @file preview-state.test.ts
 * @description Focused browser-preview state, history, and app-lock regressions for the backend facade.
 * @module lib/backend-tests/preview-state
 *
 * ## Responsibilities
 * - Preserve preview runtime assertions for disabled modules, updater fallbacks, archive prerequisites, and mock history pagination.
 * - Verify preview app-lock behavior, biometric warnings, and locked-surface guards in one focused suite.
 * - Reuse the shared preview config fixture while keeping the local `vi.hoisted` backend import mock.
 *
 * ## Not responsible for
 * - Tauri passthrough behavior, schedule/manual-review flows, or unrelated backend facade helpers.
 * - Changing preview semantics; this suite narrows ownership while keeping the existing assertions intact.
 * - Resetting global test state outside this suite.
 *
 * ## Dependencies
 * - Depends on Vitest, the mocked `@tauri-apps/api/core` guest surface, `../backend`, and `./test-helpers`.
 *
 * ## Performance notes
 * - Each test resets the in-memory preview harness before mutating only the state needed for that scenario.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'

const { invoke, isTauri } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

import { backend, backendTestHarness } from '../backend'
import { previewConfigFixture } from './test-helpers'

type HistoryQuery = Parameters<typeof backend.queryHistory>[0]

const baseHistoryQuery = {
  q: null,
  domain: null,
  profileId: null,
  browserKind: null,
  startTimeMs: null,
  endTimeMs: null,
  sort: 'newest' as const,
}

const appLockPasscode = '2468'
const makePreviewConfig = () => structuredClone(previewConfigFixture)
const queryHistory = (overrides: Partial<HistoryQuery>) =>
  backend.queryHistory({ ...baseHistoryQuery, ...overrides })
const biometricUnlock = () =>
  backend.unlockAppSession({ passcode: null, useBiometric: true })
const passcodeUnlock = (passcode: string | null) =>
  backend.unlockAppSession({ passcode, useBiometric: false })
const setConfiguredPreviewBiometricState = (
  biometricState: 'touch-id-available' | 'unsupported',
) => {
  backendTestHarness.mutateState((state) => {
    state.biometricState = biometricState
    state.appLockPasscode = appLockPasscode
    state.snapshot.config.appLock = {
      ...state.snapshot.config.appLock,
      enabled: true,
      passcodeEnabled: true,
      passcodeConfigured: true,
      biometricEnabled: true,
    }
    state.snapshot.appLockStatus = {
      ...state.snapshot.appLockStatus,
      enabled: true,
      locked: true,
      biometricEnabled: true,
    }
  })
}

describe('backend facade preview state', () => {
  beforeEach(() => {
    isTauri.mockReturnValue(false)
    invoke.mockReset()
    backendTestHarness.reset()
  })

  test('preserves fallback runtime notes for disabled modules and custom preview plugins', async () => {
    backendTestHarness.mutateState((state) => {
      state.intelligenceRuntime.recentJobs = [
        {
          ...state.intelligenceRuntime.recentJobs[0],
          createdAt: null as unknown as string,
          startedAt: null,
          finishedAt: null,
        },
      ]
      state.intelligenceRuntime.plugins.push({
        ...state.intelligenceRuntime.plugins[0],
        pluginId: 'custom-preview-plugin',
        enabled: false,
      })
      state.intelligenceRuntime.modules.push({
        ...state.intelligenceRuntime.modules[0],
        moduleId: 'custom-preview-module',
        enabled: false,
        status: 'disabled',
        notes: ['Disabled in Settings.'],
      })
      state.snapshot.config.deterministic.modules =
        state.snapshot.config.deterministic.modules.map((module, index) =>
          index === 0 ? { ...module, enabled: false } : module,
        )
      state.snapshot.config.enrichment.plugins =
        state.snapshot.config.enrichment.plugins.map((plugin) =>
          plugin.id === 'readable-content-refetch'
            ? { ...plugin, enabled: false }
            : plugin,
        )
    })

    await expect(backend.loadIntelligenceRuntime()).resolves.toMatchObject({
      queue: expect.objectContaining({
        lastActivityAt: null,
      }),
      plugins: expect.arrayContaining([
        expect.objectContaining({
          pluginId: 'custom-preview-plugin',
          enabled: false,
          queuedJobs: 0,
          runningJobs: 0,
          failedJobs: 0,
          lastCompletedAt: null,
          lastError: null,
        }),
      ]),
      modules: expect.arrayContaining([
        expect.objectContaining({
          moduleId: 'visit-derived-facts',
          enabled: false,
          status: 'disabled',
          notes: ['Disabled in Settings.'],
        }),
        expect.objectContaining({
          moduleId: 'custom-preview-module',
          enabled: false,
          status: 'disabled',
          notes: ['Disabled in Settings.'],
        }),
      ]),
    })

    await expect(backend.clearDerivedIntelligence()).resolves.toMatchObject({
      clearedVisitDerivedFactRows: 8,
    })
    await expect(backend.loadIntelligenceRuntime()).resolves.toMatchObject({
      modules: expect.arrayContaining([
        expect.objectContaining({
          moduleId: 'visit-derived-facts',
          enabled: false,
          status: 'disabled',
          staleReason: null,
          notes: ['Disabled in Settings.'],
        }),
      ]),
    })
    await expect(backend.loadIntelligenceRuntime()).resolves.toMatchObject({
      modules: expect.arrayContaining([
        expect.objectContaining({
          moduleId: 'visit-derived-facts',
          enabled: false,
          status: 'disabled',
          notes: ['Disabled in Settings.'],
        }),
      ]),
    })
  })

  test('surfaces preview-only updater responses through the backend facade', async () => {
    await expect(backend.checkForAppUpdate()).resolves.toMatchObject({
      availability: expect.objectContaining({
        supported: false,
        available: false,
        currentVersion: expect.any(String),
      }),
      pendingUpdate: null,
    })
    await expect(backend.downloadAndInstallAppUpdate()).resolves.toMatchObject({
      phase: 'unsupported',
      version: null,
      message: expect.stringContaining('cannot download or install'),
    })
    await expect(
      backend.downloadAndInstallAppUpdate('0.2.0'),
    ).resolves.toMatchObject({
      phase: 'unsupported',
    })
    await expect(backend.relaunchAfterUpdate()).resolves.toBe(false)
  })

  test('leaves preview runtime jobs unchanged when retry or cancel targets are missing', async () => {
    const before = await backend.loadIntelligenceRuntime()
    await expect(
      backendTestHarness.call('retry_intelligence_job'),
    ).resolves.toMatchObject({
      recentJobs: before.recentJobs,
    })
    await expect(
      backendTestHarness.call('cancel_intelligence_job'),
    ).resolves.toMatchObject({
      recentJobs: before.recentJobs,
    })
  })

  test('enforces preview archive prerequisites and keeps lock state aligned with archive mode', async () => {
    const previewConfig = makePreviewConfig()

    await expect(backend.runBackupNow()).rejects.toThrow(
      'Initialize the archive before running a backup.',
    )
    await expect(backend.initializeArchive(previewConfig)).rejects.toThrow(
      'Mock encrypted archive initialization requires a database key.',
    )
    const plaintextSnapshot = await backend.initializeArchive(
      {
        ...previewConfig,
        archiveMode: 'Plaintext',
      },
      null,
    )
    expect(plaintextSnapshot.archiveStatus).toMatchObject({
      encrypted: false,
      initialized: true,
      unlocked: true,
    })
    await backend.clearSessionDatabaseKey()
    expect((await backend.getAppSnapshot()).archiveStatus.unlocked).toBe(true)
    const encryptedSnapshot = await backend.rekeyArchive({
      newMode: 'Encrypted',
      newKey: 'vault-passphrase',
    })
    expect(encryptedSnapshot.archiveStatus).toMatchObject({
      encrypted: true,
      unlocked: true,
    })
    await backend.clearSessionDatabaseKey()
    expect((await backend.getAppSnapshot()).archiveStatus.unlocked).toBe(false)
    await backend.setSessionDatabaseKey('vault-passphrase')
    expect((await backend.getAppSnapshot()).archiveStatus.unlocked).toBe(true)
    await backend.saveConfig({
      ...encryptedSnapshot.config,
      selectedProfileIds: [],
    })
    await expect(backend.runBackupNow()).rejects.toThrow(
      'Select at least one profile before running a backup.',
    )
  })

  test('clamps mock history pagination limits into the supported range', async () => {
    await expect(
      backendTestHarness.call('query_history'),
    ).resolves.toMatchObject({
      total: 2,
      items: [
        expect.objectContaining({ id: 1 }),
        expect.objectContaining({ id: 2 }),
      ],
      nextCursor: null,
    })
    await expect(
      queryHistory({ limit: 0, cursor: null }),
    ).resolves.toMatchObject({
      items: [expect.objectContaining({ id: expect.any(Number) })],
      nextCursor: expect.any(String),
    })
    await expect(
      queryHistory({ limit: 5_000, cursor: null }),
    ).resolves.toMatchObject({
      total: 2,
      items: [
        expect.objectContaining({ id: 1 }),
        expect.objectContaining({ id: 2 }),
      ],
      nextCursor: null,
    })
  })

  test('supports explicit mock history page jumps with stable page metadata', async () => {
    const baseTime = Date.now()
    backendTestHarness.mutateState((state) => {
      state.history.items = Array.from({ length: 75 }, (_, index) => ({
        id: index + 1,
        profileId: 'chrome:Default',
        url: `https://example.com/sqlite/${index + 1}`,
        title: `SQLite note ${index + 1}`,
        domain: 'example.com',
        visitedAt: new Date(baseTime - index * 60_000).toISOString(),
        visitTime: baseTime - index * 60_000,
        durationMs: 5_000,
        transition: 805306368,
        sourceVisitId: index + 1,
        appId: null,
      }))
    })
    const middlePage = await queryHistory({ q: 'sqlite', limit: 10, page: 3 })
    expect(middlePage).toMatchObject({
      total: 75,
      page: 3,
      pageSize: 10,
      pageCount: 8,
      hasPrevious: true,
      hasNext: true,
    })
    expect(middlePage.items[0]).toMatchObject({ id: 21 })
    await expect(
      queryHistory({ q: 'sqlite', limit: 10, page: 8 }),
    ).resolves.toMatchObject({
      total: 75,
      page: 8,
      pageSize: 10,
      pageCount: 8,
      hasPrevious: true,
      hasNext: false,
      items: [
        expect.objectContaining({ id: 71 }),
        expect.objectContaining({ id: 72 }),
        expect.objectContaining({ id: 73 }),
        expect.objectContaining({ id: 74 }),
        expect.objectContaining({ id: 75 }),
      ],
    })
  })

  test('guards browser preview archive surfaces while app lock is active', async () => {
    const previewConfig = makePreviewConfig()
    await expect(
      backend.saveConfig({
        ...previewConfig,
        appLock: {
          ...previewConfig.appLock,
          enabled: true,
        },
      }),
    ).rejects.toThrow('Set an app lock passcode before turning on App Lock.')
    await expect(
      backend.saveConfig({
        ...previewConfig,
        appLock: {
          ...previewConfig.appLock,
          enabled: true,
          biometricEnabled: true,
        },
      }),
    ).rejects.toThrow('Biometric unlock is not available')
    await expect(
      backend.saveConfig({
        ...previewConfig,
        appLock: {
          ...previewConfig.appLock,
          enabled: true,
          passcodeEnabled: false,
        },
      }),
    ).rejects.toThrow('Enable a passcode before turning on App Lock')
    await expect(
      backend.setAppLockPasscode({ passcode: '123', recoveryHint: null }),
    ).rejects.toThrow('at least 4 characters')
    await backend.setAppLockPasscode({
      passcode: appLockPasscode,
      recoveryHint: 'digits only',
    })
    await expect(
      backend.setAppLockPasscode({
        passcode: appLockPasscode,
        recoveryHint: '   ',
      }),
    ).resolves.toMatchObject({
      recoveryHint: null,
    })
    await backend.setAppLockPasscode({
      passcode: appLockPasscode,
      recoveryHint: 'digits only',
    })
    const unlockedSnapshot = await backend.saveConfig({
      ...previewConfig,
      appLock: {
        ...previewConfig.appLock,
        enabled: true,
        passcodeConfigured: true,
        recoveryHint: 'digits only',
      },
    })
    expect(unlockedSnapshot.appLockStatus).toMatchObject({
      enabled: true,
      locked: false,
      passcodeConfigured: true,
      recoveryHint: 'digits only',
    })
    await expect(backend.lockAppSession('manual')).resolves.toMatchObject({
      enabled: true,
      locked: true,
      lockReason: 'manual',
    })
    await expect(backend.loadAppLockStatus()).resolves.toMatchObject({
      enabled: true,
      locked: true,
      passcodeConfigured: true,
    })
    await expect(backend.getAppSnapshot()).rejects.toThrow('currently locked')
    await expect(queryHistory({ q: 'sqlite', limit: 10 })).rejects.toThrow(
      'currently locked',
    )
    await expect(backend.openPathInFileManager('/tmp/pathkeep')).resolves.toBe(
      '/tmp/pathkeep',
    )
    await expect(biometricUnlock()).rejects.toThrow(
      'Biometric unlock is currently turned off in Settings.',
    )
    await expect(passcodeUnlock('9999')).rejects.toThrow('did not match')
    await expect(passcodeUnlock(null)).rejects.toThrow('did not match')
    await expect(passcodeUnlock(appLockPasscode)).resolves.toMatchObject({
      enabled: true,
      locked: false,
    })
    await expect(backend.getAppSnapshot()).resolves.toMatchObject({
      appLockStatus: expect.objectContaining({
        enabled: true,
        locked: false,
      }),
    })
  })

  test('treats app lock commands as no-ops when the feature is disabled', async () => {
    await expect(backend.lockAppSession('manual')).resolves.toMatchObject({
      enabled: false,
      locked: false,
      lockReason: null,
    })
    await expect(
      backend.unlockAppSession({
        passcode: '2468',
        useBiometric: false,
      }),
    ).resolves.toMatchObject({
      enabled: false,
      locked: false,
      lockReason: null,
    })
  })

  test('surfaces the truthful Touch ID unavailable fallback in preview mode', async () => {
    const previewConfig = makePreviewConfig()
    backendTestHarness.mutateState((state) => {
      state.biometricState = 'touch-id-unavailable'
    })
    await backend.setAppLockPasscode({
      passcode: appLockPasscode,
      recoveryHint: null,
    })
    await expect(
      backend.saveConfig({
        ...previewConfig,
        appLock: {
          ...previewConfig.appLock,
          enabled: true,
          passcodeConfigured: true,
          biometricEnabled: true,
        },
      }),
    ).rejects.toThrow(
      'Touch ID is unavailable on this Mac right now. Use the app lock passcode instead.',
    )
    backendTestHarness.mutateState((state) => {
      state.snapshot.config.appLock = {
        ...state.snapshot.config.appLock,
        enabled: true,
        passcodeEnabled: true,
        passcodeConfigured: true,
        biometricEnabled: true,
      }
    })
    await expect(biometricUnlock()).rejects.toThrow(
      'Touch ID is unavailable on this Mac right now. Use the app lock passcode instead.',
    )
  })

  test('reports truthful Touch ID capability notes when preview mode marks it available', async () => {
    setConfiguredPreviewBiometricState('touch-id-available')
    await expect(backend.loadAppLockStatus()).resolves.toMatchObject({
      enabled: true,
      locked: true,
      biometricAvailable: true,
      biometricEnabled: true,
      biometricState: 'touch-id-available',
      warnings: [],
      degradationNotes: expect.arrayContaining([
        'Touch ID is available on this Mac and can unlock the current PathKeep session.',
      ]),
    })
    await expect(biometricUnlock()).resolves.toMatchObject({
      enabled: true,
      locked: false,
      biometricAvailable: true,
      biometricState: 'touch-id-available',
    })
  })

  test('reports truthful unsupported biometric warnings in preview mode', async () => {
    setConfiguredPreviewBiometricState('unsupported')
    await expect(backend.loadAppLockStatus()).resolves.toMatchObject({
      biometricAvailable: false,
      biometricEnabled: true,
      biometricState: 'unsupported',
      warnings: [
        'Biometric unlock is reserved for future platform integration; this preview falls back to the app-lock passcode.',
      ],
      degradationNotes: expect.arrayContaining([
        'Biometric unlock is reserved for future platform integration; this preview falls back to the app-lock passcode.',
      ]),
    })
    await expect(biometricUnlock()).rejects.toThrow(
      'Biometric unlock is not available in the current desktop build.',
    )
  })

  test('surfaces preview warnings when app lock is flagged on without a passcode', async () => {
    backendTestHarness.mutateState((state) => {
      state.snapshot.config.appLock.enabled = true
      state.appLockPasscode = null
    })
    await expect(backend.loadAppLockStatus()).resolves.toMatchObject({
      enabled: true,
      passcodeConfigured: false,
      warnings: ['Set an app lock passcode before relying on session lock.'],
    })
  })

  test('clears preview app lock credentials and rejects unlock attempts with no remaining credential', async () => {
    await backend.setAppLockPasscode({
      passcode: appLockPasscode,
      recoveryHint: 'digits only',
    })
    await expect(backend.clearAppLockPasscode()).resolves.toMatchObject({
      enabled: false,
      passcodeConfigured: false,
      locked: false,
    })
    backendTestHarness.mutateState((state) => {
      state.appLockPasscode = null
      state.snapshot.config.appLock = {
        ...state.snapshot.config.appLock,
        enabled: true,
        passcodeEnabled: false,
        passcodeConfigured: false,
        biometricEnabled: false,
      }
      state.snapshot.appLockStatus = {
        ...state.snapshot.appLockStatus,
        enabled: true,
        locked: true,
      }
    })
    await expect(passcodeUnlock(appLockPasscode)).rejects.toThrow(
      'PathKeep cannot unlock without an enabled app lock credential.',
    )
  })
})
