/**
 * @file shell-data-actions.test.ts
 * @description Focused behavioral tests for shell mutation callbacks.
 * @module app/shell-data
 *
 * ## Responsibilities
 * - Verify shell actions clear stale messages before mutating backend state.
 * - Prove busy overlays are always cleared on success and failure.
 * - Exercise backup progress, completion notices, refreshes, and lock transitions directly.
 *
 * ## Not responsible for
 * - Rendering provider consumers or route guards.
 * - Retesting backend command transport serialization.
 *
 * ## Dependencies
 * - Mocks the backend client, backup progress subscription, and paint scheduling helper.
 *
 * ## Performance notes
 * - Runs without React rendering so action failure paths stay cheap to exercise.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'
import { createTranslator } from '../lib/i18n'
import type {
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  BackupProgressEvent,
  BackupReport,
  BackupRunOverview,
} from '../lib/types'
import { createShellDataActions } from './shell-data-actions'
import { createShellTask } from './shell-tasks'

const { backendMock, subscribeToBackupProgressMock, waitForNextPaintMock } =
  vi.hoisted(() => ({
    backendMock: {
      saveConfig: vi.fn(),
      initializeArchive: vi.fn(),
      runBackupNow: vi.fn(),
      setAppLockPasscode: vi.fn(),
      clearAppLockPasscode: vi.fn(),
      lockAppSession: vi.fn(),
      unlockAppSession: vi.fn(),
    },
    subscribeToBackupProgressMock: vi.fn(),
    waitForNextPaintMock: vi.fn(),
  }))

vi.mock('../lib/backend-client', () => ({
  backend: backendMock,
}))

vi.mock('../lib/ipc/backup-progress', () => ({
  subscribeToBackupProgress: subscribeToBackupProgressMock,
}))

vi.mock('../lib/wait-for-next-paint', () => ({
  waitForNextPaint: waitForNextPaintMock,
}))

const t = createTranslator('en')

describe('createShellDataActions', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    waitForNextPaintMock.mockResolvedValue(undefined)
    subscribeToBackupProgressMock.mockResolvedValue(vi.fn())
  })

  test('saves config by clearing stale state, refreshing snapshots, and bumping runtime scope', async () => {
    const harness = createActionHarness()
    const config = buildConfig({ preferredLanguage: 'zh-TW' })
    const snapshot = buildSnapshot({
      config,
      appLockStatus: buildAppLockStatus({ locked: true }),
    })
    backendMock.saveConfig.mockResolvedValue(snapshot)

    await expect(harness.actions.saveConfig(config)).resolves.toBe(snapshot)

    expect(harness.showBusyOverlay).toHaveBeenCalledWith({
      label: t('shell.savingArchiveChoices'),
      detail: t('shell.savingArchiveChoicesDetail'),
    })
    expect(harness.setNotice).toHaveBeenCalledWith(null)
    expect(harness.setError).toHaveBeenCalledWith(null)
    expect(waitForNextPaintMock).toHaveBeenCalledTimes(1)
    expect(backendMock.saveConfig).toHaveBeenCalledWith(config)
    expect(harness.setLanguagePreference).toHaveBeenCalledWith('zh-TW')
    expect(harness.setAppLockStatus).toHaveBeenCalledWith(
      snapshot.appLockStatus,
    )
    expect(harness.setSnapshot).toHaveBeenCalledWith(snapshot)
    expect(harness.refreshKey).toBe(1)
    expect(harness.refreshDashboardSnapshot).toHaveBeenCalledWith(snapshot)
    expect(harness.clearBusyOverlay).toHaveBeenCalledTimes(1)
  })

  test('initializes archives and reports non-Error failures with shipped fallback copy', async () => {
    const harness = createActionHarness()
    const config = buildConfig({ preferredLanguage: 'zh-CN' })
    const snapshot = buildSnapshot({ config })
    backendMock.initializeArchive.mockResolvedValueOnce(snapshot)

    await expect(
      harness.actions.initializeArchive(config, 'database-key'),
    ).resolves.toBe(snapshot)

    expect(backendMock.initializeArchive).toHaveBeenCalledWith(
      config,
      'database-key',
    )
    expect(harness.showBusyOverlay).toHaveBeenNthCalledWith(1, {
      label: t('shell.preparingArchive'),
      detail: t('shell.preparingArchiveDetail'),
    })
    expect(harness.setNotice).toHaveBeenLastCalledWith(
      t('shell.initializedNotice'),
    )
    expect(harness.refreshKey).toBe(1)
    expect(harness.refreshDashboardSnapshot).toHaveBeenCalledWith(snapshot)

    backendMock.initializeArchive.mockRejectedValueOnce('ipc failed')
    await expect(harness.actions.initializeArchive(config, null)).rejects.toBe(
      'ipc failed',
    )

    expect(harness.setError).toHaveBeenLastCalledWith(
      t('shell.initializeArchiveFailed'),
    )
    expect(harness.clearBusyOverlay).toHaveBeenCalledTimes(2)
  })

  test('streams manual backup progress, refreshes shell data, and reports warnings truthfully', async () => {
    const harness = createActionHarness()
    const unsubscribe = vi.fn()
    const progress: BackupProgressEvent = {
      phase: 'ingest-profile',
      label: 'Ingesting',
      detail: 'Chrome Default',
      step: 1,
      totalSteps: 3,
      completedProfiles: 0,
      totalProfiles: 1,
      sourceLabel: 'Chrome / Default',
      processedRecords: 12,
      totalRecords: 24,
      importedRecords: 10,
      duplicateRecords: 2,
    }
    subscribeToBackupProgressMock.mockImplementationOnce((listener) => {
      listener(progress)
      return Promise.resolve(unsubscribe)
    })
    const report = buildBackupReport({
      run: buildBackupRun({ id: 42 }),
      warnings: ['Safari History.db is not readable yet'],
    })
    backendMock.runBackupNow.mockResolvedValue(report)

    await expect(harness.actions.runBackup()).resolves.toBe(report)

    expect(harness.showBusyOverlay).toHaveBeenNthCalledWith(1, {
      label: t('shell.runningManualBackup'),
      detail: t('shell.runningManualBackupDetail'),
      progressLabel: t('shell.backupProgressPending'),
      progressValue: null,
      steps: expect.any(Array),
      activeStep: 0,
    })
    expect(subscribeToBackupProgressMock).toHaveBeenCalledWith(
      expect.any(Function),
    )
    expect(harness.showBusyOverlay).toHaveBeenCalledWith(
      expect.objectContaining({
        label: t('shell.backupWritingArchive'),
        detail: 'Chrome / Default',
        progressLabel: '12 records processed',
        progressValue: 50,
      }),
    )
    expect(harness.showBusyOverlay).toHaveBeenNthCalledWith(
      3,
      expect.objectContaining({
        label: t('shell.backupWritingArchive'),
        detail: t('shell.backupWritingArchiveDetail'),
        progressLabel: t('shell.backupRecordProgressPending'),
        progressValue: null,
        activeStep: 1,
      }),
    )
    expect(harness.showBusyOverlay).toHaveBeenNthCalledWith(
      4,
      expect.objectContaining({
        label: t('shell.refreshingArchiveViews'),
        detail: t('shell.refreshingArchiveViewsDetail'),
        progressLabel: '3 / 3',
        progressValue: 100,
        activeStep: 2,
      }),
    )
    expect(backendMock.runBackupNow).toHaveBeenCalledWith(false)
    expect(harness.refreshAppData).toHaveBeenCalledWith(false)
    expect(harness.setNotice).toHaveBeenLastCalledWith(
      t('shell.safariFullDiskAccessBackupWarning', { runId: 42 }),
    )
    expect(unsubscribe).toHaveBeenCalledTimes(1)
    expect(harness.clearBusyOverlay).toHaveBeenCalledTimes(1)
  })

  test('maps backup due-window skips and Safari access failures without losing cleanup', async () => {
    const harness = createActionHarness()
    const unsubscribe = vi.fn()
    subscribeToBackupProgressMock.mockResolvedValue(unsubscribe)
    backendMock.runBackupNow.mockResolvedValueOnce(
      buildBackupReport({
        dueSkipped: true,
        reason: 'Backup is not due for 6 hours',
        run: null,
      }),
    )

    await expect(harness.actions.runBackup()).resolves.toMatchObject({
      dueSkipped: true,
    })

    expect(harness.setNotice).toHaveBeenLastCalledWith(
      'Backup is not due for 6 hours',
    )

    backendMock.runBackupNow.mockRejectedValueOnce(
      new Error('Safari History.db is not readable yet'),
    )

    await expect(harness.actions.runBackup()).rejects.toThrow(
      t('shell.safariFullDiskAccessBackupError'),
    )
    expect(harness.setError).toHaveBeenLastCalledWith(
      t('shell.safariFullDiskAccessBackupError'),
    )
    expect(unsubscribe).toHaveBeenCalledTimes(2)
    expect(harness.clearBusyOverlay).toHaveBeenCalledTimes(2)
  })

  test('maps every manual backup completion notice branch explicitly', async () => {
    const harness = createActionHarness()
    backendMock.runBackupNow
      .mockResolvedValueOnce(
        buildBackupReport({
          dueSkipped: true,
          reason: null,
          run: null,
        }),
      )
      .mockResolvedValueOnce(
        buildBackupReport({
          run: buildBackupRun({ id: 88 }),
          warnings: [],
        }),
      )
      .mockResolvedValueOnce(
        buildBackupReport({
          run: buildBackupRun({ id: 89 }),
          warnings: ['Chrome profile skipped by filter'],
        }),
      )
      .mockResolvedValueOnce(
        buildBackupReport({
          run: buildBackupRun({ id: 90 }),
          warnings: [
            'Chrome profile skipped by filter',
            'Full Disk Access missing',
          ],
        }),
      )
      .mockResolvedValueOnce(buildBackupReport())

    await expect(harness.actions.runBackup()).resolves.toMatchObject({
      dueSkipped: true,
    })
    expect(harness.setNotice).toHaveBeenLastCalledWith(
      t('shell.manualBackupDueWindow'),
    )

    await expect(harness.actions.runBackup()).resolves.toMatchObject({
      run: expect.objectContaining({ id: 88 }),
    })
    expect(harness.setNotice).toHaveBeenLastCalledWith(
      t('shell.manualBackupFinished', { runId: 88 }),
    )

    await expect(harness.actions.runBackup()).resolves.toMatchObject({
      run: expect.objectContaining({ id: 89 }),
    })
    expect(harness.setNotice).toHaveBeenLastCalledWith(
      t('shell.manualBackupFinished', { runId: 89 }),
    )

    await expect(harness.actions.runBackup()).resolves.toMatchObject({
      run: expect.objectContaining({ id: 90 }),
    })
    expect(harness.setNotice).toHaveBeenLastCalledWith(
      t('shell.safariFullDiskAccessBackupWarning', { runId: 90 }),
    )

    await expect(harness.actions.runBackup()).resolves.toMatchObject({
      run: null,
    })
    expect(harness.setNotice).toHaveBeenLastCalledWith(t('common.complete'))
  })

  test('keeps ordinary backup Errors intact and uses fallback copy for unknown failures', async () => {
    const harness = createActionHarness()
    const unsubscribe = vi.fn()
    const ordinaryError = new Error('backup exploded')
    subscribeToBackupProgressMock.mockResolvedValue(unsubscribe)
    backendMock.runBackupNow
      .mockRejectedValueOnce(ordinaryError)
      .mockRejectedValueOnce('worker payload')

    await expect(harness.actions.runBackup()).rejects.toBe(ordinaryError)
    expect(harness.setError).toHaveBeenLastCalledWith('backup exploded')

    await expect(harness.actions.runBackup()).rejects.toBe('worker payload')
    expect(harness.setError).toHaveBeenLastCalledWith(
      t('shell.manualBackupFailed'),
    )
    expect(unsubscribe).toHaveBeenCalledTimes(2)
    expect(harness.clearBusyOverlay).toHaveBeenCalledTimes(2)
  })

  test('surfaces backup progress subscription failures before starting the worker', async () => {
    const harness = createActionHarness()
    subscribeToBackupProgressMock.mockRejectedValueOnce(
      new Error('subscribe failed'),
    )

    await expect(harness.actions.runBackup()).rejects.toThrow(
      'subscribe failed',
    )

    expect(backendMock.runBackupNow).not.toHaveBeenCalled()
    expect(harness.setError).toHaveBeenLastCalledWith('subscribe failed')
    expect(harness.clearBusyOverlay).toHaveBeenCalledTimes(1)
  })

  test('mirrors backup lifecycle into shell archive-task hooks', async () => {
    const task = createShellTask({
      id: 'backup-task',
      kind: 'backup',
      title: 'Backup',
      detail: 'Queued backup',
      timestamp: '2026-04-27T10:00:00.000Z',
    })
    const archiveTasks = {
      beginBackupTask: vi.fn(() => ({ task })),
      updateBackupTask: vi.fn(),
      finishBackupTask: vi.fn(),
      failBackupTask: vi.fn(),
    }
    const harness = createActionHarness({ archiveTasks })
    const progress: BackupProgressEvent = {
      phase: 'stage-profile',
      label: 'Stage profile',
      detail: 'Copying profile',
      step: 1,
      totalSteps: 3,
      completedProfiles: 0,
      totalProfiles: 1,
      profileId: 'chrome:Default',
      progressCurrent: 0,
      progressTotal: 1,
      progressPercent: 0,
      logLines: [],
    }
    subscribeToBackupProgressMock.mockImplementationOnce((listener) => {
      listener(progress)
      return Promise.resolve(vi.fn())
    })
    const report = buildBackupReport({ run: buildBackupRun({ id: 77 }) })
    backendMock.runBackupNow.mockResolvedValueOnce(report)

    await expect(harness.actions.runBackup()).resolves.toBe(report)

    expect(archiveTasks.beginBackupTask).toHaveBeenCalledTimes(1)
    expect(archiveTasks.updateBackupTask).toHaveBeenCalledWith(
      'backup-task',
      progress,
    )
    expect(archiveTasks.finishBackupTask).toHaveBeenCalledWith(
      'backup-task',
      report,
    )
    expect(archiveTasks.failBackupTask).not.toHaveBeenCalled()

    backendMock.runBackupNow.mockRejectedValueOnce(new Error('backup failed'))
    subscribeToBackupProgressMock.mockResolvedValueOnce(vi.fn())
    await expect(harness.actions.runBackup()).rejects.toThrow('backup failed')
    expect(archiveTasks.failBackupTask).toHaveBeenCalledWith(
      'backup-task',
      'backup failed',
    )
  })

  test('does not start a second backup when an archive-write task is already active', async () => {
    const activeTask = createShellTask({
      id: 'import-task',
      kind: 'import',
      title: 'Import Chrome',
      detail: 'Importing',
      timestamp: '2026-04-27T10:00:00.000Z',
    })
    const harness = createActionHarness({
      archiveTasks: {
        beginBackupTask: vi.fn(() => ({ blockedBy: activeTask })),
        updateBackupTask: vi.fn(),
        finishBackupTask: vi.fn(),
        failBackupTask: vi.fn(),
      },
    })

    await expect(harness.actions.runBackup()).rejects.toThrow(
      t('jobs.archiveTaskAlreadyRunningBody', {
        task: activeTask.title,
      }),
    )

    expect(subscribeToBackupProgressMock).not.toHaveBeenCalled()
    expect(backendMock.runBackupNow).not.toHaveBeenCalled()
  })

  test('updates app-lock status actions and keeps refresh side effects scoped', async () => {
    const harness = createActionHarness()
    const lockedStatus = buildAppLockStatus({ locked: true })
    const unlockedStatus = buildAppLockStatus({ locked: false })
    backendMock.setAppLockPasscode.mockResolvedValueOnce(lockedStatus)
    backendMock.clearAppLockPasscode.mockResolvedValueOnce(unlockedStatus)
    backendMock.lockAppSession.mockResolvedValueOnce(lockedStatus)
    backendMock.unlockAppSession.mockResolvedValueOnce(unlockedStatus)

    await expect(
      harness.actions.setAppLockPasscode({
        passcode: '123456',
        recoveryHint: 'desk',
      }),
    ).resolves.toBe(lockedStatus)
    await expect(harness.actions.clearAppLockPasscode()).resolves.toBe(
      unlockedStatus,
    )
    await expect(harness.actions.lockAppSession(undefined)).resolves.toBe(
      lockedStatus,
    )
    await expect(
      harness.actions.unlockAppSession({ passcode: '123456' }),
    ).resolves.toBe(unlockedStatus)

    expect(backendMock.setAppLockPasscode).toHaveBeenCalledWith({
      passcode: '123456',
      recoveryHint: 'desk',
    })
    expect(backendMock.lockAppSession).toHaveBeenCalledWith(null)
    expect(backendMock.unlockAppSession).toHaveBeenCalledWith({
      passcode: '123456',
    })
    expect(harness.showBusyOverlay).toHaveBeenNthCalledWith(1, {
      label: t('shell.settingAppLockPasscode'),
      detail: t('shell.settingAppLockPasscodeDetail'),
    })
    expect(harness.showBusyOverlay).toHaveBeenNthCalledWith(2, {
      label: t('shell.clearingAppLockPasscode'),
      detail: t('shell.clearingAppLockPasscodeDetail'),
    })
    expect(harness.showBusyOverlay).toHaveBeenNthCalledWith(3, {
      label: t('shell.lockingApp'),
      detail: t('shell.lockingAppDetail'),
    })
    expect(harness.showBusyOverlay).toHaveBeenNthCalledWith(4, {
      label: t('shell.unlockingApp'),
      detail: t('shell.unlockingAppDetail'),
    })
    expect(harness.setAppLockStatus).toHaveBeenCalledTimes(4)
    expect(harness.refreshAppData).toHaveBeenCalledTimes(3)
    expect(harness.refreshAppData).toHaveBeenNthCalledWith(1, false)
    expect(harness.refreshAppData).toHaveBeenNthCalledWith(2, false)
    expect(harness.refreshAppData).toHaveBeenNthCalledWith(3, false)
    expect(harness.clearLoadedState).toHaveBeenCalledTimes(1)
    expect(harness.refreshKey).toBe(1)
    expect(harness.clearBusyOverlay).toHaveBeenCalledTimes(4)
  })

  test('uses shipped fallback copy for non-Error config and app-lock failures', async () => {
    const harness = createActionHarness()
    const config = buildConfig()
    backendMock.saveConfig.mockRejectedValueOnce('save payload')
    backendMock.setAppLockPasscode.mockRejectedValueOnce('set payload')
    backendMock.lockAppSession.mockRejectedValueOnce('lock payload')

    await expect(harness.actions.saveConfig(config)).rejects.toBe(
      'save payload',
    )
    expect(harness.setError).toHaveBeenLastCalledWith(
      t('shell.savingSettingsFailed'),
    )

    await expect(
      harness.actions.setAppLockPasscode({
        passcode: '12',
        recoveryHint: null,
      }),
    ).rejects.toBe('set payload')
    expect(harness.setError).toHaveBeenLastCalledWith(
      t('shell.setAppLockPasscodeFailed'),
    )

    await expect(harness.actions.lockAppSession('idle')).rejects.toBe(
      'lock payload',
    )
    expect(harness.setError).toHaveBeenLastCalledWith(t('shell.lockAppFailed'))
    expect(harness.clearBusyOverlay).toHaveBeenCalledTimes(3)
  })

  test('surfaces app-lock failures and still clears the busy overlay', async () => {
    const harness = createActionHarness()
    const config = buildConfig()
    backendMock.saveConfig.mockRejectedValueOnce(new Error('save exploded'))
    backendMock.setAppLockPasscode.mockRejectedValueOnce(new Error('weak pin'))
    backendMock.clearAppLockPasscode.mockRejectedValueOnce('clear payload')
    backendMock.lockAppSession.mockRejectedValueOnce(new Error('lock refused'))
    backendMock.unlockAppSession.mockRejectedValueOnce('bad passcode payload')

    await expect(harness.actions.saveConfig(config)).rejects.toThrow(
      'save exploded',
    )
    await expect(
      harness.actions.setAppLockPasscode({
        passcode: '12',
        recoveryHint: null,
      }),
    ).rejects.toThrow('weak pin')
    await expect(harness.actions.clearAppLockPasscode()).rejects.toBe(
      'clear payload',
    )
    await expect(harness.actions.lockAppSession('idle')).rejects.toThrow(
      'lock refused',
    )
    await expect(
      harness.actions.unlockAppSession({ passcode: 'bad' }),
    ).rejects.toBe('bad passcode payload')

    expect(harness.setError).toHaveBeenNthCalledWith(2, 'save exploded')
    expect(harness.setError).toHaveBeenNthCalledWith(4, 'weak pin')
    expect(harness.setError).toHaveBeenNthCalledWith(
      6,
      t('shell.clearAppLockPasscodeFailed'),
    )
    expect(harness.setError).toHaveBeenNthCalledWith(8, 'lock refused')
    expect(harness.setError).toHaveBeenNthCalledWith(
      10,
      t('shell.unlockAppFailed'),
    )
    expect(harness.clearBusyOverlay).toHaveBeenCalledTimes(5)
  })
})

function createActionHarness(
  options: {
    archiveTasks?: Parameters<typeof createShellDataActions>[0]['archiveTasks']
  } = {},
) {
  let refreshKey = 0
  const setRefreshKey = vi.fn((value) => {
    refreshKey = typeof value === 'function' ? value(refreshKey) : value
  })
  const harness = {
    setLanguagePreference: vi.fn(),
    refreshDashboardSnapshot: vi.fn(),
    refreshAppData: vi.fn(),
    clearLoadedState: vi.fn(),
    showBusyOverlay: vi.fn(),
    clearBusyOverlay: vi.fn(),
    setNotice: vi.fn(),
    setError: vi.fn(),
    setSnapshot: vi.fn(),
    setAppLockStatus: vi.fn(),
    setRefreshKey,
  }

  return {
    ...harness,
    get refreshKey() {
      return refreshKey
    },
    actions: createShellDataActions({
      t,
      setLanguagePreference: harness.setLanguagePreference,
      refreshDashboardSnapshot: harness.refreshDashboardSnapshot,
      refreshAppData: harness.refreshAppData,
      clearLoadedState: harness.clearLoadedState,
      showBusyOverlay: harness.showBusyOverlay,
      clearBusyOverlay: harness.clearBusyOverlay,
      setNotice: harness.setNotice,
      setError: harness.setError,
      setSnapshot: harness.setSnapshot,
      setAppLockStatus: harness.setAppLockStatus,
      setRefreshKey,
      archiveTasks: options.archiveTasks,
    }),
  }
}

function buildConfig(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
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
    explorerBackgroundPrefetchPages: 2,
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
    enrichment: {
      plugins: [],
    },
    deterministic: {
      modules: [],
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
      enrichmentPlugins: [],
      llmProviderId: null,
      embeddingProviderId: null,
      retrievalTopK: 8,
      assistantSystemPrompt: 'Evidence only.',
      llmProviders: [],
      embeddingProviders: [],
    },
    ...overrides,
  }
}

function buildSnapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  const config = overrides.config ?? buildConfig()
  const appLockStatus = overrides.appLockStatus ?? buildAppLockStatus()

  return {
    config,
    appLockStatus,
    ...overrides,
  } as AppSnapshot
}

function buildAppLockStatus(
  overrides: Partial<AppLockStatus> = {},
): AppLockStatus {
  return {
    enabled: true,
    locked: false,
    idleTimeoutMinutes: 5,
    biometricAvailable: false,
    biometricEnabled: false,
    biometricState: 'touch-id-unavailable',
    passcodeEnabled: true,
    passcodeConfigured: true,
    configPath: '/tmp/pathkeep/app-lock.json',
    lockReason: null,
    lockedAt: null,
    lastUnlockedAt: null,
    recoveryHint: null,
    warnings: [],
    degradationNotes: [],
    ...overrides,
  }
}

function buildBackupReport(
  overrides: Partial<BackupReport> = {},
): BackupReport {
  return {
    dueSkipped: false,
    run: null,
    profiles: [],
    warnings: [],
    ...overrides,
  } as BackupReport
}

function buildBackupRun(
  overrides: Partial<BackupRunOverview> = {},
): BackupRunOverview {
  return {
    id: 1,
    startedAt: '2026-04-27T00:00:00Z',
    finishedAt: '2026-04-27T00:00:01Z',
    status: 'success',
    profilesProcessed: 1,
    newVisits: 10,
    newUrls: 5,
    newDownloads: 0,
    ...overrides,
  }
}
