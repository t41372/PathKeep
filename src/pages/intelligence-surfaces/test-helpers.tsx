/**
 * @file test-helpers.tsx
 * @description Shared test harness for Intelligence route and adjacent surface suites.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Provide one canonical render harness for Intelligence-adjacent route tests.
 * - Seed realistic archive state for route suites without each file rebuilding the same boilerplate.
 * - Centralize section-envelope helpers and the default beforeEach reset contract.
 *
 * ## Non-Responsibilities
 * - Does not own surface-specific assertions or mock payloads.
 * - Does not define local-host preview fixtures; those live in a dedicated helper file.
 * - Does not wrap unrelated page tests outside the Intelligence surface family.
 *
 * ## Dependencies
 * - Depends on the shipped app providers used by route surfaces: i18n, shell data, and profile scope.
 * - Uses `backendTestHarness` plus `backend` to seed deterministic fixture state.
 * - Uses `core-intelligence/api` to install the default overview-loader fallback mocks.
 *
 * ## Performance Notes
 * - Reuses the same seeded archive path instead of rebuilding ad hoc mock trees in every suite.
 * - Keeps test setup bounded so splitting the mega-suite does not multiply avoidable work.
 */

import type { ReactNode } from 'react'
import { render } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { vi } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import { defaultExplorerBackgroundPrefetchPages } from '../../lib/explorer-preferences'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
} from '../../lib/core-intelligence/types'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from '../../lib/i18n'
import { I18nContext, type I18nContextValue } from '../../lib/i18n/context'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import type {
  AppConfig,
  AppSnapshot,
  DashboardSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'

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
    assistantSystemPrompt:
      'You are an audit-first history research assistant. Use the available browser history evidence before answering.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

/**
 * Recreates the default suite reset so split test files all start from the same
 * deterministic surface truth.
 *
 * @returns Nothing. The function installs the shared mocks as a side effect.
 */
export function resetIntelligenceSurfaceHarness() {
  vi.restoreAllMocks()
  backendTestHarness.reset()
  window.localStorage.clear()
  vi.spyOn(
    coreIntelligenceApi,
    'loadIntelligencePrimaryOverview',
  ).mockRejectedValue(new Error('overview batching unavailable in test'))
  vi.spyOn(
    coreIntelligenceApi,
    'loadIntelligenceSecondaryOverview',
  ).mockRejectedValue(new Error('overview batching unavailable in test'))
}

/**
 * Creates the i18n context used by route suites so every split file speaks the
 * same translation contract as the shipped app.
 *
 * @param language The resolved locale the test wants to render.
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
 * Creates the default shell-data context for route suites, including a shared
 * runtime snapshot shape that individual tests can override.
 *
 * @param snapshot The app snapshot currently under test.
 * @param dashboard Optional dashboard snapshot for pages that expect one.
 * @returns A shell-data context value with stubbed mutations and refresh hooks.
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
    dashboardLoading: false,
    runtimeStatus: {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: createEmptyRuntimeSnapshot(),
      loading: false,
      error: null,
    },
    loading: false,
    busyAction: null,
    busyOverlay: null,
    error: null,
    notice: null,
    refreshKey: 1,
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn().mockResolvedValue({
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        indexQueued: 0,
        indexRunning: 0,
        recentJobs: [],
      },
      intelligence: createEmptyRuntimeSnapshot(),
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
    setAppLockPasscode: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    clearAppLockPasscode: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    lockAppSession: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    unlockAppSession: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    clearNotice: vi.fn(),
  }
}

/**
 * Provides the canonical empty runtime snapshot used across jobs, assistant,
 * settings, and intelligence tests.
 *
 * @returns A zeroed runtime snapshot with no queued modules or notes.
 */
export function createEmptyRuntimeSnapshot(): IntelligenceRuntimeSnapshot {
  return {
    queue: {
      queued: 0,
      running: 0,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      lastActivityAt: null,
    },
    plugins: [],
    modules: [],
    recentJobs: [],
    notes: [],
  }
}

/**
 * Builds section metadata wrappers so tests can talk in the same section-envelope
 * contract as the production intelligence route.
 *
 * @param sectionId The stable section id under test.
 * @param overrides Optional metadata overrides for degraded, stale, or custom windows.
 * @returns A section metadata envelope.
 */
export function createSectionMeta(
  sectionId: string,
  overrides: Partial<CoreIntelligenceSectionMeta> = {},
): CoreIntelligenceSectionMeta {
  return {
    sectionId,
    generatedAt: '2026-04-17T09:45:00Z',
    window: {
      kind: 'date-range',
      dateRange: { start: '2026-03-17', end: '2026-04-17' } satisfies DateRange,
    },
    moduleIds: [],
    sourceTables: [],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: [],
    ...overrides,
  }
}

/**
 * Wraps arbitrary section data inside the same result envelope returned by the
 * Core Intelligence API layer.
 *
 * @param sectionId The stable section id under test.
 * @param data The typed section payload.
 * @param overrides Optional metadata overrides applied through `createSectionMeta`.
 * @returns A section result envelope that route surfaces can consume directly.
 */
export function wrapSection<T>(
  sectionId: string,
  data: T,
  overrides: Partial<CoreIntelligenceSectionMeta> = {},
): CoreIntelligenceSectionResult<T> {
  return {
    data,
    meta: createSectionMeta(sectionId, overrides),
  }
}

/**
 * Renders one Intelligence-adjacent route surface inside the canonical provider
 * stack used by this mega-suite family.
 *
 * @param ui The surface under test.
 * @param options The route, locale, snapshot, and optional shell/dashboard overrides.
 * @returns The Testing Library render result for follow-up assertions.
 */
export function renderSurface(
  ui: ReactNode,
  {
    dashboard = null,
    language = 'en' as ResolvedLanguage,
    route = '/',
    shellValue,
    snapshot,
  }: {
    dashboard?: DashboardSnapshot | null
    language?: ResolvedLanguage
    route?: string
    shellValue?: ShellDataContextValue
    snapshot: AppSnapshot
  },
) {
  window.location.hash = ''
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
 * Seeds a realistic initialized archive and dashboard snapshot so split suites
 * can assert against shipped route behavior instead of synthetic mocks only.
 *
 * @returns A cloned app snapshot plus dashboard snapshot ready for mutation in one test.
 */
export async function seedArchiveState() {
  await backend.initializeArchive(baseConfig, 'vault-passphrase')
  await backend.runBackupNow(false)

  const snapshot = structuredClone(await backend.getAppSnapshot())
  const dashboard = structuredClone(await backend.loadDashboardSnapshot())

  return { snapshot, dashboard }
}

/**
 * Turns on the local AI runtime inside a seeded snapshot so assistant and
 * intelligence UI tests can stay focused on front-end behavior.
 *
 * @param snapshot The seeded app snapshot to mutate for one test.
 * @returns Nothing. The snapshot is updated in place.
 */
export function enableAi(snapshot: AppSnapshot) {
  snapshot.config.ai = {
    ...snapshot.config.ai,
    enabled: true,
    assistantEnabled: true,
    semanticIndexEnabled: true,
    llmProviderId: 'llm-local',
    embeddingProviderId: 'embed-local',
    llmProviders: [
      {
        id: 'llm-local',
        name: 'Local LLM',
        purpose: 'llm',
        requestFormat: 'openai',
        enabled: true,
        baseUrl: 'http://localhost:11434',
        apiKeySaved: false,
        defaultModel: 'qwen3:8b',
        modelCatalog: [],
        temperature: 0.2,
        maxTokens: 1200,
        dimensions: null,
        notes: null,
      },
    ],
    embeddingProviders: [
      {
        id: 'embed-local',
        name: 'Local Embedding',
        purpose: 'embedding',
        requestFormat: 'openai',
        enabled: true,
        baseUrl: 'http://localhost:11434',
        apiKeySaved: false,
        defaultModel: 'nomic-embed-text',
        modelCatalog: [],
        temperature: null,
        maxTokens: null,
        dimensions: 768,
        notes: null,
      },
    ],
  }
  snapshot.aiStatus = {
    ...snapshot.aiStatus,
    enabled: true,
    assistantEnabled: true,
    ready: true,
    state: 'ready',
    indexedItems: 128,
    llmProviderId: 'llm-local',
    embeddingProviderId: 'embed-local',
    queuedJobs: 1,
    runningJobs: 1,
  }
}
