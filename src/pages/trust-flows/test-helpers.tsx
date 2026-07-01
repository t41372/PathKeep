/**
 * @file test-helpers.tsx
 * @description Shared harness for trust-flow route tests across Import, Schedule, Security, Settings, and Audit.
 * @module pages/trust-flows
 *
 * ## Responsibilities
 * - Provide one canonical render harness for trust-flow route surfaces.
 * - Seed an initialized archive snapshot that split suites can reuse.
 * - Centralize shell-context setup and the common reset contract used by trust-flow tests.
 *
 * ## Non-Responsibilities
 * - Does not own route-specific mocks, PMEs, or assertions.
 * - Does not install top-level `vi.mock(...)` declarations for Tauri modules; each suite still controls its own mocked module boundary.
 * - Does not define audit/import/schedule fixture payloads that belong to one suite only.
 *
 * ## Dependencies
 * - Depends on the shipped i18n, profile-scope, shell-data, and backend harness modules.
 * - Uses the backend test harness to create a realistic initialized snapshot instead of page-local stubs.
 *
 * ## Performance Notes
 * - Reuses one seeded archive path and one shared reset shape so splitting the mega-suite does not multiply setup cost.
 */

import type { ReactNode } from 'react'
import { render } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { expect, vi } from 'vitest'
import {
  ShellDataContext,
  type ShellImportTaskRequest,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from '../../lib/i18n'
import { I18nContext, type I18nContextValue } from '../../lib/i18n/context'
import { defaultExplorerBackgroundPrefetchPages } from '../../lib/explorer-preferences'
import { subscribeToImportProgress } from '../../lib/ipc/import-progress'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import type { AppConfig, AppSnapshot, DashboardSnapshot } from '../../lib/types'

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
    assistantSystemPrompt:
      'You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

type TrustFlowMocks = {
  invoke: ReturnType<typeof vi.fn>
  isTauri: ReturnType<typeof vi.fn>
  subscribeToImportProgress: ReturnType<typeof vi.fn>
}

/**
 * Creates the i18n context used by trust-flow route suites.
 *
 * @param language The resolved locale the suite wants to render.
 * @returns An `I18nContextValue` suitable for route rendering.
 */
export function createI18nValue(language: ResolvedLanguage): I18nContextValue {
  const namespaceCache = new Map<string, ReturnType<typeof createTranslator>>()

  return {
    language,
    preference: language,
    setLanguagePreference: vi.fn(),
    t: createTranslator(language),
    ns: (namespace) => {
      const cached = namespaceCache.get(namespace)
      if (cached) {
        return cached
      }

      const translator = createNamespaceTranslator(language, namespace)
      namespaceCache.set(namespace, translator)
      return translator
    },
  }
}

/**
 * Creates the default shell-data context used by trust-flow suites.
 *
 * @param snapshot The app snapshot under test.
 * @param dashboard Optional dashboard snapshot for routes that read it.
 * @returns A shell-data context value with stubbed mutations.
 */
export function createShellValue(
  snapshot: AppSnapshot,
  dashboard: DashboardSnapshot | null = null,
): ShellDataContextValue {
  return {
    buildInfo: null,
    appLockStatus: snapshot.appLockStatus,
    snapshot,
    dashboard,
    loading: false,
    busyAction: null,
    busyOverlay: null,
    error: null,
    notice: null,
    refreshKey: 0,
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn().mockResolvedValue({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    }),
    saveConfig: vi.fn().mockResolvedValue(snapshot),
    initializeArchive: vi.fn().mockResolvedValue(snapshot),
    runBackup: vi.fn().mockResolvedValue({
      dueSkipped: false,
      run: null,
      profiles: [],
      warnings: [],
    }),
    runImport: vi.fn(async (request: ShellImportTaskRequest) => {
      const unsubscribe = await subscribeToImportProgress(() => undefined)
      try {
        return request.method === 'takeout'
          ? await backend.importTakeout(request.request)
          : await backend.importBrowserHistory(request.request)
      } finally {
        unsubscribe()
      }
    }),
    setAppLockPasscode: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    clearAppLockPasscode: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    lockAppSession: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    unlockAppSession: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    clearNotice: vi.fn(),
    errorKind: null,
    clearError: vi.fn(),
    recovery: null,
    runFullArchiveRestore: vi.fn().mockResolvedValue({}),
  }
}

/**
 * Keeps element-type assertions readable when suites need to climb into panel-level DOM.
 *
 * @param node The queried node.
 * @returns The same node narrowed to `HTMLElement`.
 */
export function expectHtmlElement(node: Element | null): HTMLElement {
  expect(node).toBeInstanceOf(HTMLElement)
  return node as HTMLElement
}

/**
 * Renders one trust-flow route inside the canonical provider stack used by this suite family.
 *
 * @param ui The route surface under test.
 * @param options The route, locale, snapshot, and optional dashboard override.
 * @returns The Testing Library render result for follow-up assertions.
 */
export function renderTrustPage(
  ui: ReactNode,
  {
    dashboard = null,
    language = 'en' as ResolvedLanguage,
    route = '/',
    shellValue = null,
    snapshot,
  }: {
    dashboard?: DashboardSnapshot | null
    language?: ResolvedLanguage
    route?: string
    shellValue?: ShellDataContextValue | null
    snapshot: AppSnapshot
  },
) {
  return render(
    <MemoryRouter initialEntries={[route]}>
      <I18nContext.Provider value={createI18nValue(language)}>
        <ProfileScopeProvider>
          <ShellDataContext.Provider
            value={shellValue ?? createShellValue(snapshot, dashboard)}
          >
            {ui}
          </ShellDataContext.Provider>
        </ProfileScopeProvider>
      </I18nContext.Provider>
    </MemoryRouter>,
  )
}

/**
 * Seeds an initialized snapshot and dashboard payload so split suites can keep using
 * the production-shaped backend harness instead of bespoke mocks.
 *
 * @returns A seeded app snapshot and dashboard snapshot.
 */
export async function seedInitializedSnapshot() {
  await backend.initializeArchive(config, 'vault-passphrase')
  const snapshot = await backend.getAppSnapshot()
  const dashboard = await backend.loadDashboardSnapshot()
  return { snapshot, dashboard }
}

/**
 * Replays the shared per-test reset contract for trust-flow suites, while leaving
 * top-level module mocking in the suite that owns those boundaries.
 *
 * @param mocks The hoisted mocked modules for Tauri core and import-progress wiring.
 * @returns Nothing. The function resets shared test state as a side effect.
 */
export function resetTrustFlowHarness(mocks: TrustFlowMocks) {
  vi.restoreAllMocks()
  mocks.isTauri.mockReturnValue(false)
  mocks.invoke.mockReset()
  backendTestHarness.reset()
  mocks.subscribeToImportProgress.mockResolvedValue(vi.fn())
}
