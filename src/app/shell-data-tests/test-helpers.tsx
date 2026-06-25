/* eslint-disable react-refresh/only-export-components */
/**
 * @file test-helpers.tsx
 * @description Shared test harness for the split `src/app/shell-data.test.tsx` suites.
 * @module app/shell-data-tests
 *
 * ## Responsibilities
 * - Hold the one canonical shell-data fixture contract used by every split suite.
 * - Reuse the original probe component, i18n wrapper, archive seed path, and backup-progress mock setup.
 * - Keep test-only shell interactions readable without cloning provider boilerplate into every suite.
 *
 * ## Not responsible for
 * - Owning route-specific assertions or deciding which suite covers which shell-data behavior.
 * - Changing `ShellDataProvider` behavior or inventing new test-only abstractions beyond the legacy mega-suite contract.
 * - Hiding side effects that individual tests still need to mock explicitly.
 *
 * ## Dependencies
 * - Depends on the backend test harness, i18n context helpers, and the real `ShellDataProvider`.
 * - Uses the same `subscribeToBackupProgress` mock surface as the original mega-suite.
 *
 * ## Performance notes
 * - Centralizes archive bootstrap and provider rendering so the split suites avoid duplicating expensive setup logic.
 */

import { useEffect } from 'react'
import { render } from '@testing-library/react'
import { vi, expect } from 'vitest'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import { defaultExplorerBackgroundPrefetchPages } from '../../lib/explorer-preferences'
import { subscribeToBackupProgress } from '../../lib/ipc/backup-progress'
import { subscribeToImportProgress } from '../../lib/ipc/import-progress'
import { I18nContext, type I18nContextValue } from '../../lib/i18n/context'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from '../../lib/i18n'
import type { AppConfig } from '../../lib/types'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import { ShellDataProvider } from '../shell-data'
import { useShellData } from '../shell-data-context'

vi.mock('../../lib/ipc/backup-progress', () => ({
  subscribeToBackupProgress: vi.fn(() => Promise.resolve(vi.fn())),
}))

vi.mock('../../lib/ipc/import-progress', () => ({
  subscribeToImportProgress: vi.fn(() => Promise.resolve(vi.fn())),
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
  explorerBackgroundPrefetchPages: defaultExplorerBackgroundPrefetchPages,
  appLock: {
    enabled: false,
    idleTimeoutMinutes: 5,
    biometricEnabled: false,
    passcodeEnabled: true,
    passcodeConfigured: false,
    recoveryHint: null,
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
 * Rebuilds the shared shell-data baseline before each suite so mocked provider behavior
 * stays isolated and backup-progress subscriptions never leak across files.
 */
export function resetShellDataHarness() {
  vi.restoreAllMocks()
  backendTestHarness.reset()
  vi.mocked(subscribeToBackupProgress).mockResolvedValue(vi.fn())
  vi.mocked(subscribeToImportProgress).mockResolvedValue(vi.fn())
}

/**
 * Returns the canonical mocked backup-progress subscription function so split suites can
 * override streaming behavior without importing a second owner for the same mock surface.
 */
export function getBackupProgressMock() {
  return vi.mocked(subscribeToBackupProgress)
}

/**
 * Returns the canonical mocked import-progress subscription function so shell task
 * suites can drive import progress without opening a desktop event stream.
 */
export function getImportProgressMock() {
  return vi.mocked(subscribeToImportProgress)
}

/**
 * Builds the same i18n contract the shell provider expects in production while still
 * letting tests observe language-preference updates.
 *
 * `setLanguagePreference` is optional so suites only need to pass a spy when they are
 * asserting the language handoff explicitly.
 */
export function createI18nValue(
  language: ResolvedLanguage,
  setLanguagePreference: I18nContextValue['setLanguagePreference'] = vi.fn() as I18nContextValue['setLanguagePreference'],
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
 * Seeds an initialized archive plus dashboard snapshot using the legacy mega-suite
 * bootstrap path, so split suites keep the same read-model assumptions.
 */
export async function seedSnapshot() {
  await backend.initializeArchive(baseConfig, 'vault-passphrase')
  const snapshot = structuredClone(await backend.getAppSnapshot())
  const dashboard = structuredClone(await backend.loadDashboardSnapshot())
  return { dashboard, snapshot }
}

/**
 * Probes `ShellDataProvider` state through stable test IDs and button actions.
 *
 * This intentionally mirrors the legacy mega-suite probe so the split only changes file
 * boundaries, not what each test can observe or trigger.
 */
export function ShellProbe({ onReady }: { onReady?: () => void }) {
  const shell = useShellData()

  useEffect(() => {
    if (!shell.loading) {
      onReady?.()
    }
  }, [onReady, shell.loading])

  return (
    <div>
      <div data-testid="loading">{String(shell.loading)}</div>
      <div data-testid="dashboard-loading">
        {String(shell.dashboardLoading)}
      </div>
      <div data-testid="refresh-key">{shell.refreshKey}</div>
      <div data-testid="notice">{shell.notice ?? 'none'}</div>
      <div data-testid="error">{shell.error ?? 'none'}</div>
      <div data-testid="error-kind">{shell.errorKind ?? 'none'}</div>
      <div data-testid="snapshot-language">
        {shell.snapshot?.config.preferredLanguage ?? 'none'}
      </div>
      <div data-testid="dashboard-generated-at">
        {shell.dashboard?.generatedAt ?? 'none'}
      </div>
      <div data-testid="runtime-running">
        {shell.runtimeStatus?.intelligence?.queue.running ?? 'none'}
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
      <div data-testid="archive-task-count">
        {shell.archiveTasks?.length.toString() ?? '0'}
      </div>
      <div data-testid="latest-archive-task">
        {shell.latestArchiveTask?.title ?? 'none'}
      </div>
      <div data-testid="active-archive-task">
        {shell.activeArchiveTask?.title ?? 'none'}
      </div>
      <div data-testid="unread-notifications">
        {shell.unreadNotificationCount?.toString() ?? '0'}
      </div>
      <div data-testid="notification-count">
        {shell.notifications?.length.toString() ?? '0'}
      </div>
      <div data-testid="notification-titles">
        {shell.notifications?.map((entry) => entry.title).join('|') ?? ''}
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
        onClick={() => {
          void shell.refreshRuntimeStatus().catch(() => undefined)
        }}
      >
        refresh-runtime
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
            .runImport?.({
              method: 'takeout',
              request: {
                sourcePath: '/tmp/Takeout',
                dryRun: false,
              },
              expectedRecords: 2,
            })
            .catch(() => undefined)
        }}
      >
        import-takeout
      </button>
      <button
        type="button"
        onClick={() => {
          void shell
            .runImport?.({
              method: 'browser',
              request: {
                sourcePath: '/profiles/Default/History',
                dryRun: false,
                browserFamily: 'chromium',
                browserName: 'Chrome',
                profileId: 'chrome:Default',
                profileName: 'Default',
              },
              expectedRecords: 3,
            })
            .catch(() => undefined)
        }}
      >
        import-browser
      </button>
      <button
        type="button"
        onClick={() => {
          void shell
            .runImport?.({
              method: 'browser',
              request: {
                sourcePath: '/profiles/Profile 1/History',
                dryRun: false,
                browserFamily: null,
                browserName: null,
                profileId: 'chrome:Profile 1',
                profileName: null,
              },
              expectedRecords: null,
              sourceLabel: null,
            })
            .catch(() => undefined)
        }}
      >
        import-browser-profile-id
      </button>
      <button
        type="button"
        onClick={() => {
          void shell
            .runImport?.({
              method: 'browser',
              request: {
                sourcePath: '/manual/History',
                dryRun: false,
                browserFamily: null,
                browserName: null,
                profileId: null,
                profileName: null,
              },
              expectedRecords: null,
              sourceLabel: null,
            })
            .catch(() => undefined)
        }}
      >
        import-browser-no-profile
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
      <button type="button" onClick={() => shell.clearError()}>
        clear-error
      </button>
      <button type="button" onClick={() => shell.markNotificationsRead?.()}>
        mark-notifications
      </button>
      <button
        type="button"
        onClick={() => shell.dismissNotification?.('valid')}
      >
        dismiss-notification
      </button>
    </div>
  )
}

/**
 * Renders the canonical shell-data provider + probe wrapper for split suites so each
 * test file can focus on mocked behavior instead of re-declaring provider plumbing.
 */
export function renderShellProbe(options?: {
  language?: ResolvedLanguage
  setLanguagePreference?: I18nContextValue['setLanguagePreference']
  onReady?: () => void
}) {
  const {
    language = 'en',
    setLanguagePreference = vi.fn() as I18nContextValue['setLanguagePreference'],
    onReady,
  } = options ?? {}

  return render(
    <I18nContext.Provider
      value={createI18nValue(language, setLanguagePreference)}
    >
      <ProfileScopeProvider>
        <ShellDataProvider>
          <ShellProbe onReady={onReady} />
        </ShellDataProvider>
      </ProfileScopeProvider>
    </I18nContext.Provider>,
  )
}

/**
 * Narrows nullable DOM nodes to concrete `HTMLElement`s when shell-data suites need to
 * inspect rendered panels or busy overlay output.
 */
export function expectHtmlElement(node: Element | null): HTMLElement {
  expect(node).toBeInstanceOf(HTMLElement)
  return node as HTMLElement
}

/**
 * Provides the stable build-info fixture repeatedly used by shell-data tests so suites
 * do not need to duplicate the same desktop metadata object inline.
 */
export function getDefaultBuildInfo(): NonNullable<
  Awaited<ReturnType<typeof backend.getAppBuildInfo>>
> {
  return {
    productName: 'PathKeep',
    version: '0.1.0',
    gitCommitShort: 'abc123',
    gitCommitFull: 'abc123def456',
    gitDirty: false,
  }
}
