/**
 * @file use-settings-support-state.test.tsx
 * @description Hook-level coverage for Settings support, retention, App Lock, and profile-selection state.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify Settings support state loads honest schedule/security snapshots and fallback errors.
 * - Protect retention prune, language, background prefetch, App Lock, and profile toggle handlers.
 * - Keep Settings workflow tests close to the state owner instead of relying only on route smoke coverage.
 *
 * ## Not responsible for
 * - Re-testing individual section rendering.
 * - Re-testing backend preview command implementations.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider and backend-client facade spies.
 *
 * ## Performance notes
 * - Hook-level tests exercise the workflow branches without mounting the full Settings route shell.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import { I18nProvider } from '../../lib/i18n'
import type {
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  RetentionPreview,
  RetentionPruneResult,
  ScheduleStatus,
  SecurityStatus,
} from '../../lib/types'
import { useSettingsSupportState } from './use-settings-support-state'

function Wrapper({ children }: { children: ReactNode }) {
  return <I18nProvider>{children}</I18nProvider>
}

describe('useSettingsSupportState', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    backendTestHarness.reset()
  })

  test('loads support and retention state and persists general/profile changes', async () => {
    const snapshot = await createSnapshot()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshot,
        config,
      }),
    )
    const setLanguagePreference = vi.fn()
    const openPath = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/tmp/pathkeep')
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(scheduleFixture())
    vi.spyOn(backend, 'securityStatus').mockResolvedValue(securityFixture())
    vi.spyOn(backend, 'previewRetentionPrune').mockResolvedValue(
      retentionPreviewFixture(),
    )

    const { result } = renderHook(
      () =>
        useSettingsSupportState({
          appLockStatus: appLockStatusFixture(),
          clearAppLockPasscode: vi
            .fn()
            .mockResolvedValue(appLockStatusFixture()),
          lockAppSession: vi.fn().mockResolvedValue(appLockStatusFixture()),
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig,
          setAppLockPasscode: vi.fn().mockResolvedValue(appLockStatusFixture()),
          setLanguagePreference,
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() => expect(result.current.supportStateLoaded).toBe(true))
    await waitFor(() =>
      expect(result.current.retention.preview?.buckets).toHaveLength(2),
    )
    expect(result.current.retention.needsUnlock).toBe(true)
    expect(result.current.retention.selectedBytes).toBe(96)

    await act(async () => {
      await result.current.general.onLanguageChange('zh-TW')
    })
    expect(setLanguagePreference).toHaveBeenCalledWith('zh-TW')
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({ preferredLanguage: 'zh-TW' }),
    )

    await act(async () => {
      await result.current.general.onLanguageChange('pirate')
    })
    expect(saveConfig).toHaveBeenCalledTimes(1)

    await act(async () => {
      await result.current.general.onExplorerBackgroundPrefetchPagesChange(12)
    })
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({ explorerBackgroundPrefetchPages: 10 }),
    )

    await act(async () => {
      await result.current.general.onExplorerBackgroundPrefetchPagesChange(
        snapshot.config.explorerBackgroundPrefetchPages,
      )
    })
    expect(saveConfig).toHaveBeenCalledTimes(2)

    act(() => {
      result.current.general.onOpenPath('/tmp/pathkeep')
    })
    expect(openPath).toHaveBeenCalledWith('/tmp/pathkeep')

    await act(async () => {
      await result.current.general.onCopyPath(
        'settings:audit-path',
        '/tmp/pathkeep/audit.json',
      )
    })
    expect(result.current.general.supportCopyFeedback).toEqual({
      key: 'settings:audit-path',
      tone: expect.any(String),
    })

    await act(async () => {
      await result.current.profiles.onToggleProfile('safari:Work')
    })
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        selectedProfileIds: ['chrome:Default', 'safari:Work'],
      }),
    )

    await act(async () => {
      await result.current.profiles.onToggleProfile('chrome:Default')
    })
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        selectedProfileIds: [],
      }),
    )
  })

  test('handles retention prune empty-selection, failure, refresh, and success paths', async () => {
    const snapshot = await createSnapshot()
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(scheduleFixture())
    vi.spyOn(backend, 'securityStatus').mockResolvedValue(securityFixture())
    const previewPrune = vi
      .spyOn(backend, 'previewRetentionPrune')
      .mockResolvedValue(retentionPreviewFixture())
    const runPrune = vi
      .spyOn(backend, 'runRetentionPrune')
      .mockResolvedValue(retentionPruneResultFixture())

    const { result } = renderHook(
      () =>
        useSettingsSupportState({
          appLockStatus: appLockStatusFixture(),
          clearAppLockPasscode: vi
            .fn()
            .mockResolvedValue(appLockStatusFixture()),
          lockAppSession: vi.fn().mockResolvedValue(appLockStatusFixture()),
          refreshAppData,
          refreshKey: 1,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          setAppLockPasscode: vi.fn().mockResolvedValue(appLockStatusFixture()),
          setLanguagePreference: vi.fn(),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() =>
      expect(result.current.retention.preview?.buckets).toHaveLength(2),
    )

    act(() => {
      result.current.retention.onBucketSelectionChange('snapshots', false)
      result.current.retention.onBucketSelectionChange('exports', false)
    })
    await act(async () => {
      await result.current.retention.onPrune()
    })
    expect(runPrune).not.toHaveBeenCalled()
    expect(result.current.retention.error).toBe(
      'Select at least one retention bucket before pruning.',
    )

    act(() => {
      result.current.retention.onBucketSelectionChange('snapshots', true)
    })
    runPrune.mockRejectedValueOnce(new Error('disk busy'))
    await act(async () => {
      await result.current.retention.onPrune()
    })
    expect(result.current.retention.error).toBe('disk busy')

    previewPrune.mockResolvedValueOnce({
      buckets: [
        {
          id: 'snapshots',
          bytes: 12,
          itemCount: 1,
          paths: ['/snapshots/new'],
        },
      ],
      warnings: [],
    })
    await act(async () => {
      await result.current.retention.onRefresh()
    })
    expect(result.current.retention.selectedBytes).toBe(12)

    await act(async () => {
      await result.current.retention.onPrune()
    })
    expect(runPrune).toHaveBeenLastCalledWith({ bucketIds: ['snapshots'] })
    expect(refreshAppData).toHaveBeenCalledTimes(1)
    expect(result.current.retention.result?.deletedBytes).toBe(12)
    expect(result.current.retention.action).toBeNull()
  })

  test('uses retention fallback copy for non-Error prune failures', async () => {
    const snapshot = await createSnapshot()
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(scheduleFixture())
    vi.spyOn(backend, 'securityStatus').mockResolvedValue(securityFixture())
    vi.spyOn(backend, 'previewRetentionPrune').mockResolvedValue(
      retentionPreviewFixture(),
    )
    vi.spyOn(backend, 'runRetentionPrune').mockRejectedValueOnce(
      'prune fallback',
    )

    const { result } = renderHook(
      () =>
        useSettingsSupportState({
          appLockStatus: appLockStatusFixture(),
          clearAppLockPasscode: vi
            .fn()
            .mockResolvedValue(appLockStatusFixture()),
          lockAppSession: vi.fn().mockResolvedValue(appLockStatusFixture()),
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          setAppLockPasscode: vi.fn().mockResolvedValue(appLockStatusFixture()),
          setLanguagePreference: vi.fn(),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() =>
      expect(result.current.retention.preview?.buckets).toHaveLength(2),
    )

    await act(async () => {
      await result.current.retention.onPrune()
    })

    expect(result.current.retention.error).toBe('prune fallback')
    expect(result.current.retention.action).toBeNull()
  })

  test('handles support failures, disabled retention, and App Lock handlers', async () => {
    const snapshot = await createSnapshot()
    const nextStatus = appLockStatusFixture({ passcodeConfigured: true })
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshot,
        config,
      }),
    )
    const setPasscode = vi.fn().mockResolvedValue(nextStatus)
    const clearPasscode = vi.fn().mockResolvedValue(nextStatus)
    const lockAppSession = vi.fn().mockResolvedValue(nextStatus)
    vi.spyOn(backend, 'scheduleStatus').mockRejectedValue(
      new Error('scheduler unavailable'),
    )
    vi.spyOn(backend, 'securityStatus').mockResolvedValue(securityFixture())
    const previewPrune = vi.spyOn(backend, 'previewRetentionPrune')

    const { result } = renderHook(
      () =>
        useSettingsSupportState({
          appLockStatus: nextStatus,
          clearAppLockPasscode: clearPasscode,
          enableRetentionPreview: false,
          lockAppSession,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig,
          setAppLockPasscode: setPasscode,
          setLanguagePreference: vi.fn(),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() => expect(result.current.supportStateLoaded).toBe(true))
    expect(result.current.supportState.scheduleStatus).toBeNull()
    expect(result.current.supportState.securityStatus).toBeNull()
    expect(result.current.retention.preview).toBeNull()
    expect(previewPrune).not.toHaveBeenCalled()
    expect(result.current.appLock.canEnable).toBe(true)
    expect(result.current.appLock.usesTouchId).toBe(true)

    await act(async () => {
      await result.current.retention.onRefresh()
      await result.current.retention.onPrune()
    })
    expect(result.current.retention.preview).toBeNull()

    act(() => {
      result.current.appLock.onEnabledChange(true)
      result.current.appLock.onIdleTimeoutChange(15)
      result.current.appLock.onBiometricChange(true)
      result.current.appLock.onRecoveryHintChange('lab machine')
    })
    await act(async () => {
      await result.current.appLock.onSaveConfig()
    })
    expect(saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        appLock: expect.objectContaining({
          enabled: true,
          idleTimeoutMinutes: 15,
          biometricEnabled: true,
          recoveryHint: 'lab machine',
        }),
      }),
    )

    act(() => {
      result.current.appLock.onPasscodeChange('123456')
    })
    await act(async () => {
      await result.current.appLock.onSetPasscode()
    })
    expect(setPasscode).toHaveBeenCalledWith({
      passcode: '123456',
      recoveryHint: 'lab machine',
    })
    expect(result.current.appLock.passcode).toBe('')

    await act(async () => {
      await result.current.appLock.onClearPasscode()
    })
    expect(clearPasscode).toHaveBeenCalledTimes(1)
    expect(result.current.appLock.recoveryHint).toBe('')

    await act(async () => {
      await result.current.appLock.onLockNow()
    })
    expect(lockAppSession).toHaveBeenCalledWith('manual')
    expect(result.current.appLock.action).toBeNull()
  })

  test('surfaces retention preview failures and no-ops without a snapshot', async () => {
    const snapshot = await createSnapshot()
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(scheduleFixture())
    vi.spyOn(backend, 'securityStatus').mockResolvedValue(securityFixture())
    const previewPrune = vi
      .spyOn(backend, 'previewRetentionPrune')
      .mockRejectedValueOnce(new Error('preview offline'))
      .mockRejectedValueOnce('not an error')
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshot,
        config,
      }),
    )

    const { result, rerender } = renderHook(
      ({ nextSnapshot }: { nextSnapshot: AppSnapshot | null }) =>
        useSettingsSupportState({
          appLockStatus: null,
          clearAppLockPasscode: vi
            .fn()
            .mockResolvedValue(appLockStatusFixture()),
          lockAppSession: vi.fn().mockResolvedValue(appLockStatusFixture()),
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig,
          setAppLockPasscode: vi.fn().mockResolvedValue(appLockStatusFixture()),
          setLanguagePreference: vi.fn(),
          snapshot: nextSnapshot,
        }),
      {
        initialProps: {
          nextSnapshot: snapshot as AppSnapshot | null,
        },
        wrapper: Wrapper,
      },
    )

    await waitFor(() =>
      expect(result.current.retention.error).toBe('preview offline'),
    )

    await act(async () => {
      await result.current.retention.onRefresh()
    })
    expect(result.current.retention.error).toBe('not an error')

    previewPrune.mockRejectedValueOnce(new Error('refresh offline'))
    await act(async () => {
      await result.current.retention.onRefresh()
    })
    expect(result.current.retention.error).toBe('refresh offline')

    rerender({
      nextSnapshot: null,
    })
    await act(async () => {
      await result.current.general.onLanguageChange('en')
      await result.current.general.onExplorerBackgroundPrefetchPagesChange(4)
      await result.current.profiles.onToggleProfile('chrome:Default')
      await result.current.appLock.onSaveConfig()
    })

    expect(saveConfig).not.toHaveBeenCalled()
    expect(previewPrune).toHaveBeenCalledTimes(4)
  })

  test('uses fallback copy for non-Error initial retention preview failures', async () => {
    const snapshot = await createSnapshot()
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(scheduleFixture())
    vi.spyOn(backend, 'securityStatus').mockResolvedValue(securityFixture())
    vi.spyOn(backend, 'previewRetentionPrune').mockRejectedValue(
      'retention fallback',
    )

    const { result } = renderHook(
      () =>
        useSettingsSupportState({
          appLockStatus: null,
          clearAppLockPasscode: vi
            .fn()
            .mockResolvedValue(appLockStatusFixture()),
          lockAppSession: vi.fn().mockResolvedValue(appLockStatusFixture()),
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          setAppLockPasscode: vi.fn().mockResolvedValue(appLockStatusFixture()),
          setLanguagePreference: vi.fn(),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() =>
      expect(result.current.retention.error).toBe('retention fallback'),
    )
  })

  test('ignores late support and retention results after unmount', async () => {
    const snapshot = await createSnapshot()
    const schedule = deferred<ScheduleStatus>()
    const security = deferred<SecurityStatus>()
    const retention = deferred<RetentionPreview>()
    vi.spyOn(backend, 'scheduleStatus').mockReturnValue(schedule.promise)
    vi.spyOn(backend, 'securityStatus').mockReturnValue(security.promise)
    vi.spyOn(backend, 'previewRetentionPrune').mockReturnValue(
      retention.promise,
    )

    const { unmount } = renderHook(
      () =>
        useSettingsSupportState({
          appLockStatus: null,
          clearAppLockPasscode: vi
            .fn()
            .mockResolvedValue(appLockStatusFixture()),
          lockAppSession: vi.fn().mockResolvedValue(appLockStatusFixture()),
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          setAppLockPasscode: vi.fn().mockResolvedValue(appLockStatusFixture()),
          setLanguagePreference: vi.fn(),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    unmount()

    await act(async () => {
      schedule.reject(new Error('late schedule failure'))
      security.resolve(securityFixture())
      retention.reject(new Error('late retention failure'))
      await Promise.resolve()
      await Promise.resolve()
    })
  })

  test('keeps App Lock handlers safe before a draft exists and without platform status', async () => {
    const snapshot = await createSnapshot()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshot,
        config: {
          ...config,
          appLock: {
            ...config.appLock,
            recoveryHint: null,
          },
        },
      }),
    )
    const setPasscode = vi.fn().mockResolvedValue(appLockStatusFixture())
    vi.spyOn(backend, 'scheduleStatus').mockResolvedValue(scheduleFixture())
    vi.spyOn(backend, 'securityStatus').mockResolvedValue(securityFixture())
    vi.spyOn(backend, 'previewRetentionPrune').mockResolvedValue(
      retentionPreviewFixture(),
    )

    const { result, rerender } = renderHook(
      ({ nextSnapshot }: { nextSnapshot: AppSnapshot | null }) =>
        useSettingsSupportState({
          appLockStatus: null,
          clearAppLockPasscode: vi
            .fn()
            .mockResolvedValue(appLockStatusFixture()),
          lockAppSession: vi.fn().mockResolvedValue(appLockStatusFixture()),
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig,
          setAppLockPasscode: setPasscode,
          setLanguagePreference: vi.fn(),
          snapshot: nextSnapshot,
        }),
      {
        initialProps: { nextSnapshot: null as AppSnapshot | null },
        wrapper: Wrapper,
      },
    )

    act(() => {
      result.current.appLock.onEnabledChange(true)
      result.current.appLock.onIdleTimeoutChange(30)
      result.current.appLock.onBiometricChange(true)
      result.current.appLock.onRecoveryHintChange('ignored')
    })
    expect(result.current.appLock.currentSettings).toBeNull()

    rerender({ nextSnapshot: snapshot })
    await waitFor(() =>
      expect(result.current.appLock.currentSettings).not.toBeNull(),
    )

    act(() => {
      result.current.appLock.onEnabledChange(true)
      result.current.appLock.onBiometricChange(true)
      result.current.appLock.onRecoveryHintChange('   ')
      result.current.appLock.onPasscodeChange('246810')
    })
    await act(async () => {
      await result.current.appLock.onSaveConfig()
    })
    expect(saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        appLock: expect.objectContaining({
          biometricEnabled: false,
          passcodeConfigured: false,
          recoveryHint: null,
        }),
      }),
    )
    expect(result.current.appLock.recoveryHint).toBe('')

    await act(async () => {
      await result.current.appLock.onSetPasscode()
    })
    expect(setPasscode).toHaveBeenCalledWith({
      passcode: '246810',
      recoveryHint: null,
    })
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, reject, resolve }
}

async function createSnapshot(): Promise<AppSnapshot> {
  const snapshot = await backend.getAppSnapshot()
  return {
    ...snapshot,
    config: {
      ...snapshot.config,
      initialized: true,
      selectedProfileIds: ['chrome:Default'],
      explorerBackgroundPrefetchPages: 2,
      appLock: {
        ...snapshot.config.appLock,
        enabled: false,
        idleTimeoutMinutes: 5,
        biometricEnabled: false,
        passcodeEnabled: true,
        passcodeConfigured: false,
        recoveryHint: null,
      },
    },
    browserProfiles: [
      {
        browserName: 'Google Chrome',
        browserFamily: 'chromium',
        browserVersion: '124.0.0',
        profileId: 'chrome:Default',
        profileName: 'Default',
        userName: null,
        profilePath: '/Users/test/Chrome/Default',
        historyPath: '/Users/test/Chrome/Default/History',
        faviconsPath: null,
        historyExists: true,
        historyReadable: true,
        accessIssue: null,
        historyFileName: 'History',
        historyBytes: 1024,
        faviconsBytes: 0,
        supportingBytes: 1024,
        retentionBoundary: {
          kind: 'browser-managed',
          localDays: 90,
        },
      },
      {
        browserName: 'Safari',
        browserFamily: 'safari',
        browserVersion: null,
        profileId: 'safari:Work',
        profileName: 'Work',
        userName: null,
        profilePath: '/Users/test/Safari',
        historyPath: '/Users/test/Safari/History.db',
        faviconsPath: null,
        historyExists: true,
        historyReadable: true,
        accessIssue: null,
        historyFileName: 'History.db',
        historyBytes: 2048,
        faviconsBytes: 0,
        supportingBytes: 2048,
        retentionBoundary: {
          kind: 'macos-safari',
          localDays: 365,
        },
      },
    ],
  }
}

function scheduleFixture(): ScheduleStatus {
  return {
    platform: 'macos',
    label: 'com.yi-ting.pathkeep.backup',
    dueAfterHours: 72,
    checkIntervalHours: 6,
    applySupported: true,
    installState: 'installed',
    detectedFiles: ['/Users/test/Library/LaunchAgents/pathkeep.plist'],
    manualSteps: [],
    auditPath: '/Users/test/pathkeep/schedule-audit.json',
    lastSuccessfulBackupAt: null,
    warnings: [],
  }
}

function securityFixture(): SecurityStatus {
  return {
    initialized: true,
    mode: 'Encrypted',
    encrypted: true,
    unlocked: false,
    databasePath: '/Users/test/pathkeep/history-vault.sqlite',
    strongholdPath: '/Users/test/pathkeep/stronghold',
    rememberDatabaseKeyInKeyring: false,
    lastSuccessfulBackupAt: null,
    lastRekeyAt: null,
    lastRekeyRunId: null,
    lastRekeySnapshotPath: null,
    keyringStatus: {
      available: true,
      backend: 'file-backed-test',
      storedSecret: false,
    },
    warnings: [],
  }
}

function appLockStatusFixture(
  overrides: Partial<AppLockStatus> = {},
): AppLockStatus {
  return {
    enabled: false,
    locked: false,
    idleTimeoutMinutes: 5,
    biometricAvailable: true,
    biometricEnabled: false,
    biometricState: 'touch-id-available',
    passcodeEnabled: true,
    passcodeConfigured: false,
    configPath: '/Users/test/pathkeep/config.json',
    lockReason: null,
    lockedAt: null,
    lastUnlockedAt: null,
    recoveryHint: null,
    warnings: [],
    degradationNotes: [],
    ...overrides,
  }
}

function retentionPreviewFixture(): RetentionPreview {
  return {
    buckets: [
      {
        id: 'snapshots',
        bytes: 64,
        itemCount: 2,
        paths: ['/snapshots/a', '/snapshots/b'],
      },
      {
        id: 'exports',
        bytes: 32,
        itemCount: 1,
        paths: ['/exports/a.csv'],
      },
    ],
    warnings: [],
  }
}

function retentionPruneResultFixture(): RetentionPruneResult {
  return {
    runId: 12,
    deletedBytes: 12,
    deletedFiles: 1,
    buckets: [
      {
        id: 'snapshots',
        bytes: 12,
        itemCount: 1,
        paths: ['/snapshots/new'],
      },
    ],
    warnings: [],
  }
}
