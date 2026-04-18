/**
 * This module belongs to the application shell layer for Shell Data.test.tsx.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `createI18nValue`
 * - `seedSnapshot`
 * - `ShellProbe`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import { useEffect } from 'react'
import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../lib/backend-client'
import { backendTestHarness } from '../lib/backend'
import { subscribeToBackupProgress } from '../lib/ipc/backup-progress'
import { I18nContext, type I18nContextValue } from '../lib/i18n/context'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from '../lib/i18n'
import type {
  AppConfig,
  AppSnapshot,
  BackupProgressEvent,
  BackupReport,
} from '../lib/types'
import { ShellDataProvider } from './shell-data'
import { useShellData } from './shell-data-context'

vi.mock('../lib/ipc/backup-progress', () => ({
  subscribeToBackupProgress: vi.fn(() => Promise.resolve(vi.fn())),
}))

const baseConfig: AppConfig = {
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
  appLock: {
    enabled: false,
    idleTimeoutMinutes: 5,
    biometricEnabled: false,
    passcodeEnabled: true,
    passcodeConfigured: false,
    recoveryHint: null,
  },
  analytics: {
    enabled: false,
    consentGrantedAt: null,
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
    plugins: [
      {
        id: 'readable-content-refetch',
        enabled: true,
        version: 'diagnostic',
      },
    ],
  },
  deterministic: {
    modules: [
      { id: 'visit-derived-facts', enabled: true, version: 'ci-v1' },
      { id: 'daily-rollups', enabled: true, version: 'ci-v1' },
      { id: 'sessions', enabled: true, version: 'ci-v1' },
      { id: 'search-trails', enabled: true, version: 'ci-v1' },
      { id: 'refind-pages', enabled: true, version: 'ci-v1' },
      { id: 'activity-mix', enabled: true, version: 'ci-v1' },
      { id: 'search-effectiveness', enabled: true, version: 'ci-v1' },
      { id: 'domain-deep-dive', enabled: true, version: 'ci-v1' },
    ],
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
    enrichmentPlugins: [
      { pluginId: 'title-normalization', enabled: true },
      { pluginId: 'readable-content-refetch', enabled: true },
    ],
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt: 'Evidence only.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

/**
 * Creates i18n value.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
function createI18nValue(
  language: ResolvedLanguage,
  setLanguagePreference = vi.fn(),
): I18nContextValue {
  return {
    language,
    preference: language,
    setLanguagePreference,
    t: createTranslator(language),
    ns: (namespace) => createNamespaceTranslator(language, namespace),
  }
}

/**
 * Explains how seed snapshot works.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
async function seedSnapshot() {
  await backend.initializeArchive(baseConfig, 'vault-passphrase')
  const snapshot = structuredClone(await backend.getAppSnapshot())
  const dashboard = structuredClone(await backend.loadDashboardSnapshot())
  return { dashboard, snapshot }
}

/**
 * Explains how shell probe works.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
function ShellProbe({ onReady }: { onReady?: () => void }) {
  const shell = useShellData()

  useEffect(() => {
    if (!shell.loading) {
      onReady?.()
    }
  }, [onReady, shell.loading])

  return (
    <div>
      <div data-testid="loading">{String(shell.loading)}</div>
      <div data-testid="refresh-key">{shell.refreshKey}</div>
      <div data-testid="notice">{shell.notice ?? 'none'}</div>
      <div data-testid="error">{shell.error ?? 'none'}</div>
      <div data-testid="snapshot-language">
        {shell.snapshot?.config.preferredLanguage ?? 'none'}
      </div>
      <div data-testid="app-lock-enabled">
        {String(shell.appLockStatus?.enabled ?? false)}
      </div>
      <div data-testid="app-lock-locked">
        {String(shell.appLockStatus?.locked ?? false)}
      </div>
      <div data-testid="busy-label">{shell.busyOverlay?.label ?? 'none'}</div>
      <div data-testid="busy-detail">{shell.busyOverlay?.detail ?? 'none'}</div>
      <div data-testid="busy-progress-label">
        {shell.busyOverlay?.progressLabel ?? 'none'}
      </div>
      <div data-testid="busy-progress-value">
        {shell.busyOverlay?.progressValue?.toString() ?? 'none'}
      </div>
      <button
        type="button"
        onClick={() => {
          void shell.refreshAppData().catch(() => undefined)
        }}
      >
        refresh
      </button>
      <button
        type="button"
        onClick={() =>
          void shell
            .saveConfig({
              ...(shell.snapshot?.config ?? baseConfig),
              preferredLanguage: 'zh-CN',
            })
            .catch(() => undefined)
        }
      >
        save
      </button>
      <button
        type="button"
        onClick={() =>
          void shell
            .initializeArchive({
              ...baseConfig,
              preferredLanguage: 'zh-TW',
            })
            .catch(() => undefined)
        }
      >
        initialize
      </button>
      <button
        type="button"
        onClick={() => {
          void shell.runBackup().catch(() => undefined)
        }}
      >
        backup
      </button>
      <button
        type="button"
        onClick={() => {
          void shell
            .setAppLockPasscode({
              passcode: '2468',
              recoveryHint: 'digits only',
            })
            .catch(() => undefined)
        }}
      >
        set-passcode
      </button>
      <button
        type="button"
        onClick={() => {
          void shell
            .saveConfig({
              ...(shell.snapshot?.config ?? baseConfig),
              appLock: {
                ...(shell.snapshot?.config.appLock ?? baseConfig.appLock),
                enabled: true,
                idleTimeoutMinutes: 1,
              },
            })
            .catch(() => undefined)
        }}
      >
        enable-lock
      </button>
      <button
        type="button"
        onClick={() => {
          void shell.clearAppLockPasscode().catch(() => undefined)
        }}
      >
        clear-passcode
      </button>
      <button
        type="button"
        onClick={() => {
          void shell.lockAppSession('manual').catch(() => undefined)
        }}
      >
        lock
      </button>
      <button
        type="button"
        onClick={() => {
          void shell.lockAppSession().catch(() => undefined)
        }}
      >
        lock-default
      </button>
      <button
        type="button"
        onClick={() => {
          void shell
            .unlockAppSession({
              passcode: '2468',
              useBiometric: false,
            })
            .catch(() => undefined)
        }}
      >
        unlock
      </button>
      <button type="button" onClick={() => shell.clearNotice()}>
        clear
      </button>
    </div>
  )
}

describe('ShellDataProvider', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    backendTestHarness.reset()
    vi.mocked(subscribeToBackupProgress).mockResolvedValue(vi.fn())
  })

  test('loads and mutates shell data through provider actions', async () => {
    const user = userEvent.setup()
    const languageSpy = vi.fn()
    const unsubscribe = vi.fn()
    const { dashboard, snapshot } = await seedSnapshot()
    const savedSnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, preferredLanguage: 'zh-CN' },
    }
    const initializedSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        initialized: true,
        preferredLanguage: 'zh-TW',
      },
    }
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'saveConfig').mockResolvedValue(savedSnapshot)
    vi.spyOn(backend, 'initializeArchive').mockResolvedValue(
      initializedSnapshot,
    )
    vi.mocked(subscribeToBackupProgress).mockResolvedValueOnce(unsubscribe)
    vi.spyOn(backend, 'runBackupNow').mockResolvedValue({
      dueSkipped: false,
      run: {
        id: 42,
        startedAt: '2026-04-07T00:00:00Z',
        finishedAt: '2026-04-07T00:05:00Z',
        status: 'success',
        manifestHash: 'manifest-42',
        profileScope: ['chrome:Default'],
        profilesProcessed: 1,
        newVisits: 2,
        newUrls: 1,
        newDownloads: 0,
        runType: 'backup',
      },
      profiles: [],
      warnings: [],
      remoteBackup: null,
    })

    render(
      <I18nContext.Provider value={createI18nValue('en', languageSpy)}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(languageSpy).toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).toHaveTextContent(
        'zh-CN',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).not.toHaveTextContent('none'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(/run #42/i),
    )
    expect(subscribeToBackupProgress).toHaveBeenCalledTimes(1)
    expect(unsubscribe).toHaveBeenCalledTimes(1)

    await user.click(screen.getByRole('button', { name: 'clear' }))
    expect(screen.getByTestId('notice')).toHaveTextContent('none')

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('refresh-key')).not.toHaveTextContent('0'),
    )
  })

  test('auto-unlocks a remembered archive key once and reuses the session key afterwards', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const rememberedSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        rememberDatabaseKeyInKeyring: true,
      },
      archiveStatus: {
        ...snapshot.archiveStatus,
        encrypted: true,
        unlocked: false,
      },
      keyringStatus: {
        ...snapshot.keyringStatus,
        available: true,
        storedSecret: true,
      },
    }
    const unlockedSnapshot: AppSnapshot = {
      ...snapshot,
      config: rememberedSnapshot.config,
      keyringStatus: rememberedSnapshot.keyringStatus,
    }

    const keyringSpy = vi
      .spyOn(backend, 'keyringGetDatabaseKey')
      .mockResolvedValue('vault-passphrase')
    const sessionSpy = vi
      .spyOn(backend, 'setSessionDatabaseKey')
      .mockResolvedValue(undefined)
    const getAppSnapshotSpy = vi
      .spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(rememberedSnapshot)
      .mockResolvedValue(unlockedSnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(
      snapshot.appLockStatus,
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(keyringSpy).toHaveBeenCalledTimes(1)
    expect(sessionSpy).toHaveBeenCalledWith('vault-passphrase')
    const snapshotCallsAfterBoot = getAppSnapshotSpy.mock.calls.length
    expect(snapshotCallsAfterBoot).toBeGreaterThanOrEqual(2)

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(getAppSnapshotSpy.mock.calls.length).toBeGreaterThan(
        snapshotCallsAfterBoot,
      ),
    )
    expect(keyringSpy).toHaveBeenCalledTimes(1)
  })

  test('keeps the locked snapshot when the keyring returns no database key', async () => {
    const { dashboard, snapshot } = await seedSnapshot()
    const rememberedSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        rememberDatabaseKeyInKeyring: true,
      },
      archiveStatus: {
        ...snapshot.archiveStatus,
        encrypted: true,
        unlocked: false,
      },
      keyringStatus: {
        ...snapshot.keyringStatus,
        available: true,
        storedSecret: true,
      },
    }

    const keyringSpy = vi
      .spyOn(backend, 'keyringGetDatabaseKey')
      .mockResolvedValue(null)
    const sessionSpy = vi
      .spyOn(backend, 'setSessionDatabaseKey')
      .mockResolvedValue(undefined)
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(rememberedSnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(
      snapshot.appLockStatus,
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(keyringSpy).toHaveBeenCalledTimes(1)
    expect(sessionSpy).not.toHaveBeenCalled()
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false')
  })

  test('uses the paint fallback and surfaces refresh errors without breaking follow-up saves', async () => {
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const savedSnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, preferredLanguage: 'zh-CN' },
    }
    const originalRequestAnimationFrame = window.requestAnimationFrame
    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      writable: true,
      value: undefined,
    })

    const getAppSnapshotSpy = vi
      .spyOn(backend, 'getAppSnapshot')
      .mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadDashboardSnapshot')
      .mockResolvedValueOnce(dashboard)
      .mockRejectedValueOnce(new Error('follow-up dashboard refresh failed'))
      .mockResolvedValue(dashboard)
    vi.spyOn(backend, 'saveConfig').mockResolvedValue(savedSnapshot)

    try {
      render(
        <I18nContext.Provider value={createI18nValue('en')}>
          <ShellDataProvider>
            <ShellProbe />
          </ShellDataProvider>
        </I18nContext.Provider>,
      )

      await waitFor(() =>
        expect(screen.getByTestId('loading')).toHaveTextContent('false'),
      )

      await user.click(screen.getByRole('button', { name: 'save' }))
      await waitFor(() =>
        expect(screen.getByTestId('snapshot-language')).toHaveTextContent(
          'zh-CN',
        ),
      )
      expect(screen.getByTestId('error')).toHaveTextContent('none')

      getAppSnapshotSpy.mockRejectedValueOnce('not-an-error')
      await user.click(screen.getByRole('button', { name: 'refresh' }))
      await waitFor(() =>
        expect(screen.getByTestId('error')).toHaveTextContent(
          translator('shell.loadingLatestArchiveState'),
        ),
      )

      getAppSnapshotSpy.mockRejectedValueOnce(new Error('refresh failed'))
      await user.click(screen.getByRole('button', { name: 'refresh' }))
      await waitFor(() =>
        expect(screen.getByTestId('error')).toHaveTextContent('refresh failed'),
      )
    } finally {
      Object.defineProperty(window, 'requestAnimationFrame', {
        configurable: true,
        writable: true,
        value: originalRequestAnimationFrame,
      })
    }
  })

  test('surfaces initial refresh failures without leaking an unhandled rejection', async () => {
    const { snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(
      snapshot.appLockStatus,
    )
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'getAppSnapshot').mockRejectedValueOnce(
      new Error('initial refresh failed'),
    )

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('error')).toHaveTextContent(
      'initial refresh failed',
    )
  })

  test('ignores follow-up dashboard refresh failures after saving config', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const savedSnapshot: AppSnapshot = {
      ...snapshot,
      config: { ...snapshot.config, preferredLanguage: 'zh-CN' },
    }
    const loadDashboardSnapshotSpy = vi
      .spyOn(backend, 'loadDashboardSnapshot')
      .mockResolvedValueOnce(dashboard)
      .mockRejectedValueOnce(new Error('follow-up dashboard refresh failed'))
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'saveConfig').mockResolvedValue(savedSnapshot)

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).toHaveTextContent(
        'zh-CN',
      ),
    )
    await waitFor(() =>
      expect(loadDashboardSnapshotSpy).toHaveBeenCalledTimes(2),
    )
    expect(screen.getByTestId('error')).toHaveTextContent('none')
  })

  test('boots into the lock state and reloads shell data after unlock', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const lockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: true,
      passcodeConfigured: true,
      lockReason: 'startup',
    }
    const unlockedStatus = {
      ...lockedStatus,
      locked: false,
      lockReason: null,
      lastUnlockedAt: '2026-04-08T01:00:00Z',
    }

    const getAppSnapshotSpy = vi
      .spyOn(backend, 'getAppSnapshot')
      .mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadAppLockStatus')
      .mockResolvedValueOnce(lockedStatus)
      .mockResolvedValue(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'unlockAppSession').mockResolvedValue(unlockedStatus)

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('true')
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true')
    expect(screen.getByTestId('snapshot-language')).toHaveTextContent('none')
    expect(getAppSnapshotSpy).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false'),
    )
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
        'none',
      ),
    )
    expect(getAppSnapshotSpy).toHaveBeenCalledTimes(1)
  })

  test('runs app lock success actions through the shell provider', async () => {
    const user = userEvent.setup()

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'set-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'enable-lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('true'),
    )
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false')

    await user.click(screen.getByRole('button', { name: 'lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true'),
    )
    expect(screen.getByTestId('snapshot-language')).toHaveTextContent('none')

    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false'),
    )
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
        'none',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'lock-default' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true'),
    )
    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'clear-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('false'),
    )
  })

  test('auto-locks after idle timeout when app lock is enabled', async () => {
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
      idleTimeoutMinutes: 1,
    }
    const lockedStatus = {
      ...unlockedStatus,
      locked: true,
      lockReason: 'idle-timeout',
    }

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        appLock: {
          ...snapshot.config.appLock,
          enabled: true,
          idleTimeoutMinutes: 1,
          passcodeConfigured: true,
        },
      },
      appLockStatus: unlockedStatus,
    })
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    const lockSpy = vi
      .spyOn(backend, 'lockAppSession')
      .mockResolvedValue(lockedStatus)

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('true')

    const visibilityDescriptor = Object.getOwnPropertyDescriptor(
      document,
      'visibilityState',
    )
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })

    try {
      await act(async () => {
        Object.defineProperty(document, 'visibilityState', {
          configurable: true,
          value: 'hidden',
        })
        document.dispatchEvent(new Event('visibilitychange'))
        await Promise.resolve()
        Object.defineProperty(document, 'visibilityState', {
          configurable: true,
          value: 'visible',
        })
        window.dispatchEvent(new Event('pointerdown'))
        document.dispatchEvent(new Event('visibilitychange'))
        await Promise.resolve()
      })

      await act(async () => {
        vi.advanceTimersByTime(60_000)
        await Promise.resolve()
        await Promise.resolve()
      })

      vi.runOnlyPendingTimers()
      vi.useRealTimers()

      await waitFor(() => expect(lockSpy).toHaveBeenCalledWith('idle-timeout'))
      await waitFor(() =>
        expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true'),
      )
      expect(screen.getByTestId('snapshot-language')).toHaveTextContent('none')
    } finally {
      if (vi.isFakeTimers()) {
        vi.runOnlyPendingTimers()
      }
      if (visibilityDescriptor) {
        Object.defineProperty(document, 'visibilityState', visibilityDescriptor)
      } else {
        delete (document as { visibilityState?: string }).visibilityState
      }
      vi.useRealTimers()
    }
  })

  test('keeps the original error when a lock refresh still reports an unlocked session', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
    }

    vi.spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce(
        new Error(
          'PathKeep is currently locked. Unlock the app before requesting archive data.',
        ),
      )
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadAppLockStatus')
      .mockResolvedValueOnce(unlockedStatus)
      .mockResolvedValueOnce(unlockedStatus)
      .mockResolvedValueOnce(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('currently locked'),
    )
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false')
  })

  test('falls back to locked app state when archive refresh reports a lock error', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
    }
    const lockedStatus = {
      ...unlockedStatus,
      locked: true,
      lockReason: 'manual',
    }

    vi.spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce(
        new Error(
          'PathKeep is currently locked. Unlock the app before requesting archive data.',
        ),
      )
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadAppLockStatus')
      .mockResolvedValueOnce(unlockedStatus)
      .mockResolvedValueOnce(unlockedStatus)
      .mockResolvedValueOnce(lockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false')

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true'),
    )
    expect(screen.getByTestId('snapshot-language')).toHaveTextContent('none')
    expect(screen.getByTestId('error')).toHaveTextContent('none')
  })

  test('surfaces idle-timeout lock failures without clearing the loaded shell state', async () => {
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
      idleTimeoutMinutes: 1,
    }

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        appLock: {
          ...snapshot.config.appLock,
          enabled: true,
          idleTimeoutMinutes: 1,
          passcodeConfigured: true,
        },
      },
      appLockStatus: unlockedStatus,
    })
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    const lockSpy = vi
      .spyOn(backend, 'lockAppSession')
      .mockRejectedValueOnce(new Error('idle lock failed'))
      .mockRejectedValueOnce('not-an-error')

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })

    try {
      await act(async () => {
        window.dispatchEvent(new Event('pointerdown'))
        await Promise.resolve()
      })
      await act(async () => {
        vi.advanceTimersByTime(60_000)
        await Promise.resolve()
        await Promise.resolve()
      })
      vi.runOnlyPendingTimers()
      vi.useRealTimers()

      await waitFor(() =>
        expect(screen.getByTestId('error')).toHaveTextContent(
          'idle lock failed',
        ),
      )
      expect(lockSpy).toHaveBeenCalledWith('idle-timeout')
      expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
        'none',
      )

      vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
      await act(async () => {
        window.dispatchEvent(new Event('pointerdown'))
        await Promise.resolve()
      })
      await act(async () => {
        vi.advanceTimersByTime(60_000)
        await Promise.resolve()
        await Promise.resolve()
      })
      vi.runOnlyPendingTimers()
      vi.useRealTimers()

      await waitFor(() =>
        expect(screen.getByTestId('error')).toHaveTextContent(
          translator('shell.lockAppFailed'),
        ),
      )
      expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
        'none',
      )
    } finally {
      if (vi.isFakeTimers()) {
        vi.runOnlyPendingTimers()
      }
      vi.useRealTimers()
    }
  })

  test('surfaces app lock action failures with both explicit and fallback errors', async () => {
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
      idleTimeoutMinutes: 1,
    }

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        appLock: {
          ...snapshot.config.appLock,
          enabled: true,
          idleTimeoutMinutes: 1,
          passcodeConfigured: true,
        },
      },
      appLockStatus: unlockedStatus,
    })
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'setAppLockPasscode')
      .mockRejectedValueOnce(new Error('set passcode failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'clearAppLockPasscode')
      .mockRejectedValueOnce(new Error('clear passcode failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'lockAppSession')
      .mockRejectedValueOnce(new Error('lock failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'unlockAppSession')
      .mockRejectedValueOnce(new Error('unlock failed'))
      .mockRejectedValueOnce('not-an-error')

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'set-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'set passcode failed',
      ),
    )
    await user.click(screen.getByRole('button', { name: 'set-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.setAppLockPasscodeFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'clear-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'clear passcode failed',
      ),
    )
    await user.click(screen.getByRole('button', { name: 'clear-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.clearAppLockPasscodeFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('lock failed'),
    )
    await user.click(screen.getByRole('button', { name: 'lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.lockAppFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('unlock failed'),
    )
    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.unlockAppFailed'),
      ),
    )
  })

  test('surfaces provider errors and context misuse clearly', async () => {
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined)
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'saveConfig')
      .mockRejectedValueOnce(new Error('save failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'initializeArchive')
      .mockRejectedValueOnce(new Error('initialize failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'runBackupNow')
      .mockRejectedValueOnce(new Error('backup failed'))
      .mockRejectedValueOnce('not-an-error')

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('save failed'),
    )

    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.savingSettingsFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'initialize failed',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.initializeArchiveFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('backup failed'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.manualBackupFailed'),
      ),
    )

    vi.mocked(subscribeToBackupProgress).mockRejectedValueOnce(
      new Error('subscribe failed'),
    )
    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('subscribe failed'),
    )

    expect(() => render(<ShellProbe />)).toThrow(
      'useShellData must be used inside ShellDataProvider',
    )

    consoleError.mockRestore()
  })

  test('surfaces due-window and generic completion notices for manual backups', async () => {
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'runBackupNow')
      .mockResolvedValueOnce({
        dueSkipped: true,
        reason: 'Backup is still within the due window.',
        run: null,
        profiles: [],
        warnings: [],
        remoteBackup: null,
      })
      .mockResolvedValueOnce({
        dueSkipped: true,
        reason: null,
        run: null,
        profiles: [],
        warnings: [],
        remoteBackup: null,
      })
      .mockResolvedValueOnce({
        dueSkipped: false,
        run: null,
        profiles: [],
        warnings: [],
        remoteBackup: null,
      })

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(
        'Backup is still within the due window.',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(
        translator('shell.manualBackupDueWindow'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(
        translator('common.complete'),
      ),
    )
  })

  test('tracks backup progress phases through the shared busy overlay state', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const translator = createTranslator('en')
    const unsubscribe = vi.fn()
    let listener: ((event: BackupProgressEvent) => void) | null = null
    let resolveBackup: ((value: BackupReport) => void) | null = null

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue({
      productName: 'PathKeep',
      version: '0.1.0',
      gitCommitShort: 'abc123',
      gitCommitFull: 'abc123def456',
      gitDirty: false,
    })
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.mocked(subscribeToBackupProgress).mockImplementation((nextListener) => {
      listener = nextListener
      return Promise.resolve(unsubscribe)
    })
    vi.spyOn(backend, 'runBackupNow').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveBackup = resolve
        }),
    )

    render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ShellDataProvider>
          <ShellProbe />
        </ShellDataProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.backupWritingArchive'),
      ),
    )
    expect(screen.getByTestId('busy-progress-label')).toHaveTextContent('2 / 3')
    expect(
      Number(screen.getByTestId('busy-progress-value').textContent),
    ).toBeCloseTo(67, 0)

    act(() => {
      listener?.({
        phase: 'prepare',
        label: 'Inspect selected browser profiles',
        detail: 'Queued 3 readable profile(s) for the canonical backup run.',
        step: 0,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 3,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-progress-label')).toHaveTextContent(
        '0 / 3',
      ),
    )

    act(() => {
      listener?.({
        phase: 'stage-profile',
        label: 'Stage source profile',
        detail: 'Copying chrome:Default into the staging area (1/3).',
        step: 1,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 3,
        profileId: 'chrome:Default',
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-detail')).toHaveTextContent(
        'chrome:Default (1/3)',
      ),
    )
    expect(
      Number(screen.getByTestId('busy-progress-value').textContent),
    ).toBeCloseTo(33, 0)

    act(() => {
      listener?.({
        phase: 'ingest-profile',
        label: 'Write canonical archive facts',
        detail: 'Processing chrome:Default and writing archive rows (2/3).',
        step: 1,
        totalSteps: 3,
        completedProfiles: 1,
        totalProfiles: 3,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.backupWritingArchive'),
      ),
    )

    act(() => {
      listener?.({
        phase: 'finalize',
        label: 'Finalize manifest and cached totals',
        detail: 'Committing run artifacts after 3 processed profile(s).',
        step: 2,
        totalSteps: 3,
        completedProfiles: 3,
        totalProfiles: 3,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-progress-label')).toHaveTextContent(
        '3 / 3',
      ),
    )
    expect(screen.getByTestId('busy-progress-value')).toHaveTextContent('100')

    act(() => {
      listener?.({
        phase: 'mystery',
        label: 'Unexpected phase',
        detail: 'Fallback branch should still stay honest.',
        step: 0,
        totalSteps: 3,
        completedProfiles: 0,
        totalProfiles: 0,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.runningManualBackup'),
      ),
    )
    expect(screen.getByTestId('busy-progress-label')).toHaveTextContent('1 / 3')
    expect(
      Number(screen.getByTestId('busy-progress-value').textContent),
    ).toBeCloseTo(33, 0)

    act(() => {
      listener?.({
        phase: 'stage-profile',
        label: 'Stage source profile',
        detail: 'Fallback branch without profile scope.',
        step: 0,
        totalSteps: 0,
        completedProfiles: 0,
        totalProfiles: 0,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.backupWritingArchive'),
      ),
    )
    expect(screen.getByTestId('busy-progress-label')).toHaveTextContent('0 / 0')
    expect(screen.getByTestId('busy-progress-value')).toHaveTextContent('none')

    act(() => {
      listener?.({
        phase: 'finalize',
        label: 'Finalize without profile counts',
        detail: 'Fallback branch without totals.',
        step: 0,
        totalSteps: 0,
        completedProfiles: 0,
        totalProfiles: 0,
        profileId: null,
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId('busy-label')).toHaveTextContent(
        translator('shell.refreshingArchiveViews'),
      ),
    )
    expect(screen.getByTestId('busy-progress-label')).toHaveTextContent('0 / 0')
    expect(screen.getByTestId('busy-progress-value')).toHaveTextContent('none')

    act(() => {
      resolveBackup?.({
        dueSkipped: false,
        run: {
          id: 73,
          startedAt: '2026-04-08T00:00:00Z',
          finishedAt: '2026-04-08T00:05:00Z',
          status: 'success',
          manifestHash: 'manifest-73',
          profileScope: ['chrome:Default'],
          profilesProcessed: 1,
          newVisits: 2,
          newUrls: 1,
          newDownloads: 0,
          runType: 'backup',
        },
        profiles: [],
        warnings: [],
        remoteBackup: null,
      })
    })

    await waitFor(() =>
      expect(screen.getByTestId('notice')).toHaveTextContent(/run #73/i),
    )
    expect(unsubscribe).toHaveBeenCalledTimes(1)
    expect(screen.getByTestId('busy-label')).toHaveTextContent('none')
  })
})
