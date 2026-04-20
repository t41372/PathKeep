/**
 * This test file protects the shipped behavior of the Intelligence Surfaces.test.tsx front-end surface.
 *
 * Why this file exists:
 * - These assertions keep route-level trust, loading, and degraded-state promises from quietly regressing.
 * - If a design or product contract changes, the corresponding test should move with it instead of letting the route drift.
 *
 * Main declarations:
 * - `createI18nValue`
 * - `createShellValue`
 * - `renderSurface`
 * - `ScopeSwitcher`
 * - `seedArchiveState`
 * - `enableAi`
 *
 * Source-of-truth notes:
 * - Route behavior is defined jointly by `docs/design/screens-and-nav.md`, `docs/design/ux-principles.md`, and the relevant feature docs.
 * - Tests should verify real user-facing promises such as deep links, scoped callouts, loading grammar, and repair entry points.
 */

import { type ReactNode, useState } from 'react'
import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../app/shell-data-context'
import { backend } from '../lib/backend-client'
import { backendTestHarness } from '../lib/backend'
import { I18nContext, type I18nContextValue } from '../lib/i18n/context'
import {
  createNamespaceTranslator,
  createTranslator,
  type ResolvedLanguage,
} from '../lib/i18n'
import * as coreIntelligenceApi from '../lib/core-intelligence/api'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DayInsights,
  DateRange,
  IntelligenceLocalHostBundle,
  IntelligenceLocalHostBuildResult,
  IntelligenceLocalHostPreview,
  QueryFamilyDetail,
  RefindPageDetail,
} from '../lib/core-intelligence/types'
import { ProfileScopeProvider } from '../lib/profile-scope'
import { ProfileScopeContext } from '../lib/profile-scope-context'
import type {
  AiProviderConnectionTestReport,
  AiQueueStatus,
  AppConfig,
  AppSnapshot,
  DashboardSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../lib/types'
import { AssistantPage } from './assistant'
import { DashboardPage } from './dashboard'
import { ExplorerPage } from './explorer'
import {
  CompareSetInsightsRoutePage,
  DayInsightsRoutePage,
  DomainDeepDiveRoutePage,
  IntelligencePage,
  QueryFamilyInsightsRoutePage,
  RefindPageInsightsRoutePage,
  SessionInsightsRoutePage,
  TrailInsightsRoutePage,
} from './intelligence'
import { JobsPage } from './jobs'
import { SettingsPage } from './settings'

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
    assistantSystemPrompt:
      'You are an audit-first history research assistant. Use the available browser history evidence before answering.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

/**
 * Creates i18n value.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function createI18nValue(language: ResolvedLanguage): I18nContextValue {
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
 * Creates shell value.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function createShellValue(
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
    saveConfig: vi.fn().mockResolvedValue(snapshot),
    initializeArchive: vi.fn().mockResolvedValue(snapshot),
    runBackup: vi.fn().mockResolvedValue({
      dueSkipped: false,
      run: null,
      profiles: [],
      warnings: [],
      remoteBackup: null,
    }),
    setAppLockPasscode: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    clearAppLockPasscode: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    lockAppSession: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    unlockAppSession: vi.fn().mockResolvedValue(snapshot.appLockStatus),
    clearNotice: vi.fn(),
  }
}

function createEmptyRuntimeSnapshot(): IntelligenceRuntimeSnapshot {
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

function createSectionMeta(
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

function wrapSection<T>(
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
 * Explains how render surface works.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function renderSurface(
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
 * Explains how seed archive state works.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
async function seedArchiveState() {
  await backend.initializeArchive(baseConfig, 'vault-passphrase')
  await backend.runBackupNow(false)

  const snapshot = structuredClone(await backend.getAppSnapshot())
  const dashboard = structuredClone(await backend.loadDashboardSnapshot())

  return { snapshot, dashboard }
}

/**
 * Explains how enable ai works.
 *
 * Keeping this as a named declaration makes the Intelligence Surfaces.test.tsx surface easier to review and test than burying the behavior inside another anonymous callback.
 */
function enableAi(snapshot: AppSnapshot) {
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

function createLocalHostPreview(
  locale: string,
  profileId: string | null = 'chrome:Default',
): IntelligenceLocalHostPreview {
  const bundle: IntelligenceLocalHostBundle = {
    bundleVersion: 'pathkeep.core-intelligence.local-host.v1',
    hostId: 'browser-snippet-v1',
    generatedAt: '2026-04-18T10:15:00Z',
    locale,
    dateRange: { start: '2026-03-17', end: '2026-04-17' },
    profileId,
    embedCards: [
      {
        cardId: 'digest:visits',
        cardType: 'digest',
        title: 'Visits',
        eyebrow: '2026-03-17 → 2026-04-17',
        body: 'Preview fixture for the trusted local snippet host.',
        metricLabel: 'visit_count',
        metricValue: '128',
        href: null,
        internalOnly: false,
      },
      {
        cardId: 'refind:sqlite',
        cardType: 'refind_page',
        title: 'SQLite WAL guide',
        eyebrow: 'Refind',
        body: 'This page kept resurfacing across 4 days and 3 trails.',
        metricLabel: 'refind_score',
        metricValue: '0.82',
        href: 'https://sqlite.org/wal.html',
        internalOnly: true,
      },
    ],
    widgetSnapshot: {
      generatedAt: '2026-04-18T10:15:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: {
          value: 128,
          trend: 'up',
          previousValue: 120,
          changePercent: 7,
        },
        totalSearches: {
          value: 32,
          trend: 'up',
          previousValue: 28,
          changePercent: 14,
        },
        newDomains: {
          value: 9,
          trend: 'up',
          previousValue: 8,
          changePercent: 13,
        },
        deepReadPages: {
          value: 5,
          trend: 'up',
          previousValue: 4,
          changePercent: 25,
        },
        refindPages: {
          value: 3,
          trend: 'up',
          previousValue: 2,
          changePercent: 50,
        },
      },
      highlights: [
        {
          cardId: 'refind:sqlite',
          cardType: 'refind_page',
          title: 'SQLite WAL guide',
          eyebrow: 'Refind',
          body: 'This page kept resurfacing across 4 days and 3 trails.',
          metricLabel: 'refind_score',
          metricValue: '0.82',
          href: 'https://sqlite.org/wal.html',
          internalOnly: true,
        },
      ],
      notes: [
        'Widget snapshots only expose aggregate Core Intelligence read models.',
      ],
    },
    publicSnapshot: {
      generatedAt: '2026-04-18T10:15:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: {
          value: 128,
          trend: 'up',
          previousValue: 120,
          changePercent: 7,
        },
        totalSearches: {
          value: 32,
          trend: 'up',
          previousValue: 28,
          changePercent: 14,
        },
        newDomains: {
          value: 9,
          trend: 'up',
          previousValue: 8,
          changePercent: 13,
        },
        deepReadPages: {
          value: 5,
          trend: 'up',
          previousValue: 4,
          changePercent: 25,
        },
        refindPages: {
          value: 3,
          trend: 'up',
          previousValue: 2,
          changePercent: 50,
        },
      },
      topDomains: ['sqlite.org', 'github.com'],
      searchEngines: [
        { searchEngine: 'google', displayName: 'Google', searchCount: 18 },
      ],
      discoveryTrend: {
        availableYears: [],
        points: [
          {
            dateKey: '2026-04-07',
            discoveryRate: 0.35,
            newDomainCount: 4,
            totalVisits: 22,
          },
        ],
      },
      notes: [
        'Public snapshots intentionally omit visit-level identifiers and direct page URLs.',
      ],
    },
    trustedOnlyCardIds: ['refind:sqlite'],
    trustedOnlyCardCount: 1,
    boundaryNotes: [
      'This local host only uses deterministic Core Intelligence read models.',
      'Trusted-only cards must stay inside PathKeep-controlled local surfaces.',
    ],
  }

  return {
    artifactRoot:
      '/Users/tim/Library/Application Support/PathKeep/integrations/core-intelligence/browser-snippet-v1',
    entryFilePath:
      '/Users/tim/Library/Application Support/PathKeep/integrations/core-intelligence/browser-snippet-v1/index.html',
    generatedFiles: [
      {
        relativePath:
          'integrations/core-intelligence/browser-snippet-v1/index.html',
        absolutePath:
          '/Users/tim/Library/Application Support/PathKeep/integrations/core-intelligence/browser-snippet-v1/index.html',
        purpose:
          'Core Intelligence snippet that can be opened directly in a local browser.',
        contents:
          '<!doctype html><title>PathKeep Core Intelligence Snippet</title>',
      },
      {
        relativePath:
          'integrations/core-intelligence/browser-snippet-v1/bundle.json',
        absolutePath:
          '/Users/tim/Library/Application Support/PathKeep/integrations/core-intelligence/browser-snippet-v1/bundle.json',
        purpose:
          'Machine-readable JSON bundle for the same local host artifact.',
        contents: JSON.stringify(bundle, null, 2),
      },
    ],
    bundle,
    boundaryNotes: bundle.boundaryNotes,
    manualSteps: [
      'Review index.html and bundle.json before handing this folder to another trusted local tool.',
      'Open index.html from this folder inside a trusted local browser surface.',
    ],
    warnings: [
      'This local snippet includes trusted-only cards and should not be treated like a public export.',
    ],
    installedHost: null,
  }
}

describe('intelligence surfaces', () => {
  beforeEach(() => {
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
  })

  test('renders localized dashboard intelligence and trust callouts', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('zh-TW', 'dashboard')
    const commonT = createNamespaceTranslator('zh-TW', 'common')

    enableAi(snapshot)
    snapshot.config.rememberDatabaseKeyInKeyring = true
    snapshot.keyringStatus.storedSecret = false
    snapshot.browserProfiles.push({
      profileId: 'safari:Personal',
      profileName: 'Personal',
      browserFamily: 'safari',
      browserName: 'Safari',
      userName: 'tim',
      profilePath: '/Users/tim/Library/Safari',
      historyPath: '/Users/tim/Library/Safari/History.db',
      faviconsPath: null,
      historyExists: false,
      browserVersion: null,
      historyFileName: 'History.db',
      historyBytes: 18 * 1024 * 1024,
      faviconsBytes: 0,
      supportingBytes: 2 * 1024 * 1024,
      retentionBoundary: {
        kind: 'macos-safari',
        localDays: 365,
      },
    })
    snapshot.config.selectedProfileIds.push('safari:Personal')

    renderSurface(<DashboardPage />, {
      dashboard,
      language: 'zh-TW',
      route: '/',
      snapshot,
    })

    expect(
      await screen.findByText(dashboardT('intelligenceTitle')),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('semanticSearchAction') }),
    ).toBeVisible()
    expect(
      screen.getAllByRole('link', { name: dashboardT('reviewInsightsAction') }),
    ).toHaveLength(2)
    expect(
      screen.getAllByRole('link', { name: dashboardT('reviewSecurity') }),
    ).toHaveLength(2)
    expect(
      screen.getAllByRole('link', { name: dashboardT('reviewImportBatches') }),
    ).toHaveLength(2)
    expect(
      screen.getByText(commonT('browserRetentionManagedLabel')),
    ).toBeVisible()
    expect(
      screen.getAllByText((content) =>
        content.includes(commonT('browserRetentionArchiveBoundary')),
      ).length,
    ).toBeGreaterThan(0)
  })

  test('routes dashboard yearly browsing rhythm into day insights without inline detail fetches', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const getDiscoveryTrendSpy = vi
      .spyOn(coreIntelligenceApi, 'getDiscoveryTrend')
      .mockImplementation((dateRange) =>
        Promise.resolve(
          wrapSection('discovery-trend', {
            availableYears: [2025, 2024],
            points:
              dateRange.start === '2024-01-01'
                ? [
                    {
                      dateKey: '2024-04-18',
                      discoveryRate: 0.33,
                      newDomainCount: 1,
                      totalVisits: 3,
                    },
                  ]
                : [
                    {
                      dateKey: '2025-04-18',
                      discoveryRate: 0.25,
                      newDomainCount: 2,
                      totalVisits: 8,
                    },
                  ],
          }),
        ),
      )
    const getDigestSummarySpy = vi
      .spyOn(coreIntelligenceApi, 'getDigestSummary')
      .mockResolvedValue(
        wrapSection('digest-summary', {
          dateRange: {
            start: '2024-04-18',
            end: '2024-04-18',
          } satisfies DateRange,
          totalVisits: { value: 3, trend: 'flat' as const },
          totalSearches: { value: 1, trend: 'flat' as const },
          newDomains: { value: 1, trend: 'flat' as const },
          deepReadPages: { value: 2, trend: 'flat' as const },
          refindPages: { value: 0, trend: 'flat' as const },
        }),
      )
    const getTopSitesSpy = vi
      .spyOn(coreIntelligenceApi, 'getTopSites')
      .mockResolvedValue(
        wrapSection('top-sites', [
          {
            registrableDomain: 'sqlite.org',
            displayName: 'SQLite',
            domainCategory: 'docs',
            visitCount: 3,
            uniqueDays: 1,
            averageDailyVisits: 3,
            uniqueUrls: 2,
          },
        ]),
      )
    const getBrowsingRhythmSpy = vi
      .spyOn(coreIntelligenceApi, 'getBrowsingRhythm')
      .mockResolvedValue(
        wrapSection('browsing-rhythm', {
          cells: [{ dow: 4, hour: 10, visitCount: 3 }],
          maxCount: 3,
        }),
      )

    renderSurface(
      <Routes>
        <Route path="/" element={<DashboardPage />} />
        <Route
          path="/intelligence/day/:date"
          element={<div data-testid="day-insights-route-target" />}
        />
      </Routes>,
      {
        dashboard,
        route: '/',
        snapshot,
      },
    )

    const yearSelect = await screen.findByTestId('browsing-rhythm-year-select')
    expect(yearSelect).toHaveValue('2025')
    expect(getDigestSummarySpy).not.toHaveBeenCalled()
    expect(getTopSitesSpy).not.toHaveBeenCalled()
    expect(getBrowsingRhythmSpy).not.toHaveBeenCalled()

    await user.selectOptions(yearSelect, '2024')
    await waitFor(() =>
      expect(getDiscoveryTrendSpy).toHaveBeenLastCalledWith(
        { start: '2024-01-01', end: '2024-12-31' },
        null,
        'day',
      ),
    )

    await user.click(
      await screen.findByRole('button', {
        name: /2024-04-18 · 3 visits · 1 new sites/i,
      }),
    )

    expect(
      await screen.findByTestId('day-insights-route-target'),
    ).toBeInTheDocument()
    expect(getDigestSummarySpy).not.toHaveBeenCalled()
    expect(getTopSitesSpy).not.toHaveBeenCalled()
    expect(getBrowsingRhythmSpy).not.toHaveBeenCalled()
  })

  test('routes dashboard archive-key failures toward the security page', async () => {
    const { snapshot } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('en', 'dashboard')

    renderSurface(<DashboardPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        dashboard: null,
        snapshot: null,
        error: 'database key is required for encrypted archives',
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: dashboardT('archiveUnlockRequiredTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByText(dashboardT('archiveUnlockRequiredBody')),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('archiveUnlockAction') }),
    ).toHaveAttribute('href', '/security#unlock-archive')
  })

  test('recovers actionable unlock guidance when shell bootstrap only surfaced a generic dashboard error', async () => {
    const { snapshot } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('en', 'dashboard')
    vi.spyOn(backend, 'securityStatus').mockResolvedValue({
      initialized: true,
      mode: 'Encrypted',
      encrypted: true,
      unlocked: false,
      databasePath: snapshot.directories.archiveDatabasePath,
      strongholdPath: snapshot.directories.strongholdPath,
      rememberDatabaseKeyInKeyring: false,
      lastSuccessfulBackupAt: null,
      lastRekeyAt: null,
      lastRekeyRunId: null,
      lastRekeySnapshotPath: null,
      keyringStatus: {
        available: true,
        backend: 'macOS Keychain',
        storedSecret: false,
      },
      warnings: ['database key is required for encrypted archives'],
    })

    renderSurface(<DashboardPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        dashboard: null,
        snapshot: null,
        error: dashboardT('archiveUnavailableBody'),
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: dashboardT('archiveUnlockRequiredTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('archiveUnlockAction') }),
    ).toHaveAttribute('href', '/security#unlock-archive')
  })

  test('falls back to the onboarding zero-state when shell bootstrap failed before archive initialization', async () => {
    const { snapshot } = await seedArchiveState()
    const dashboardT = createNamespaceTranslator('en', 'dashboard')
    vi.spyOn(backend, 'securityStatus').mockResolvedValue({
      initialized: false,
      mode: 'Plaintext',
      encrypted: false,
      unlocked: false,
      databasePath: snapshot.directories.archiveDatabasePath,
      strongholdPath: snapshot.directories.strongholdPath,
      rememberDatabaseKeyInKeyring: false,
      lastSuccessfulBackupAt: null,
      lastRekeyAt: null,
      lastRekeyRunId: null,
      lastRekeySnapshotPath: null,
      keyringStatus: {
        available: true,
        backend: 'macOS Keychain',
        storedSecret: false,
      },
      warnings: [],
    })

    renderSurface(<DashboardPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        dashboard: null,
        snapshot: null,
        error: dashboardT('archiveUnavailableBody'),
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: dashboardT('zeroStateTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('openOnboardingFlow') }),
    ).toHaveAttribute('href', '/onboarding')
  })

  test('renders top-sites inside a scroll region so long lists do not stretch the section', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection(
        'top-sites',
        Array.from({ length: 20 }, (_, index) => ({
          registrableDomain: `example-${index + 1}.com`,
          displayName: `Example ${index + 1}`,
          domainCategory: 'reference',
          visitCount: 100 - index,
          uniqueDays: 20 - Math.floor(index / 2),
          averageDailyVisits: Number((5 - index * 0.1).toFixed(1)),
          uniqueUrls: 10 - Math.floor(index / 3),
        })),
        {
          moduleIds: ['daily-rollups'],
          sourceTables: ['domain_daily_rollups'],
        },
      ),
    )

    renderSurface(<IntelligencePage />, {
      route: '/intelligence?profileId=chrome:Default',
      snapshot,
    })

    expect(await screen.findByText('Example 1')).toBeVisible()

    const topSitesSection = screen
      .getByRole('heading', { name: intelligenceT('topSitesTitle') })
      .closest('section')
    expect(topSitesSection).not.toBeNull()
    if (!(topSitesSection instanceof HTMLElement)) {
      throw new Error('expected top sites section')
    }

    expect(
      topSitesSection.querySelector('.intelligence-section__scroll-region'),
    ).not.toBeNull()
  })

  test('renders section-level metadata for stale, disabled, and degraded intelligence sections', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const settingsT = createNamespaceTranslator('en', 'settings')

    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection('top-sites', [], {
        state: 'stale',
        stateReason: 'Visibility changed after the last deterministic rebuild.',
        moduleIds: ['daily-rollups'],
        sourceTables: ['domain_daily_rollups'],
        notes: [
          'Manual rebuild required before these summaries are fresh again.',
        ],
        window: {
          kind: 'date-range',
          dateRange: { start: '2026-04-01', end: '2026-04-07' },
        },
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
      wrapSection('stable-sources', [], {
        state: 'disabled',
        stateReason: 'Disabled in Settings.',
        moduleIds: ['refind-pages'],
        sourceTables: ['source_effectiveness'],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
      wrapSection('observed-interactions', [], {
        state: 'degraded',
        stateReason:
          'No supported browser-reported interaction evidence is available for this scope yet.',
        sourceTables: ['visit_engagement_evidence'],
      }),
    )

    renderSurface(<IntelligencePage />, {
      route:
        '/intelligence?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default',
      snapshot,
    })

    expect(
      await screen.findByTestId('intelligence-section-meta-top-sites'),
    ).toBeVisible()

    const topSitesSection = screen
      .getByRole('heading', { name: intelligenceT('topSitesTitle') })
      .closest('section')
    if (!(topSitesSection instanceof HTMLElement)) {
      throw new Error('expected top sites section')
    }
    const topSitesMeta = within(topSitesSection).getByTestId(
      'intelligence-section-meta-top-sites',
    )
    await user.click(
      within(topSitesMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(topSitesMeta).getAllByText(settingsT('deterministicModuleStale'))
        .length,
    ).toBeGreaterThan(0)
    expect(within(topSitesMeta).getByText('domain_daily_rollups')).toBeVisible()
    expect(
      within(topSitesMeta).getByText(
        'Visibility changed after the last deterministic rebuild.',
      ),
    ).toBeVisible()

    const stableSourcesSection = screen
      .getByRole('heading', { name: intelligenceT('stableSourcesTitle') })
      .closest('section')
    if (!(stableSourcesSection instanceof HTMLElement)) {
      throw new Error('expected stable sources section')
    }
    const stableSourcesMeta = within(stableSourcesSection).getByTestId(
      'intelligence-section-meta-stable-sources',
    )
    await user.click(
      within(stableSourcesMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(stableSourcesMeta).getAllByText(
        settingsT('deterministicModuleDisabled'),
      ).length,
    ).toBeGreaterThan(0)

    const observedSection = screen
      .getByRole('heading', { name: intelligenceT('observedTitle') })
      .closest('section')
    if (!(observedSection instanceof HTMLElement)) {
      throw new Error('expected observed interactions section')
    }
    const observedMeta = within(observedSection).getByTestId(
      'intelligence-section-meta-observed-interactions',
    )
    await user.click(
      within(observedMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(observedMeta).getAllByText(
        intelligenceT('sectionMetaStateDegraded'),
      ).length,
    ).toBeGreaterThan(0)
    expect(
      within(observedMeta).getByText(
        'No supported browser-reported interaction evidence is available for this scope yet.',
      ),
    ).toBeVisible()
  })

  test('renders compact evidence badges on overview and day-insights surfaces', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const settingsT = createNamespaceTranslator('en', 'settings')

    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection(
        'top-sites',
        [
          {
            registrableDomain: 'sqlite.org',
            displayName: 'SQLite',
            domainCategory: 'docs',
            visitCount: 3,
            uniqueDays: 2,
            averageDailyVisits: 1.5,
            uniqueUrls: 2,
          },
        ],
        {
          moduleIds: ['daily-rollups'],
          sourceTables: ['domain_daily_rollups'],
        },
      ),
    )

    const overview = renderSurface(<IntelligencePage />, {
      route: '/intelligence?profileId=chrome:Default',
      snapshot,
    })

    const overviewMeta = await screen.findByTestId(
      'intelligence-section-meta-top-sites',
    )
    expect(
      within(overviewMeta).getByText(intelligenceT('sectionMetaTitle')),
    ).toBeVisible()
    expect(
      within(overviewMeta).getByText(settingsT('deterministicModuleReady')),
    ).toBeVisible()

    await user.click(
      within(overviewMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(overviewMeta).getByText(intelligenceT('sectionMetaGeneratedAt')),
    ).toBeVisible()

    overview.unmount()

    vi.spyOn(coreIntelligenceApi, 'getDayInsights').mockResolvedValue(
      wrapSection<DayInsights>(
        'day-insights',
        {
          date: '2026-04-18',
          digestSummary: {
            dateRange: { start: '2026-04-18', end: '2026-04-18' },
            totalVisits: { value: 8, trend: 'flat' },
            totalSearches: { value: 3, trend: 'flat' },
            newDomains: { value: 2, trend: 'flat' },
            deepReadPages: { value: 4, trend: 'flat' },
            refindPages: { value: 1, trend: 'flat' },
          },
          topSites: [],
          activityMix: {
            categories: [{ domainCategory: 'docs', visitCount: 8, share: 1 }],
            changeVsPrevious: [],
          },
          refindPages: [],
          queryFamilies: {
            families: [],
            total: 0,
            page: 0,
            pageSize: 8,
          },
          hourlyActivity: Array.from({ length: 24 }, (_, hour) => ({
            hour,
            visitCount: hour === 10 ? 4 : 0,
          })),
          drilldown: {
            explorerDateRange: { start: '2026-04-18', end: '2026-04-18' },
          },
        },
        {
          moduleIds: ['daily-rollups', 'activity-mix'],
          sourceTables: ['daily_summary_rollups', 'category_daily_rollups'],
        },
      ),
    )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/day/2026-04-18?profileId=chrome:Default',
        snapshot,
      },
    )

    const dayMeta = await screen.findByTestId(
      'intelligence-section-meta-day-insights',
    )
    expect(
      within(dayMeta).getByText(intelligenceT('sectionMetaTitle')),
    ).toBeVisible()
    expect(
      within(dayMeta).getByText(settingsT('deterministicModuleReady')),
    ).toBeVisible()
  })

  test('refreshes section metadata when intelligence scope or time range changes', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    const topSitesSpy = vi
      .spyOn(coreIntelligenceApi, 'getTopSites')
      .mockImplementation((dateRange, profileId) =>
        Promise.resolve(
          wrapSection('top-sites', [], {
            state: 'stale',
            stateReason: 'Scope-sensitive test metadata.',
            moduleIds: ['daily-rollups'],
            sourceTables: ['domain_daily_rollups'],
            window: {
              kind: 'date-range',
              dateRange,
            },
            notes: [profileId ?? 'archive-wide'],
          }),
        ),
      )

    function ScopedIntelligenceHarness() {
      const [activeProfileId, setActiveProfileId] = useState<string | null>(
        'chrome:Default',
      )

      return (
        <MemoryRouter
          initialEntries={[
            '/intelligence?range=custom&start=2026-04-01&end=2026-04-07',
          ]}
        >
          <I18nContext.Provider value={createI18nValue('en')}>
            <ProfileScopeContext.Provider
              value={{ activeProfileId, setActiveProfileId }}
            >
              <ShellDataContext.Provider value={createShellValue(snapshot)}>
                <button
                  type="button"
                  onClick={() => setActiveProfileId('firefox:Research')}
                >
                  Switch profile
                </button>
                <Routes>
                  <Route path="/intelligence" element={<IntelligencePage />} />
                </Routes>
              </ShellDataContext.Provider>
            </ProfileScopeContext.Provider>
          </I18nContext.Provider>
        </MemoryRouter>
      )
    }

    render(<ScopedIntelligenceHarness />)

    const topSitesMeta = await screen.findByTestId(
      'intelligence-section-meta-top-sites',
    )
    await user.click(
      within(topSitesMeta).getByRole('button', {
        name: intelligenceT('sectionMetaOpenPanelAria'),
      }),
    )
    expect(
      within(topSitesMeta).getByText('2026-04-01 → 2026-04-07'),
    ).toBeVisible()
    expect(within(topSitesMeta).getByText('chrome:Default')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Switch profile' }))
    await waitFor(() => {
      expect(
        topSitesSpy.mock.calls.some(
          (call) =>
            call[0]?.start === '2026-04-01' &&
            call[0]?.end === '2026-04-07' &&
            call[1] === 'firefox:Research',
        ),
      ).toBe(true)
    })
    expect(
      await screen.findByText(/Core Intelligence is only reading Research/i),
    ).toBeVisible()

    const previousTopSitesCallCount = topSitesSpy.mock.calls.length

    await user.click(
      screen.getByRole('button', { name: intelligenceT('rangeWeek') }),
    )
    await waitFor(() => {
      expect(topSitesSpy.mock.calls.length).toBeGreaterThan(
        previousTopSitesCallCount,
      )
      const matchingWeekCalls = topSitesSpy.mock.calls.filter(
        (call) =>
          call[1] === 'firefox:Research' &&
          (call[0]?.start !== '2026-04-01' || call[0]?.end !== '2026-04-07'),
      )
      expect(matchingWeekCalls.length).toBeGreaterThan(0)
    })
    expect(
      screen.queryByText('2026-04-01 → 2026-04-07'),
    ).not.toBeInTheDocument()
  })

  test('shows a security recovery empty state in settings when the archive needs unlocking', async () => {
    const { snapshot } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const dashboardT = createNamespaceTranslator('en', 'dashboard')

    await backend.clearSessionDatabaseKey()

    renderSurface(<SettingsPage />, {
      snapshot,
      shellValue: {
        ...createShellValue(snapshot),
        dashboard: null,
        snapshot: null,
        error: 'database key is required for encrypted archives',
      },
    })

    expect(
      await screen.findByRole('heading', {
        name: settingsT('archiveUnlockTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: dashboardT('reviewSecurity') }),
    ).toHaveAttribute('href', '/security')
  })

  test('renders settings enrichment runtime review and syncs plugin toggles', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const runtimeSnapshot: IntelligenceRuntimeSnapshot = {
      queue: {
        queued: 1,
        running: 0,
        succeeded: 1,
        failed: 1,
        cancelled: 0,
        lastActivityAt: '2026-04-10T16:30:00Z',
      },
      plugins: [
        {
          pluginId: 'title-normalization',
          sourceKind: 'local',
          enabled: true,
          storedRecords: 42,
          queuedJobs: 0,
          runningJobs: 0,
          failedJobs: 0,
          lastCompletedAt: '2026-04-10T16:20:00Z',
          lastError: null,
        },
        {
          pluginId: 'readable-content-refetch',
          sourceKind: 'network',
          enabled: true,
          storedRecords: 8,
          queuedJobs: 1,
          runningJobs: 0,
          failedJobs: 1,
          lastCompletedAt: '2026-04-10T15:40:00Z',
          lastError: '429 from upstream host',
        },
      ],
      modules: [
        {
          moduleId: 'search-trails',
          enabled: true,
          version: 'ci-v1',
          status: 'ready',
          dependsOn: ['visit-derived-facts', 'sessions'],
          derivedTables: [
            'search_trails',
            'search_trail_members',
            'search_events',
            'search_event_terms',
            'query_families',
          ],
          lastRunId: 12,
          lastBuiltAt: '2026-04-10T16:25:00Z',
          lastInvalidatedAt: null,
          staleReason: null,
          notes: [
            'Search trails and query families reflect the latest normalized visits.',
          ],
        },
        {
          moduleId: 'refind-pages',
          enabled: true,
          version: 'ci-v1',
          status: 'stale',
          dependsOn: ['visit-derived-facts', 'search-trails'],
          derivedTables: ['refind_pages', 'source_effectiveness'],
          lastRunId: 11,
          lastBuiltAt: '2026-04-09T16:25:00Z',
          lastInvalidatedAt: '2026-04-10T16:28:00Z',
          staleReason:
            'Visibility changed after the last deterministic rebuild.',
          notes: [
            'Manual rebuild required before refind pages and source effectiveness are fresh again.',
          ],
        },
      ],
      recentJobs: [
        {
          id: 411,
          jobType: 'enrichment-plugin',
          pluginId: 'readable-content-refetch',
          state: 'failed',
          historyId: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Article',
          attempt: 2,
          createdAt: '2026-04-10T15:35:00Z',
          startedAt: '2026-04-10T15:36:00Z',
          finishedAt: '2026-04-10T15:37:00Z',
          updatedAt: '2026-04-10T15:37:00Z',
          heartbeatAt: null,
          progressLabel: null,
          progressDetail: null,
          progressCurrent: null,
          progressTotal: null,
          progressPercent: null,
          lastError: '429 from upstream host',
          retryable: true,
          cancellable: false,
        },
      ],
      notes: [
        'Browser preview mode shows a deterministic queue/runtime fixture.',
      ],
    }

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      runtimeSnapshot,
    )
    const shellValue = createShellValue(snapshot, dashboard)
    shellValue.saveConfig = vi.fn().mockResolvedValue(snapshot)

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'en',
      route: '/settings',
      shellValue,
      snapshot,
    })

    expect(
      await screen.findByText(settingsT('firstPartyRuntimeTitle')),
    ).toBeVisible()
    expect(screen.getByText('Title normalization')).toBeVisible()
    expect(screen.getByText('Search trails')).toBeVisible()
    expect(screen.getByText('Refind pages')).toBeVisible()
    expect(screen.getAllByText('Page content fetcher').length).toBeGreaterThan(
      0,
    )
    expect(
      screen.getAllByText('1 queued / 0 running / 1 failed').length,
    ).toBeGreaterThan(0)

    const titleNormalizationRow = screen
      .getByText('Title normalization')
      .closest('.result-row')
    expect(titleNormalizationRow).not.toBeNull()
    if (!(titleNormalizationRow instanceof HTMLElement)) {
      throw new Error('expected title normalization row')
    }
    await user.click(
      within(titleNormalizationRow).getByRole('button', {
        name: settingsT('disablePlugin'),
      }),
    )

    await waitFor(() => expect(shellValue.saveConfig).toHaveBeenCalledTimes(1))
    expect(shellValue.saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        enrichment: {
          plugins: expect.arrayContaining([
            expect.objectContaining({
              id: 'title-normalization',
              enabled: false,
            }),
          ]),
        },
        ai: expect.objectContaining({
          enrichmentPlugins: expect.arrayContaining([
            expect.objectContaining({
              pluginId: 'title-normalization',
              enabled: false,
            }),
          ]),
        }),
      }),
    )
  })

  test('renders settings manual external outputs review and lets the user switch surfaces', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const commonT = createNamespaceTranslator('en', 'common')

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    const embedSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligenceEmbedCards')
      .mockResolvedValue([
        {
          cardId: 'digest:visits',
          cardType: 'digest',
          title: 'Visits',
          eyebrow: '2026-03-17 → 2026-04-17',
          body: 'Total visits in the selected intelligence window.',
          metricLabel: 'visit_count',
          metricValue: '128',
          href: null,
          internalOnly: false,
        },
        {
          cardId: 'refind:sqlite',
          cardType: 'refind_page',
          title: 'SQLite WAL guide',
          eyebrow: 'Refind',
          body: 'This page kept resurfacing across 4 days and 3 trails.',
          metricLabel: 'refind_score',
          metricValue: '0.82',
          href: 'https://sqlite.org/wal.html',
          internalOnly: true,
        },
      ])
    const widgetSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligenceWidgetSnapshot')
      .mockResolvedValue({
        generatedAt: '2026-04-17T09:45:00Z',
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        digestSummary: {
          dateRange: { start: '2026-03-17', end: '2026-04-17' },
          totalVisits: {
            value: 128,
            previousValue: 120,
            changePercent: 7,
            trend: 'up',
          },
          totalSearches: {
            value: 32,
            previousValue: 28,
            changePercent: 14,
            trend: 'up',
          },
          newDomains: {
            value: 9,
            previousValue: 8,
            changePercent: 13,
            trend: 'up',
          },
          deepReadPages: {
            value: 5,
            previousValue: 4,
            changePercent: 25,
            trend: 'up',
          },
          refindPages: {
            value: 3,
            previousValue: 2,
            changePercent: 50,
            trend: 'up',
          },
        },
        highlights: [
          {
            cardId: 'refind:sqlite',
            cardType: 'refind_page',
            title: 'SQLite WAL guide',
            eyebrow: 'Refind',
            body: 'This page kept resurfacing across 4 days and 3 trails.',
            metricLabel: 'refind_score',
            metricValue: '0.82',
            href: 'https://sqlite.org/wal.html',
            internalOnly: true,
          },
        ],
        notes: [
          'Widget snapshots only expose aggregate Core Intelligence read models.',
        ],
      })
    const publicSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligencePublicSnapshot')
      .mockResolvedValue({
        generatedAt: '2026-04-17T09:45:00Z',
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        digestSummary: {
          dateRange: { start: '2026-03-17', end: '2026-04-17' },
          totalVisits: {
            value: 128,
            previousValue: 120,
            changePercent: 7,
            trend: 'up',
          },
          totalSearches: {
            value: 32,
            previousValue: 28,
            changePercent: 14,
            trend: 'up',
          },
          newDomains: {
            value: 9,
            previousValue: 8,
            changePercent: 13,
            trend: 'up',
          },
          deepReadPages: {
            value: 5,
            previousValue: 4,
            changePercent: 25,
            trend: 'up',
          },
          refindPages: {
            value: 3,
            previousValue: 2,
            changePercent: 50,
            trend: 'up',
          },
        },
        topDomains: ['sqlite.org', 'github.com'],
        searchEngines: [
          {
            searchEngine: 'google',
            displayName: 'Google',
            searchCount: 18,
          },
        ],
        discoveryTrend: {
          availableYears: [],
          points: [
            {
              dateKey: '2026-04-07',
              discoveryRate: 0.35,
              newDomainCount: 4,
              totalVisits: 22,
            },
            {
              dateKey: '2026-04-14',
              discoveryRate: 0.41,
              newDomainCount: 5,
              totalVisits: 24,
            },
          ],
        },
        notes: [
          'Public snapshots intentionally omit visit-level identifiers and direct page URLs.',
        ],
      })
    const localHostPreviewSpy = vi
      .spyOn(coreIntelligenceApi, 'previewIntelligenceLocalHost')
      .mockResolvedValue(createLocalHostPreview('en'))

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'en',
      route: '/settings',
      snapshot,
    })

    const panel = await screen.findByTestId('settings-external-outputs')
    await waitFor(() => {
      expect(embedSpy).toHaveBeenCalledTimes(1)
      expect(widgetSpy).toHaveBeenCalledTimes(1)
      expect(publicSpy).toHaveBeenCalledTimes(1)
      expect(localHostPreviewSpy).toHaveBeenCalledTimes(1)
    })
    expect(
      within(panel).getByText(settingsT('externalOutputsSummaryTitle')),
    ).toBeVisible()
    expect(within(panel).getByText('SQLite WAL guide')).toBeVisible()
    expect(
      within(panel).getByText(settingsT('externalOutputsTrustedOnlyBadge')),
    ).toBeVisible()

    await user.click(
      within(panel).getByRole('tab', {
        name: settingsT('externalOutputsTabWidget'),
      }),
    )
    expect(
      await within(panel).findByText(
        settingsT('externalOutputsWidgetTrustedTitle'),
      ),
    ).toBeVisible()
    await user.click(
      within(panel).getAllByRole('button', { name: commonT('copyAction') })[0],
    )
    expect(
      await within(panel).findByText(commonT('copiedNotice')),
    ).toBeVisible()

    await user.click(
      within(panel).getByRole('tab', {
        name: settingsT('externalOutputsTabPublic'),
      }),
    )
    expect(
      await within(panel).findByText(
        settingsT('externalOutputsPublicRedactedTitle'),
      ),
    ).toBeVisible()
    expect(within(panel).getByText('sqlite.org')).toBeVisible()
    expect(
      within(panel).getByText(
        settingsT('externalOutputsLocalHostSummaryTitle'),
      ),
    ).toBeVisible()
    expect(
      within(panel).getByRole('button', {
        name: settingsT('externalOutputsLocalHostCreateAction'),
      }),
    ).toBeVisible()
  })

  test('renders settings search rules review and saves custom rules through the derived-state surface', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const queueSpy = vi
      .spyOn(coreIntelligenceApi, 'queueCoreIntelligenceRebuild')
      .mockResolvedValue({
        jobId: 77,
        state: 'queued',
        notes: ['Queued rebuild after search rule update.'],
      })

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'en',
      route: '/settings',
      snapshot,
    })

    expect(
      await screen.findByRole('heading', {
        name: settingsT('searchRulesTitle'),
      }),
    ).toBeVisible()
    expect(screen.getByText('Docs Search')).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: settingsT('searchRulesAdd') }),
    )
    const editor = await screen.findByRole('region', {
      name: settingsT('searchRulesEditorTitle'),
    })
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesDisplayName'),
      }),
      'MDN Search',
    )
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesEngineId'),
      }),
      'mdn-search',
    )
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesHostPattern'),
      }),
      'developer.mozilla.org',
    )
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesPathPrefix'),
      }),
      '/search',
    )
    await user.type(
      within(editor).getByRole('textbox', {
        name: settingsT('searchRulesQueryParam'),
      }),
      'q',
    )
    await user.click(
      within(editor).getByRole('button', {
        name: settingsT('searchRulesSave'),
      }),
    )

    await waitFor(() => expect(queueSpy).toHaveBeenCalledTimes(1))
    expect(await screen.findByText('MDN Search')).toBeVisible()

    const docsRuleRow = screen.getByText('Docs Search').closest('.result-row')
    expect(docsRuleRow).not.toBeNull()
    if (!(docsRuleRow instanceof HTMLElement)) {
      throw new Error('expected docs search rule row')
    }
    await user.click(
      within(docsRuleRow).getByRole('button', {
        name: settingsT('searchRulesDelete'),
      }),
    )

    await waitFor(() => expect(queueSpy).toHaveBeenCalledTimes(2))
    await waitFor(() => {
      expect(screen.queryByText('Docs Search')).not.toBeInTheDocument()
    })
  })

  test.each([
    {
      expectedTitleKey: 'externalOutputsUnlockTitle',
      mutate: (snapshot: AppSnapshot) => {
        snapshot.archiveStatus.unlocked = false
      },
    },
    {
      expectedTitleKey: 'externalOutputsNeedsArchiveTitle',
      mutate: (snapshot: AppSnapshot) => {
        snapshot.config.initialized = false
      },
    },
  ])(
    'keeps settings manual external outputs gated behind archive readiness truth ($expectedTitleKey)',
    async ({ expectedTitleKey, mutate }) => {
      const { snapshot, dashboard } = await seedArchiveState()
      const settingsT = createNamespaceTranslator('en', 'settings')
      mutate(snapshot)

      const embedSpy = vi.spyOn(
        coreIntelligenceApi,
        'getIntelligenceEmbedCards',
      )
      const widgetSpy = vi.spyOn(
        coreIntelligenceApi,
        'getIntelligenceWidgetSnapshot',
      )
      const publicSpy = vi.spyOn(
        coreIntelligenceApi,
        'getIntelligencePublicSnapshot',
      )
      const localHostPreviewSpy = vi.spyOn(
        coreIntelligenceApi,
        'previewIntelligenceLocalHost',
      )
      vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
        createEmptyRuntimeSnapshot(),
      )

      renderSurface(<SettingsPage />, {
        dashboard,
        language: 'en',
        route: '/settings',
        snapshot,
      })

      const panel = await screen.findByTestId('settings-external-outputs')
      expect(within(panel).getByText(settingsT(expectedTitleKey))).toBeVisible()
      expect(embedSpy).not.toHaveBeenCalled()
      expect(widgetSpy).not.toHaveBeenCalled()
      expect(publicSpy).not.toHaveBeenCalled()
      expect(localHostPreviewSpy).not.toHaveBeenCalled()
    },
  )

  test('refetches settings manual external outputs when shared scope or time range changes', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    const embedSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligenceEmbedCards')
      .mockResolvedValue([])
    const widgetSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligenceWidgetSnapshot')
      .mockResolvedValue({
        generatedAt: '2026-04-17T09:45:00Z',
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        digestSummary: {
          dateRange: { start: '2026-03-17', end: '2026-04-17' },
          totalVisits: {
            value: 128,
            previousValue: 120,
            changePercent: 7,
            trend: 'up',
          },
          totalSearches: {
            value: 32,
            previousValue: 28,
            changePercent: 14,
            trend: 'up',
          },
          newDomains: {
            value: 9,
            previousValue: 8,
            changePercent: 13,
            trend: 'up',
          },
          deepReadPages: {
            value: 5,
            previousValue: 4,
            changePercent: 25,
            trend: 'up',
          },
          refindPages: {
            value: 3,
            previousValue: 2,
            changePercent: 50,
            trend: 'up',
          },
        },
        highlights: [],
        notes: [],
      })
    const localHostPreviewSpy = vi
      .spyOn(coreIntelligenceApi, 'previewIntelligenceLocalHost')
      .mockResolvedValue(createLocalHostPreview('en'))
    const publicSpy = vi
      .spyOn(coreIntelligenceApi, 'getIntelligencePublicSnapshot')
      .mockResolvedValue({
        generatedAt: '2026-04-17T09:45:00Z',
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        digestSummary: {
          dateRange: { start: '2026-03-17', end: '2026-04-17' },
          totalVisits: {
            value: 128,
            previousValue: 120,
            changePercent: 7,
            trend: 'up',
          },
          totalSearches: {
            value: 32,
            previousValue: 28,
            changePercent: 14,
            trend: 'up',
          },
          newDomains: {
            value: 9,
            previousValue: 8,
            changePercent: 13,
            trend: 'up',
          },
          deepReadPages: {
            value: 5,
            previousValue: 4,
            changePercent: 25,
            trend: 'up',
          },
          refindPages: {
            value: 3,
            previousValue: 2,
            changePercent: 50,
            trend: 'up',
          },
        },
        topDomains: [],
        searchEngines: [],
        discoveryTrend: {
          availableYears: [],
          points: [],
        },
        notes: [],
      })

    function ScopedSettingsHarness() {
      const [activeProfileId, setActiveProfileId] = useState<string | null>(
        'chrome:Default',
      )

      return (
        <MemoryRouter initialEntries={['/settings']}>
          <I18nContext.Provider value={createI18nValue('en')}>
            <ProfileScopeContext.Provider
              value={{ activeProfileId, setActiveProfileId }}
            >
              <ShellDataContext.Provider
                value={createShellValue(snapshot, dashboard)}
              >
                <button
                  type="button"
                  onClick={() => setActiveProfileId('firefox:Research')}
                >
                  {settingsT('externalOutputsScopedTitle')}
                </button>
                <SettingsPage />
              </ShellDataContext.Provider>
            </ProfileScopeContext.Provider>
          </I18nContext.Provider>
        </MemoryRouter>
      )
    }

    render(<ScopedSettingsHarness />)

    await screen.findByTestId('settings-external-outputs')
    await waitFor(() => {
      expect(embedSpy).toHaveBeenLastCalledWith(
        expect.objectContaining({
          start: expect.any(String),
          end: expect.any(String),
        }),
        'chrome:Default',
        6,
      )
      expect(widgetSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'chrome:Default',
        4,
      )
      expect(publicSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'chrome:Default',
      )
      expect(localHostPreviewSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'en',
        'chrome:Default',
      )
    })

    const initialRange = embedSpy.mock.calls.at(-1)?.[0]

    await user.click(
      screen.getByRole('button', {
        name: settingsT('externalOutputsScopedTitle'),
      }),
    )
    await waitFor(() => {
      expect(embedSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'firefox:Research',
        6,
      )
      expect(widgetSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'firefox:Research',
        4,
      )
      expect(publicSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'firefox:Research',
      )
      expect(localHostPreviewSpy).toHaveBeenLastCalledWith(
        expect.any(Object),
        'en',
        'firefox:Research',
      )
    })

    await user.click(
      within(screen.getByTestId('settings-external-outputs')).getByRole(
        'button',
        { name: intelligenceT('rangeWeek') },
      ),
    )
    await waitFor(() => {
      const latestRange = embedSpy.mock.calls.at(-1)?.[0]
      expect(latestRange).toEqual(
        expect.objectContaining({
          start: expect.any(String),
          end: expect.any(String),
        }),
      )
      expect(latestRange).not.toEqual(initialRange)
      expect(localHostPreviewSpy).toHaveBeenLastCalledWith(
        expect.objectContaining({
          start: expect.any(String),
          end: expect.any(String),
        }),
        'en',
        'firefox:Research',
      )
    })
  })

  test('builds the trusted local host and exposes verify/open actions in settings', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedArchiveState()
    const settingsT = createNamespaceTranslator('en', 'settings')
    const commonT = createNamespaceTranslator('en', 'common')
    const previewPayload = createLocalHostPreview('en')
    const builtPayload: IntelligenceLocalHostBuildResult = {
      ...createLocalHostPreview('en'),
      installedHost: {
        artifactRoot: previewPayload.artifactRoot,
        entryFilePath: previewPayload.entryFilePath,
        bundle: previewPayload.bundle,
      },
    }

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getIntelligenceEmbedCards',
    ).mockResolvedValue([])
    vi.spyOn(
      coreIntelligenceApi,
      'getIntelligenceWidgetSnapshot',
    ).mockResolvedValue({
      generatedAt: '2026-04-17T09:45:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      highlights: [],
      notes: [],
    })
    vi.spyOn(
      coreIntelligenceApi,
      'getIntelligencePublicSnapshot',
    ).mockResolvedValue({
      generatedAt: '2026-04-17T09:45:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      topDomains: [],
      searchEngines: [],
      discoveryTrend: { points: [], availableYears: [] },
      notes: [],
    })
    const previewSpy = vi
      .spyOn(coreIntelligenceApi, 'previewIntelligenceLocalHost')
      .mockResolvedValue(previewPayload)
    const buildSpy = vi
      .spyOn(coreIntelligenceApi, 'buildIntelligenceLocalHost')
      .mockResolvedValue(builtPayload)
    const openExternalUrlSpy = vi
      .spyOn(backend, 'openExternalUrl')
      .mockResolvedValue('file:///tmp/pathkeep/index.html')
    const openPathSpy = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue(previewPayload.artifactRoot)

    renderSurface(<SettingsPage />, {
      dashboard,
      language: 'en',
      route: '/settings',
      snapshot,
    })

    const panel = await screen.findByTestId('settings-external-outputs')
    await within(panel).findByText(
      settingsT('externalOutputsLocalHostSummaryTitle'),
    )

    await user.click(
      within(panel).getByRole('button', {
        name: settingsT('externalOutputsLocalHostCreateAction'),
      }),
    )

    await waitFor(() => {
      expect(buildSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          start: expect.any(String),
          end: expect.any(String),
        }),
        'en',
        null,
      )
    })
    expect(
      await within(panel).findByText(
        settingsT('externalOutputsLocalHostBuilt'),
      ),
    ).toBeVisible()
    expect(
      within(panel).getByRole('button', {
        name: settingsT('externalOutputsLocalHostOpenAction'),
      }),
    ).toBeVisible()
    expect(
      within(panel).getByRole('button', { name: settingsT('openDirectory') }),
    ).toBeVisible()
    expect(
      within(panel).getAllByRole('button', { name: commonT('copyAction') })
        .length,
    ).toBeGreaterThan(0)

    await user.click(
      within(panel).getByRole('button', {
        name: settingsT('externalOutputsLocalHostOpenAction'),
      }),
    )
    expect(openExternalUrlSpy).toHaveBeenCalledWith(
      `file://${encodeURI(builtPayload.installedHost!.entryFilePath)}`,
    )

    await user.click(
      within(panel).getByRole('button', { name: settingsT('openDirectory') }),
    )
    expect(openPathSpy).toHaveBeenCalledWith(
      builtPayload.installedHost!.artifactRoot,
    )
    expect(previewSpy).toHaveBeenCalled()
  })

  test('renders background jobs controls and lets the user pause or replay work', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')
    const queueStatus: AiQueueStatus = {
      paused: false,
      concurrency: 2,
      queued: 1,
      running: 1,
      failed: 1,
      recentJobs: [
        {
          id: 77,
          jobType: 'index-build',
          state: 'failed',
          priority: 10,
          attempt: 2,
          maxAttempts: 3,
          runId: null,
          summary: 'Provider quota window has not reset yet.',
          queuedAt: '2026-04-07T18:00:00Z',
          availableAt: '2026-04-07T18:00:00Z',
          startedAt: '2026-04-07T18:01:00Z',
          finishedAt: '2026-04-07T18:02:00Z',
          heartbeatAt: '2026-04-07T18:01:30Z',
          errorCode: 'rate-limited',
          errorMessage: '429',
        },
        {
          id: 78,
          jobType: 'assistant',
          state: 'queued',
          priority: 10,
          attempt: 1,
          maxAttempts: 3,
          runId: null,
          summary: null,
          queuedAt: '2026-04-07T18:03:00Z',
          availableAt: '2026-04-07T18:03:00Z',
          startedAt: null,
          finishedAt: null,
          heartbeatAt: null,
          errorCode: null,
          errorMessage: null,
        },
      ],
    }
    const runtimeSnapshot: IntelligenceRuntimeSnapshot = {
      queue: {
        queued: 1,
        running: 1,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        lastActivityAt: '2026-04-10T16:30:00Z',
      },
      plugins: [
        {
          pluginId: 'readable-content-refetch',
          sourceKind: 'network',
          enabled: true,
          storedRecords: 5,
          queuedJobs: 1,
          runningJobs: 0,
          failedJobs: 1,
          lastCompletedAt: '2026-04-10T16:20:00Z',
          lastError: '429 from upstream host',
        },
      ],
      modules: [
        {
          moduleId: 'sessions',
          enabled: true,
          version: 'ci-v1',
          status: 'stale',
          dependsOn: ['visit-derived-facts'],
          derivedTables: ['sessions'],
          lastRunId: 12,
          lastBuiltAt: '2026-04-10T16:25:00Z',
          lastInvalidatedAt: '2026-04-10T16:28:00Z',
          staleReason: 'New imports were added after the last rebuild.',
          notes: ['Session grouping stayed stable across the latest rebuild.'],
        },
      ],
      recentJobs: [
        {
          id: 411,
          jobType: 'deterministic-rebuild',
          pluginId: null,
          state: 'running',
          historyId: null,
          profileId: 'chrome:Default',
          url: null,
          title: 'chrome:Default · 30 days',
          attempt: 2,
          createdAt: '2026-04-10T15:35:00Z',
          startedAt: '2026-04-10T15:36:00Z',
          finishedAt: null,
          updatedAt: '2026-04-10T15:36:45Z',
          heartbeatAt: '2026-04-10T15:36:45Z',
          progressLabel: 'Scoring visits',
          progressDetail: '24,000 / 64,781 visits',
          progressCurrent: 24000,
          progressTotal: 64781,
          progressPercent: 46.8,
          lastError: null,
          retryable: false,
          cancellable: true,
        },
        {
          id: 412,
          jobType: 'enrichment-plugin',
          pluginId: 'readable-content-refetch',
          state: 'failed',
          historyId: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Article',
          attempt: 2,
          createdAt: '2026-04-10T15:20:00Z',
          startedAt: '2026-04-10T15:21:00Z',
          finishedAt: '2026-04-10T15:22:00Z',
          updatedAt: '2026-04-10T15:22:00Z',
          heartbeatAt: null,
          progressLabel: null,
          progressDetail: null,
          progressCurrent: null,
          progressTotal: null,
          progressPercent: null,
          lastError: '429 from upstream host',
          retryable: true,
          cancellable: false,
        },
      ],
      notes: [
        'Recovered 1 interrupted deterministic rebuild job after restart.',
      ],
    }

    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue(queueStatus)
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      runtimeSnapshot,
    )
    const replaySpy = vi
      .spyOn(backend, 'replayAiJob')
      .mockResolvedValue(queueStatus.recentJobs[0])
    const retrySpy = vi
      .spyOn(backend, 'retryIntelligenceJob')
      .mockResolvedValue(runtimeSnapshot)

    const pausedSnapshot = structuredClone(snapshot)
    pausedSnapshot.config.ai.jobQueuePaused = true
    const shellValue = createShellValue(snapshot)
    shellValue.saveConfig = vi.fn().mockResolvedValue(pausedSnapshot)

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      shellValue,
      snapshot,
    })

    expect(await screen.findByText(jobsT('failedTitle'))).toBeVisible()
    expect(screen.getByText(jobsT('statusEyebrow'))).toBeVisible()
    expect(screen.getByText('Derived-data queue')).toBeVisible()
    expect(screen.getByText('Scoring visits')).toBeVisible()
    expect(
      screen.getAllByText('24,000 / 64,781 visits').length,
    ).toBeGreaterThan(0)
    expect(screen.getByText('47%')).toBeVisible()

    await user.click(screen.getByRole('button', { name: jobsT('pauseQueue') }))
    await waitFor(() =>
      expect(shellValue.saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          ai: expect.objectContaining({ jobQueuePaused: true }),
        }),
      ),
    )

    const aiPanel = screen.getByText(jobsT('recentAiJobs')).closest('.panel')
    expect(aiPanel).not.toBeNull()
    if (!(aiPanel instanceof HTMLElement)) {
      throw new Error('expected recent ai jobs panel')
    }
    await user.click(
      within(aiPanel).getAllByRole('button', { name: jobsT('retryJob') })[0],
    )
    expect(replaySpy).toHaveBeenCalledWith(77)

    const runtimePanel = screen
      .getByText(jobsT('recentRuntimeJobs'))
      .closest('.panel')
    expect(runtimePanel).not.toBeNull()
    if (!(runtimePanel instanceof HTMLElement)) {
      throw new Error('expected recent runtime jobs panel')
    }
    await user.click(
      within(runtimePanel).getByRole('button', { name: jobsT('retryJob') }),
    )
    expect(retrySpy).toHaveBeenCalledWith(412)
  })

  test('keeps failed backlog honest even when the latest runtime item is still running', async () => {
    const { snapshot } = await seedArchiveState()
    const jobsT = createNamespaceTranslator('en', 'jobs')

    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue({
      paused: false,
      concurrency: 1,
      queued: 0,
      running: 0,
      failed: 0,
      recentJobs: [],
    })
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue({
      queue: {
        queued: 3,
        running: 1,
        succeeded: 0,
        failed: 2,
        cancelled: 0,
        lastActivityAt: '2026-04-10T16:30:00Z',
      },
      plugins: [
        {
          pluginId: 'readable-content-refetch',
          sourceKind: 'network',
          enabled: true,
          storedRecords: 5,
          queuedJobs: 3,
          runningJobs: 1,
          failedJobs: 2,
          lastCompletedAt: '2026-04-10T16:20:00Z',
          lastError: 'unsupported-content',
        },
      ],
      modules: [],
      recentJobs: [
        {
          id: 990,
          jobType: 'enrichment-plugin',
          pluginId: 'readable-content-refetch',
          state: 'running',
          historyId: 2,
          profileId: 'chrome:Default',
          url: 'https://example.com/article',
          title: 'Article',
          attempt: 1,
          createdAt: '2026-04-10T15:20:00Z',
          startedAt: '2026-04-10T15:21:00Z',
          finishedAt: null,
          updatedAt: '2026-04-10T15:22:00Z',
          heartbeatAt: null,
          progressLabel: null,
          progressDetail: null,
          progressCurrent: null,
          progressTotal: null,
          progressPercent: null,
          lastError: null,
          retryable: false,
          cancellable: true,
        },
      ],
      notes: [],
    })

    renderSurface(<JobsPage />, {
      language: 'en',
      route: '/jobs',
      snapshot,
    })

    expect(
      await screen.findByText(jobsT('needsReviewBacklog', { count: 2 })),
    ).toBeVisible()
    expect(
      screen.getAllByText(jobsT('errorUnsupportedContent')).length,
    ).toBeGreaterThan(0)
  })

  test('keeps polling while background work is still queued or running', async () => {
    vi.useFakeTimers()

    try {
      const { snapshot } = await seedArchiveState()
      const jobsT = createNamespaceTranslator('en', 'jobs')

      const queueStatus: AiQueueStatus = {
        paused: false,
        concurrency: 1,
        queued: 2,
        running: 1,
        failed: 0,
        recentJobs: [],
      }

      const runtimeSnapshot: IntelligenceRuntimeSnapshot = {
        queue: {
          queued: 1,
          running: 0,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: '2026-04-10T16:30:00Z',
        },
        plugins: [],
        modules: [],
        recentJobs: [],
        notes: [],
      }

      const loadAiQueueStatusSpy = vi
        .spyOn(backend, 'loadAiQueueStatus')
        .mockResolvedValue(queueStatus)
      const loadRuntimeSpy = vi
        .spyOn(backend, 'loadIntelligenceRuntime')
        .mockResolvedValue(runtimeSnapshot)

      renderSurface(<JobsPage />, {
        language: 'en',
        route: '/jobs',
        snapshot,
      })

      await act(async () => {
        await Promise.resolve()
        await Promise.resolve()
      })

      expect(screen.getByText(jobsT('runningTitle'))).toBeVisible()
      expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(1)
      expect(loadRuntimeSpy).toHaveBeenCalledTimes(1)

      await act(async () => {
        await vi.advanceTimersByTimeAsync(3000)
      })

      expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(2)
      expect(loadRuntimeSpy).toHaveBeenCalledTimes(2)

      await act(async () => {
        await vi.advanceTimersByTimeAsync(3000)
      })

      expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(3)
      expect(loadRuntimeSpy).toHaveBeenCalledTimes(3)
    } finally {
      vi.useRealTimers()
    }
  })

  test('renders assistant queue state, provider probe, and answer citations', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const assistantT = createNamespaceTranslator('zh-CN', 'assistant')

    enableAi(snapshot)
    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')

    const queueStatus: AiQueueStatus = {
      paused: false,
      concurrency: 1,
      queued: 1,
      running: 1,
      failed: 0,
      recentJobs: [
        {
          id: 77,
          jobType: 'assistant',
          state: 'queued',
          priority: 10,
          attempt: 1,
          maxAttempts: 3,
          runId: null,
          summary: null,
          queuedAt: '2026-04-07T18:00:00Z',
          availableAt: '2026-04-07T18:00:00Z',
          startedAt: null,
          finishedAt: null,
          heartbeatAt: null,
          errorCode: null,
          errorMessage: null,
        },
      ],
    }
    const providerProbe: AiProviderConnectionTestReport = {
      providerId: 'llm-local',
      purpose: 'llm',
      model: 'qwen3:8b',
      ok: true,
      latencyMs: 210,
      capabilities: {
        supportsChat: true,
        supportsEmbeddings: false,
        supportsStreaming: true,
        supportsToolUse: false,
        supportsStructuredOutput: true,
      },
      warnings: [],
      message: 'Provider responded with a healthy chat completion.',
      actionHint: 'Safe to use for evidence-backed answers.',
      retryHint: null,
      errorCode: null,
    }

    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue(queueStatus)
    vi.spyOn(backend, 'testAiProviderConnection').mockResolvedValue(
      providerProbe,
    )
    vi.spyOn(backend, 'askAiAssistant').mockResolvedValue({
      state: 'completed',
      answer: '语义搜索质量趋势稳定，证据契约文档被多次访问。',
      jobId: 91,
      runId: 12,
      providerId: 'llm-local',
      embeddingProviderId: 'embed-local',
      citations: [
        {
          historyId: 301,
          profileId: 'chrome:Default',
          url: 'https://example.com/semantic-quality',
          title: 'Semantic quality notes',
          visitedAt: '2026-04-06T15:20:00Z',
          score: 0.92,
        },
      ],
      notes: ['Answer kept inside the archive evidence boundary.'],
    })

    renderSurface(<AssistantPage />, {
      language: 'zh-CN',
      route: '/assistant',
      snapshot,
    })

    expect(await screen.findByText(assistantT('scopedViewTitle'))).toBeVisible()
    expect(await screen.findByText(assistantT('runningContext'))).toBeVisible()
    expect(
      await screen.findByText(assistantT('queuedJobLabel', { id: 77 })),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: assistantT('testProvider') }),
    )
    expect(
      await screen.findByText(assistantT('providerReachable')),
    ).toBeVisible()

    const input = await screen.findByPlaceholderText(
      assistantT('inputPlaceholder'),
    )
    expect(
      screen.getByRole('button', { name: assistantT('sendAction') }),
    ).toBeVisible()
    await user.type(input, '总结最近的证据{enter}')

    expect(
      await screen.findByText('语义搜索质量趋势稳定，证据契约文档被多次访问。'),
    ).toBeVisible()
    expect(
      await screen.findByText(assistantT('evidenceLabel', { count: 1 })),
    ).toBeVisible()

    window.localStorage.removeItem('pathkeep.profile-scope')
  })

  test('inherits shared intelligence scope and lets explicit profileId override it', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const summary = {
      dateRange: { start: '2026-04-01', end: '2026-04-07' },
      totalVisits: { value: 12, trend: 'flat' as const },
      totalSearches: { value: 3, trend: 'flat' as const },
      newDomains: { value: 2, trend: 'flat' as const },
      deepReadPages: { value: 1, trend: 'flat' as const },
      refindPages: { value: 1, trend: 'flat' as const },
    }
    const digestSpy = vi
      .spyOn(coreIntelligenceApi, 'getDigestSummary')
      .mockResolvedValue(wrapSection('digest-summary', summary))

    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')

    try {
      const first = renderSurface(<IntelligencePage />, {
        route: '/intelligence',
        snapshot,
      })

      expect(await screen.findByTestId('intelligence-page')).toBeVisible()
      await waitFor(() =>
        expect(digestSpy).toHaveBeenCalledWith(
          expect.anything(),
          'chrome:Default',
        ),
      )
      expect(
        await screen.findByText(
          intelligenceT('scopedViewBody', { profile: 'Default' }),
        ),
      ).toBeVisible()

      first.unmount()
      digestSpy.mockClear()

      renderSurface(<IntelligencePage />, {
        route: '/intelligence?profileId=firefox:Research',
        snapshot,
      })

      await waitFor(() =>
        expect(digestSpy).toHaveBeenCalledWith(
          expect.anything(),
          'firefox:Research',
        ),
      )
      expect(
        await screen.findByText(
          intelligenceT('scopedViewBody', { profile: 'Research' }),
        ),
      ).toBeVisible()
    } finally {
      window.localStorage.removeItem('pathkeep.profile-scope')
    }
  })

  test('shows a compact runtime digest without a full-width settings banner', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const shellValue = createShellValue(snapshot, null)
    shellValue.runtimeStatus = {
      aiQueue: {
        paused: false,
        concurrency: 1,
        queued: 0,
        running: 0,
        failed: 0,
        recentJobs: [],
      },
      intelligence: {
        queue: {
          queued: 1,
          running: 1,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: '2026-04-17T09:40:00Z',
        },
        plugins: [],
        modules: [],
        recentJobs: [
          {
            id: 812,
            jobType: 'deterministic-rebuild',
            pluginId: null,
            state: 'running',
            historyId: null,
            profileId: 'chrome:Default',
            url: null,
            title: 'chrome:Default · 30 days',
            attempt: 1,
            createdAt: '2026-04-17T09:35:00Z',
            startedAt: '2026-04-17T09:36:00Z',
            finishedAt: null,
            updatedAt: '2026-04-17T09:40:00Z',
            heartbeatAt: '2026-04-17T09:40:00Z',
            progressLabel: 'Scoring visits',
            progressDetail: '24,000 / 64,781 visits',
            progressCurrent: 24000,
            progressTotal: 64781,
            progressPercent: 46.8,
            lastError: null,
            retryable: false,
            cancellable: true,
          },
        ],
        notes: [],
      },
      loading: false,
      error: null,
    }

    renderSurface(<IntelligencePage />, {
      route: '/intelligence',
      shellValue,
      snapshot,
    })

    const digest = await screen.findByTestId('intelligence-runtime-digest')
    expect(
      within(digest).getByText(intelligenceT('runtimeDigestTitle')),
    ).toBeVisible()
    expect(
      within(digest).getByText(
        intelligenceT('runtimeDigestRunningTitle', { count: 1 }),
      ),
    ).toBeVisible()
    expect(within(digest).getByText('24,000 / 64,781 visits')).toBeVisible()
    expect(within(digest).getByRole('link', { name: 'Jobs' })).toHaveAttribute(
      'href',
      '/jobs',
    )
    expect(
      screen.queryByText(intelligenceT('externalOutputsReviewTitle')),
    ).not.toBeInTheDocument()
  })

  test('renders archive-wide copy and decoded domain paths without raw keys in zh-TW', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('zh-TW', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockResolvedValue(
      wrapSection('digest-summary', {
        dateRange: { start: '2026-04-01', end: '2026-04-07' },
        totalVisits: { value: 12, trend: 'flat' as const },
        totalSearches: { value: 3, trend: 'flat' as const },
        newDomains: { value: 2, trend: 'flat' as const },
        deepReadPages: { value: 1, trend: 'flat' as const },
        refindPages: { value: 1, trend: 'flat' as const },
      }),
    )
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue(
      wrapSection('on-this-day', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection('top-sites', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
      wrapSection('engine-ranking', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
      wrapSection('search-concepts', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
      wrapSection('query-families', {
        page: 0,
        pageSize: 20,
        total: 0,
        families: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
      wrapSection('refind-pages', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getActivityMix').mockResolvedValue(
      wrapSection('activity-mix', {
        categories: [
          { domainCategory: 'community', visitCount: 5, share: 0.25 },
          { domainCategory: 'search', visitCount: 15, share: 0.75 },
        ],
        changeVsPrevious: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getBrowsingRhythm').mockResolvedValue(
      wrapSection('browsing-rhythm', {
        cells: [],
        maxCount: 0,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
      wrapSection('stable-sources', [
        {
          registrableDomain: 'wikipedia.org',
          displayName: 'wikipedia.org',
          sourceRole: 'landing',
          trailCount: 0,
          stableLandingCount: 1,
          effectivenessScore: 0.1,
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
      wrapSection('search-effectiveness', {
        engineStats: [],
        topResolvingSources: [],
        hardestTopics: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
      wrapSection('friction-signals', [
        {
          registrableDomain: 'example.com',
          url: 'https://example.com/article',
          evidenceType: 'weak',
          signalKind: 'single_bounce',
          occurrenceCount: 1,
          description: 'Came back to search once.',
        },
      ]),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getReopenedInvestigations',
    ).mockResolvedValue(
      wrapSection('reopened-investigations', [
        {
          investigationId: 'query::chatgpt',
          anchorType: 'query_family',
          anchorId: 'chatgpt',
          anchorLabel: 'ChatGPT',
          occurrenceCount: 3,
          distinctDays: 3,
          firstSeenAt: '2026-04-01',
          lastSeenAt: '2026-04-07',
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
      wrapSection('discovery-trend', { points: [], availableYears: [] }),
    )
    vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
      wrapSection('breadth-index', {
        breadthScore: 42,
        hhi: 0.42,
        concentrationDomainCount: 5,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
      wrapSection('path-flows', [
        {
          flowId: 'chatgpt-hop',
          flowPattern: 'chat.openai.com → chatgpt.com',
          stepCount: 2,
          occurrenceCount: 6,
          lastSeenAt: '2026-04-07T10:00:00Z',
          steps: [
            {
              index: 0,
              label: 'chat.openai.com',
              registrableDomain: 'openai.com',
            },
            {
              index: 1,
              label: 'chatgpt.com',
              registrableDomain: 'chatgpt.com',
            },
          ],
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
      wrapSection('habit-patterns', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
      wrapSection('interrupted-habits', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getCompareSets').mockResolvedValue(
      wrapSection('compare-sets', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getMultiBrowserDiff').mockResolvedValue(
      wrapSection('multi-browser-diff', {
        profiles: [],
        sharedDomains: [],
        exclusiveDomains: [],
        categoryDistributions: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
      wrapSection('observed-interactions', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getDomainDeepDive').mockResolvedValue(
      wrapSection('domain-deep-dive', {
        registrableDomain: 'github.com',
        displayName: 'GitHub',
        domainCategory: 'community',
        totalVisits: 38,
        activeDays: 7,
        trailCount: 4,
        arrivalBreakdown: { search: 10, link: 12, typed: 8, other: 8 },
        topPages: [
          {
            path: '/wiki/%E5%93%88%E5%B8%83%E6%96%AF%E5%A0%A1%E5%90%9B%E4%B8%BB%E5%9C%8B',
            visitCount: 12,
          },
        ],
        topReferrers: [],
        topExits: [],
        visitTrend: [],
      }),
    )

    const first = renderSurface(<IntelligencePage />, {
      language: 'zh-TW',
      route: '/intelligence',
      snapshot,
    })

    expect(
      await screen.findByText(intelligenceT('activityMixHelp')),
    ).toBeVisible()
    expect(
      (await screen.findAllByText(intelligenceT('archiveWideBadge'))).length,
    ).toBeGreaterThan(0)
    expect(
      screen.queryByText(intelligenceT('archiveWideBody')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(intelligenceT('externalOutputsReviewBody')),
    ).not.toBeInTheDocument()
    expect(
      screen.getAllByText(intelligenceT('category_community')).length,
    ).toBeGreaterThan(0)
    expect(screen.getByText(intelligenceT('activityMixHelp'))).toBeVisible()
    expect(
      screen.queryByText(intelligenceT('stableSourcesTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(intelligenceT('searchEffectivenessTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(intelligenceT('frictionTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(intelligenceT('reopenedTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText('intelligence.archiveWideBadge'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText('intelligence.archiveWideBody'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText('Intelligence.category_community'),
    ).not.toBeInTheDocument()

    first.unmount()

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/domain/:domain"
          element={<DomainDeepDiveRoutePage />}
        />
      </Routes>,
      {
        language: 'zh-TW',
        route: '/intelligence/domain/github.com?range=month',
        snapshot,
      },
    )

    expect(await screen.findByText('/wiki/哈布斯堡君主國')).toBeVisible()
    expect(
      screen.queryByText(/%E5%93%88%E5%B8%83%E6%96%AF/i),
    ).not.toBeInTheDocument()
  })

  test('navigates to the day insights route when a browsing-rhythm day is selected', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    const getDigestSummarySpy = vi
      .spyOn(coreIntelligenceApi, 'getDigestSummary')
      .mockImplementation((dateRange) => {
        if (dateRange.start === dateRange.end) {
          return Promise.resolve(
            wrapSection('digest-summary', {
              dateRange,
              totalVisits: { value: 42, trend: 'flat' as const },
              totalSearches: { value: 8, trend: 'flat' as const },
              newDomains: { value: 2, trend: 'flat' as const },
              deepReadPages: { value: 5, trend: 'flat' as const },
              refindPages: { value: 1, trend: 'flat' as const },
            }),
          )
        }

        return Promise.resolve(
          wrapSection('digest-summary', {
            dateRange,
            totalVisits: { value: 240, trend: 'flat' as const },
            totalSearches: { value: 24, trend: 'flat' as const },
            newDomains: { value: 9, trend: 'flat' as const },
            deepReadPages: { value: 18, trend: 'flat' as const },
            refindPages: { value: 4, trend: 'flat' as const },
          }),
        )
      })
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue(
      wrapSection('on-this-day', []),
    )
    const getTopSitesSpy = vi
      .spyOn(coreIntelligenceApi, 'getTopSites')
      .mockImplementation((dateRange) => {
        if (dateRange.start === dateRange.end) {
          return Promise.resolve(
            wrapSection('top-sites', [
              {
                registrableDomain: 'sqlite.org',
                displayName: 'sqlite.org',
                domainCategory: 'docs',
                visitCount: 14,
                uniqueDays: 1,
                averageDailyVisits: 14,
                uniqueUrls: 4,
              },
            ]),
          )
        }

        return Promise.resolve(
          wrapSection('top-sites', [
            {
              registrableDomain: 'example.com',
              displayName: 'example.com',
              domainCategory: 'docs',
              visitCount: 20,
              uniqueDays: 5,
              averageDailyVisits: 4,
              uniqueUrls: 6,
            },
          ]),
        )
      })
    vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
      wrapSection('engine-ranking', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
      wrapSection('search-concepts', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
      wrapSection('query-families', {
        page: 0,
        pageSize: 10,
        total: 0,
        families: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
      wrapSection('refind-pages', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getActivityMix').mockResolvedValue(
      wrapSection('activity-mix', {
        categories: [{ domainCategory: 'docs', visitCount: 20, share: 1 }],
        changeVsPrevious: [],
      }),
    )
    const getBrowsingRhythmSpy = vi
      .spyOn(coreIntelligenceApi, 'getBrowsingRhythm')
      .mockImplementation((dateRange) =>
        Promise.resolve(
          wrapSection('browsing-rhythm', {
            cells:
              dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15'
                ? [{ dow: 3, hour: 10, visitCount: 9 }]
                : [{ dow: 4, hour: 8, visitCount: 4 }],
            maxCount:
              dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15'
                ? 9
                : 4,
          }),
        ),
      )
    vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
      wrapSection('stable-sources', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
      wrapSection('search-effectiveness', {
        engineStats: [],
        topResolvingSources: [],
        hardestTopics: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
      wrapSection('friction-signals', []),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getReopenedInvestigations',
    ).mockResolvedValue(wrapSection('reopened-investigations', []))
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      (_dateRange, _profileId, granularity) =>
        Promise.resolve(
          wrapSection('discovery-trend', {
            availableYears: [2026, 2025],
            points:
              granularity === 'day'
                ? [
                    {
                      dateKey: '2026-04-15',
                      discoveryRate: 0.18,
                      newDomainCount: 2,
                      totalVisits: 42,
                    },
                    {
                      dateKey: '2026-04-16',
                      discoveryRate: 0.1,
                      newDomainCount: 1,
                      totalVisits: 12,
                    },
                  ]
                : [],
          }),
        ),
    )
    vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
      wrapSection('breadth-index', {
        breadthScore: 42,
        hhi: 0.42,
        concentrationDomainCount: 5,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
      wrapSection('path-flows', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
      wrapSection('habit-patterns', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
      wrapSection('interrupted-habits', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getCompareSets').mockResolvedValue(
      wrapSection('compare-sets', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getMultiBrowserDiff').mockResolvedValue(
      wrapSection('multi-browser-diff', {
        profiles: [],
        sharedDomains: [],
        exclusiveDomains: [],
        categoryDistributions: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
      wrapSection('observed-interactions', []),
    )

    renderSurface(
      <Routes>
        <Route path="/intelligence" element={<IntelligencePage />} />
        <Route
          path="/intelligence/day/:date"
          element={<div data-testid="day-insights-route-target" />}
        />
      </Routes>,
      {
        language: 'en',
        route: '/intelligence',
        snapshot,
      },
    )

    await user.click(
      await screen.findByRole('button', {
        name: /2026-04-15 · 42 visits · 2 new sites/i,
      }),
    )

    expect(
      await screen.findByTestId('day-insights-route-target'),
    ).toBeInTheDocument()
    expect(
      getDigestSummarySpy.mock.calls.some(
        ([dateRange]) =>
          dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15',
      ),
    ).toBe(false)
    expect(
      getTopSitesSpy.mock.calls.some(
        ([dateRange]) =>
          dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15',
      ),
    ).toBe(false)
    expect(
      getBrowsingRhythmSpy.mock.calls.some(
        ([dateRange]) =>
          dateRange.start === '2026-04-15' && dateRange.end === '2026-04-15',
      ),
    ).toBe(false)
  })

  test('renders browsing rhythm as a real-date calendar and keeps secondary cards capped', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockImplementation(
      (dateRange) =>
        Promise.resolve(
          wrapSection('digest-summary', {
            dateRange,
            totalVisits: {
              value: dateRange.start === dateRange.end ? 8 : 180,
              trend: 'flat' as const,
            },
            totalSearches: { value: 6, trend: 'flat' as const },
            newDomains: { value: 2, trend: 'flat' as const },
            deepReadPages: { value: 4, trend: 'flat' as const },
            refindPages: { value: 1, trend: 'flat' as const },
          }),
        ),
    )
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue(
      wrapSection('on-this-day', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockImplementation(
      (dateRange) =>
        Promise.resolve(
          wrapSection('top-sites', [
            {
              registrableDomain:
                dateRange.start === dateRange.end
                  ? 'calendar.test'
                  : 'example.com',
              displayName:
                dateRange.start === dateRange.end
                  ? 'calendar.test'
                  : 'example.com',
              domainCategory: 'docs',
              visitCount: dateRange.start === dateRange.end ? 8 : 24,
              uniqueDays: 1,
              averageDailyVisits: 8,
              uniqueUrls: 3,
            },
          ]),
        ),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
      wrapSection('engine-ranking', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
      wrapSection('search-concepts', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchQueries').mockResolvedValue(
      wrapSection('search-activity', {
        page: 0,
        pageSize: 20,
        total: 1,
        rows: [
          {
            visitId: 88,
            profileId: 'chrome:Default',
            browserKind: 'chrome',
            searchEngine: 'google',
            displayName: 'Google',
            rawQuery: 'sqlite wal checkpoint',
            normalizedQuery: 'sqlite wal checkpoint',
            searchedAt: '2026-01-28T09:30:00Z',
            searchedAtMs: 1_706_434_200_000,
            exactRepeatCount: 2,
            familyCount: 4,
            familyId: 'family-1',
            trailId: 'trail-1',
            trailInitialQuery: 'sqlite wal checkpoint',
            trailReformulationCount: 3,
          },
        ],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
      wrapSection('query-families', {
        page: 0,
        pageSize: 10,
        total: 0,
        families: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
      wrapSection('refind-pages', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getActivityMix').mockResolvedValue(
      wrapSection('activity-mix', {
        categories: [
          { domainCategory: 'docs', visitCount: 24, share: 0.6 },
          { domainCategory: 'search', visitCount: 16, share: 0.4 },
        ],
        changeVsPrevious: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getBrowsingRhythm').mockImplementation(
      (dateRange) =>
        Promise.resolve(
          wrapSection('browsing-rhythm', {
            cells:
              dateRange.start === dateRange.end
                ? [
                    { dow: 6, hour: 9, visitCount: 2 },
                    { dow: 6, hour: 10, visitCount: 6 },
                    { dow: 6, hour: 14, visitCount: 4 },
                  ]
                : [],
            maxCount: dateRange.start === dateRange.end ? 6 : 0,
          }),
        ),
    )
    vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
      wrapSection('stable-sources', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
      wrapSection('search-effectiveness', {
        engineStats: [],
        topResolvingSources: [],
        hardestTopics: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
      wrapSection('friction-signals', []),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getReopenedInvestigations',
    ).mockResolvedValue(wrapSection('reopened-investigations', []))
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockImplementation(
      (_dateRange, _profileId, granularity) =>
        Promise.resolve(
          wrapSection('discovery-trend', {
            availableYears: [2026],
            points:
              granularity === 'day'
                ? [
                    {
                      dateKey: '2026-01-03',
                      discoveryRate: 0.25,
                      newDomainCount: 1,
                      totalVisits: 4,
                    },
                    {
                      dateKey: '2026-01-17',
                      discoveryRate: 0.125,
                      newDomainCount: 1,
                      totalVisits: 8,
                    },
                    {
                      dateKey: '2026-01-30',
                      discoveryRate: 0.25,
                      newDomainCount: 2,
                      totalVisits: 8,
                    },
                  ]
                : [
                    {
                      dateKey: '2026-W01',
                      discoveryRate: 0.14,
                      newDomainCount: 3,
                      totalVisits: 21,
                    },
                    {
                      dateKey: '2026-W04',
                      discoveryRate: 0.22,
                      newDomainCount: 5,
                      totalVisits: 23,
                    },
                  ],
          }),
        ),
    )
    vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
      wrapSection('breadth-index', {
        breadthScore: 62,
        hhi: 0.32,
        concentrationDomainCount: 5,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
      wrapSection('path-flows', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
      wrapSection('habit-patterns', [
        {
          registrableDomain: 'linux.do',
          displayName: 'linux.do',
          habitType: 'daily_habit',
          meanIntervalDays: 1.8,
          cv: 0.2,
          visitCount: 12,
          lastVisitedAt: '2026-01-30T08:00:00Z',
          isInterrupted: false,
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
      wrapSection('interrupted-habits', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getCompareSets').mockResolvedValue(
      wrapSection('compare-sets', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getMultiBrowserDiff').mockResolvedValue(
      wrapSection('multi-browser-diff', {
        profiles: [],
        sharedDomains: [],
        exclusiveDomains: [],
        categoryDistributions: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
      wrapSection('observed-interactions', []),
    )

    const { container } = renderSurface(<IntelligencePage />, {
      language: 'en',
      route: '/intelligence?range=custom&start=2026-01-01&end=2026-01-31',
      snapshot,
    })

    expect(
      await screen.findByRole('button', {
        name: /2026-01-03 · 4 visits · 1 new sites/i,
      }),
    ).toBeVisible()
    expect(
      screen.getByRole('button', {
        name: /2026-01-30 · 8 visits · 2 new sites/i,
      }),
    ).toBeVisible()
    expect(screen.getByText('2026 Week 4')).toBeVisible()

    const searchSection = screen.getByText('Search Activity').closest('section')
    const mixSection = screen.getByText('Activity Mix').closest('section')
    const sharedRow = searchSection?.parentElement
    expect(sharedRow).toHaveClass('intelligence-row--two-col')
    expect(sharedRow).toContainElement(mixSection)

    expect(
      searchSection?.querySelector('.intelligence-section__body'),
    ).not.toBeNull()
    expect(
      mixSection?.querySelector('.intelligence-section__body'),
    ).not.toBeNull()
    expect(
      screen
        .getByText('Browsing Rhythm')
        .closest('section')
        ?.querySelector('.intelligence-section__body--workbench'),
    ).not.toBeNull()
    expect(
      container.querySelector('.intelligence-secondary-grid'),
    ).not.toBeNull()
    expect(
      screen.queryByRole('heading', {
        name: intelligenceT('onThisDayTitle'),
      }),
    ).not.toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: 'Recent Queries' }))
    expect(
      await screen.findByRole('link', {
        name: 'Open query-family insights',
      }),
    ).toHaveAttribute(
      'href',
      '/intelligence/query-family/family-1?range=custom&start=2026-01-01&end=2026-01-31&profileId=chrome%3ADefault',
    )
    expect(
      screen.getByRole('link', { name: 'Open trail insights' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-1?range=custom&start=2026-01-01&end=2026-01-31&profileId=chrome%3ADefault',
    )
  })

  test('renders grouped storage analytics in the intelligence health tail', async () => {
    const { snapshot, dashboard } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')
    const commonT = createNamespaceTranslator('en', 'common')

    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      createEmptyRuntimeSnapshot(),
    )
    vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockResolvedValue(
      wrapSection('digest-summary', {
        dateRange: { start: '2026-01-01', end: '2026-01-31' },
        totalVisits: { value: 180, trend: 'flat' as const },
        totalSearches: { value: 24, trend: 'flat' as const },
        newDomains: { value: 8, trend: 'flat' as const },
        deepReadPages: { value: 12, trend: 'flat' as const },
        refindPages: { value: 4, trend: 'flat' as const },
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection('top-sites', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
      wrapSection('engine-ranking', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
      wrapSection('search-concepts', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
      wrapSection('query-families', {
        page: 0,
        pageSize: 10,
        total: 0,
        families: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
      wrapSection('refind-pages', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getActivityMix').mockResolvedValue(
      wrapSection('activity-mix', {
        categories: [],
        changeVsPrevious: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getBrowsingRhythm').mockResolvedValue(
      wrapSection('browsing-rhythm', {
        cells: [],
        maxCount: 0,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
      wrapSection('stable-sources', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
      wrapSection('search-effectiveness', {
        engineStats: [],
        topResolvingSources: [],
        hardestTopics: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
      wrapSection('friction-signals', []),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getReopenedInvestigations',
    ).mockResolvedValue(wrapSection('reopened-investigations', []))
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
      wrapSection('discovery-trend', { points: [], availableYears: [] }),
    )
    vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
      wrapSection('breadth-index', {
        breadthScore: 0,
        hhi: 0,
        concentrationDomainCount: 0,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
      wrapSection('path-flows', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getCompareSets').mockResolvedValue(
      wrapSection('compare-sets', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getMultiBrowserDiff').mockResolvedValue(
      wrapSection('multi-browser-diff', {
        profiles: [],
        sharedDomains: [],
        exclusiveDomains: [],
        categoryDistributions: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
      wrapSection('observed-interactions', []),
    )

    renderSurface(<IntelligencePage />, {
      dashboard,
      route: '/intelligence?range=custom&start=2026-01-01&end=2026-01-31',
      snapshot,
    })

    const storageHeading = await screen.findByRole('heading', {
      name: intelligenceT('storageAnalytics'),
    })
    const storageSection = storageHeading.closest('section')
    if (!(storageSection instanceof HTMLElement)) {
      throw new Error('expected storage analytics section')
    }

    expect(storageHeading).toBeVisible()
    expect(
      within(storageSection).getAllByText(commonT('coreHistory')).length,
    ).toBeGreaterThan(0)
    expect(
      within(storageSection).getAllByText(commonT('otherData')).length,
    ).toBeGreaterThan(0)
    expect(
      within(storageSection).getByText(commonT('canonicalArchive')),
    ).toBeVisible()
    expect(
      within(storageSection).getByText(commonT('auditArtifacts')),
    ).toBeVisible()
  })

  test('renders explorer session view and keeps navigation tracing wired to the selected grouped visit', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getSessions').mockResolvedValue({
      sessions: [
        {
          sessionId: 'session-1',
          firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
          lastVisitMs: Date.parse('2026-04-05T14:30:00Z'),
          visitCount: 3,
          searchCount: 1,
          domainCount: 2,
          isDeepDive: true,
          autoTitle: 'SQLite WAL research',
        },
      ],
      total: 1,
      page: 0,
      pageSize: 20,
    })
    vi.spyOn(coreIntelligenceApi, 'getSessionDetail').mockResolvedValue({
      session: {
        sessionId: 'session-1',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:30:00Z'),
        visitCount: 3,
        searchCount: 1,
        domainCount: 2,
        isDeepDive: true,
        autoTitle: 'SQLite WAL research',
      },
      visits: [
        {
          visitId: 101,
          url: 'https://www.sqlite.org/wal.html',
          title: 'SQLite WAL',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:05:00Z'),
          isSearchEvent: false,
          searchQuery: null,
          searchEngine: null,
          trailId: null,
          transitionType: 'link',
        },
      ],
      trails: [],
    })
    vi.spyOn(coreIntelligenceApi, 'getNavigationPath').mockResolvedValue({
      targetVisitId: 101,
      steps: [
        {
          visitId: 100,
          url: 'https://www.google.com/search?q=sqlite+wal',
          title: 'Google',
          visitTimeMs: Date.parse('2026-04-05T14:04:00Z'),
          depth: 0,
        },
        {
          visitId: 101,
          url: 'https://www.sqlite.org/wal.html',
          title: 'SQLite WAL',
          visitTimeMs: Date.parse('2026-04-05T14:05:00Z'),
          depth: 1,
        },
      ],
    })

    renderSurface(<ExplorerPage />, {
      route: '/explorer?view=session&start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    expect(await screen.findByText('SQLite WAL research')).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: /SQLite WAL research/i }),
    )
    await user.click(await screen.findByText('SQLite WAL'))
    await user.click(
      screen.getByRole('button', { name: intelligenceT('tracerTitle') }),
    )
    expect(await screen.findByText('Google')).toBeVisible()
    expect(
      screen.getByText(new RegExp(intelligenceT('tracerHere'))),
    ).toBeVisible()
  })

  test('renders explorer trail view and keeps grouped selection wired to the detail rail', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getSearchTrails').mockResolvedValue({
      trails: [
        {
          trailId: 'trail-1',
          sessionId: 'session-1',
          initialQuery: 'sqlite wal checkpoint',
          searchEngine: 'Google',
          reformulationCount: 1,
          visitCount: 2,
          landingUrl: 'https://www.sqlite.org/pragma.html',
          landingDomain: 'sqlite.org',
          firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
          lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
          maxDepth: 2,
          queries: ['sqlite wal checkpoint', 'sqlite wal checkpoint passive'],
        },
      ],
      total: 1,
      page: 0,
      pageSize: 20,
    })
    vi.spyOn(coreIntelligenceApi, 'getTrailDetail').mockResolvedValue({
      trail: {
        trailId: 'trail-1',
        sessionId: 'session-1',
        initialQuery: 'sqlite wal checkpoint',
        searchEngine: 'Google',
        reformulationCount: 1,
        visitCount: 2,
        landingUrl: 'https://www.sqlite.org/pragma.html',
        landingDomain: 'sqlite.org',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
        maxDepth: 2,
        queries: ['sqlite wal checkpoint', 'sqlite wal checkpoint passive'],
      },
      members: [
        {
          trailId: 'trail-1',
          visitId: 201,
          ordinal: 0,
          role: 'search_event',
          url: 'https://www.google.com/search?q=sqlite+wal+checkpoint+passive',
          title: 'Google',
          visitTimeMs: Date.parse('2026-04-05T14:02:00Z'),
          searchQuery: 'sqlite wal checkpoint passive',
        },
        {
          trailId: 'trail-1',
          visitId: 202,
          ordinal: 1,
          role: 'landing',
          url: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          searchQuery: null,
        },
      ],
    })
    vi.spyOn(coreIntelligenceApi, 'getNavigationPath').mockResolvedValue({
      targetVisitId: 202,
      steps: [
        {
          visitId: 201,
          url: 'https://www.google.com/search?q=sqlite+wal+checkpoint+passive',
          title: 'Google',
          visitTimeMs: Date.parse('2026-04-05T14:02:00Z'),
          depth: 0,
        },
        {
          visitId: 202,
          url: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          depth: 1,
        },
      ],
    })

    renderSurface(<ExplorerPage />, {
      route: '/explorer?view=trail&start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    expect(await screen.findByText('"sqlite wal checkpoint"')).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: /sqlite wal checkpoint/i }),
    )
    await user.click(await screen.findByText('PRAGMA wal_checkpoint'))
    await user.click(
      screen.getByRole('button', { name: intelligenceT('tracerTitle') }),
    )
    expect(
      (await screen.findAllByText('PRAGMA wal_checkpoint')).length,
    ).toBeGreaterThan(1)
  })

  test('keeps domain deep dives deep-linkable and preserves route-backed scope and date range', async () => {
    const { snapshot } = await seedArchiveState()

    const domainSpy = vi
      .spyOn(coreIntelligenceApi, 'getDomainDeepDive')
      .mockResolvedValue(
        wrapSection(
          'domain-deep-dive',
          {
            registrableDomain: 'github.com',
            displayName: 'GitHub',
            domainCategory: 'developer',
            totalVisits: 38,
            activeDays: 7,
            trailCount: 4,
            arrivalBreakdown: { search: 10, link: 12, typed: 8, other: 8 },
            topPages: [{ path: '/issues', visitCount: 12 }],
            topReferrers: [
              { domain: 'google.com', displayName: 'Google', count: 6 },
            ],
            topExits: [
              {
                domain: 'stackoverflow.com',
                displayName: 'Stack Overflow',
                count: 4,
              },
            ],
            visitTrend: [{ dateKey: '2026-04-05', visitCount: 6 }],
          },
          {
            moduleIds: ['daily-rollups', 'search-trails', 'domain-deep-dive'],
            sourceTables: [
              'visit_derived_facts',
              'domain_daily_rollups',
              'search_trails',
              'habit_patterns',
              'path_flows',
            ],
          },
        ),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/domain/:domain"
          element={<DomainDeepDiveRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/domain/github.com?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(await screen.findByTestId('domain-deep-dive-page')).toBeVisible()
    await waitFor(() =>
      expect(domainSpy).toHaveBeenCalledWith(
        'github.com',
        { start: '2026-04-01', end: '2026-04-07' },
        'chrome:Default',
      ),
    )
    expect(screen.getByRole('link', { name: /Back/i })).toHaveAttribute(
      'href',
      '/intelligence?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome%3ADefault',
    )
  })

  test('renders day insights as a first-class route with exact-day explorer and domain links', async () => {
    const { snapshot } = await seedArchiveState()
    const dayInsightsSpy = vi
      .spyOn(coreIntelligenceApi, 'getDayInsights')
      .mockResolvedValue(
        wrapSection<DayInsights>(
          'day-insights',
          {
            date: '2026-04-18',
            digestSummary: {
              dateRange: { start: '2026-04-18', end: '2026-04-18' },
              totalVisits: { value: 8, trend: 'flat' },
              totalSearches: { value: 3, trend: 'flat' },
              newDomains: { value: 2, trend: 'flat' },
              deepReadPages: { value: 4, trend: 'flat' },
              refindPages: { value: 1, trend: 'flat' },
            },
            topSites: [
              {
                registrableDomain: 'sqlite.org',
                displayName: 'SQLite',
                domainCategory: 'docs',
                visitCount: 4,
                uniqueDays: 1,
                averageDailyVisits: 4,
                uniqueUrls: 2,
              },
            ],
            activityMix: {
              categories: [{ domainCategory: 'docs', visitCount: 8, share: 1 }],
              changeVsPrevious: [],
            },
            refindPages: [
              {
                canonicalUrl: 'https://sqlite.org/lang.html',
                url: 'https://sqlite.org/lang.html',
                title: 'SQLite docs',
                registrableDomain: 'sqlite.org',
                crossDayCount: 3,
                trailCount: 2,
                searchArrivalCount: 1,
                typedRevisitCount: 0,
                refindScore: 5,
                firstSeenAt: '2026-04-10T00:00:00Z',
                lastSeenAt: '2026-04-18T00:00:00Z',
              },
            ],
            queryFamilies: {
              families: [
                {
                  familyId: 'family-1',
                  anchorQuery: 'sqlite wal',
                  memberCount: 3,
                  searchEngine: 'google',
                  queries: ['sqlite wal', 'sqlite checkpoint'],
                  firstSeenAt: '2026-04-18T00:00:00Z',
                  lastSeenAt: '2026-04-18T01:00:00Z',
                },
              ],
              total: 1,
              page: 0,
              pageSize: 8,
            },
            hourlyActivity: Array.from({ length: 24 }, (_, hour) => ({
              hour,
              visitCount: hour === 10 ? 4 : 0,
            })),
            drilldown: {
              explorerDateRange: { start: '2026-04-18', end: '2026-04-18' },
            },
          },
          {
            moduleIds: [
              'daily-rollups',
              'search-trails',
              'refind-pages',
              'activity-mix',
            ],
            sourceTables: [
              'daily_summary_rollups',
              'domain_daily_rollups',
              'category_daily_rollups',
              'query_families',
              'refind_pages',
            ],
          },
        ),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/day/:date"
          element={<DayInsightsRoutePage />}
        />
      </Routes>,
      {
        route: '/intelligence/day/2026-04-18?profileId=chrome:Default',
        snapshot,
      },
    )

    expect(await screen.findByTestId('day-insights-page')).toBeVisible()
    expect(dayInsightsSpy).toHaveBeenCalledWith('2026-04-18', 'chrome:Default')
    expect(
      screen.getByRole('link', { name: 'Open exact-day evidence' }),
    ).toHaveAttribute(
      'href',
      '/explorer?profileId=chrome%3ADefault&start=2026-04-18&end=2026-04-18',
    )
    const topSitesSection = screen
      .getByRole('heading', { name: 'Standout Sites' })
      .closest('section')
    if (!(topSitesSection instanceof HTMLElement)) {
      throw new Error('expected standout sites section')
    }
    expect(
      within(topSitesSection).getByRole('link', { name: /SQLite/i }),
    ).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-18&end=2026-04-18&profileId=chrome%3ADefault',
    )
  })

  test('renders query-family insights as a first-class route with related trail links', async () => {
    const { snapshot } = await seedArchiveState()
    const detailSpy = vi
      .spyOn(coreIntelligenceApi, 'getQueryFamilyDetail')
      .mockResolvedValue(
        wrapSection<QueryFamilyDetail>(
          'query-family-detail',
          {
            family: {
              familyId: 'family-1',
              anchorQuery: 'sqlite wal',
              memberCount: 3,
              searchEngine: 'google',
              queries: ['sqlite wal', 'sqlite checkpoint'],
              firstSeenAt: '2026-04-18T00:00:00Z',
              lastSeenAt: '2026-04-18T01:00:00Z',
            },
            relatedTrails: [
              {
                trailId: 'trail-1',
                sessionId: 'session-1',
                initialQuery: 'sqlite wal',
                searchEngine: 'google',
                reformulationCount: 1,
                visitCount: 2,
                landingUrl: 'https://sqlite.org/wal.html',
                landingDomain: 'sqlite.org',
                firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
                lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
                maxDepth: 2,
                queries: ['sqlite wal', 'sqlite checkpoint'],
              },
            ],
          },
          {
            moduleIds: ['search-trails'],
            sourceTables: ['query_families', 'search_trails', 'search_events'],
          },
        ),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/query-family/:familyId"
          element={<QueryFamilyInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/query-family/family-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /sqlite wal/i }),
    ).toBeVisible()
    expect(detailSpy).toHaveBeenCalledWith(
      'family-1',
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
    )
    expect(
      screen.getByRole('link', { name: 'Open evidence in Explorer' }),
    ).toHaveAttribute(
      'href',
      '/explorer?profileId=chrome%3ADefault&start=2026-04-01&end=2026-04-30&q=sqlite+wal',
    )
    expect(screen.getByRole('link', { name: /sqlite wal/i })).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault',
    )
  })

  test('renders refind-page insights with day and trail drilldowns', async () => {
    const { snapshot } = await seedArchiveState()
    const detailSpy = vi
      .spyOn(coreIntelligenceApi, 'getRefindPageDetail')
      .mockResolvedValue(
        wrapSection<RefindPageDetail>(
          'refind-page-detail',
          {
            page: {
              canonicalUrl: 'https://sqlite.org/lang.html',
              url: 'https://sqlite.org/lang.html',
              title: 'SQLite docs',
              registrableDomain: 'sqlite.org',
              crossDayCount: 3,
              trailCount: 2,
              searchArrivalCount: 1,
              typedRevisitCount: 0,
              refindScore: 5,
              firstSeenAt: '2026-04-10T00:00:00Z',
              lastSeenAt: '2026-04-18T00:00:00Z',
            },
            explanation: {
              canonicalUrl: 'https://sqlite.org/lang.html',
              refindScore: 5,
              factors: [
                {
                  signal: 'cross_day_count',
                  rawValue: 3,
                  weight: 3,
                  contribution: 9,
                },
              ],
              visitIds: [101, 102],
            },
            recentDays: ['2026-04-18', '2026-04-12'],
            relatedTrails: [
              {
                trailId: 'trail-1',
                sessionId: 'session-1',
                initialQuery: 'sqlite wal',
                searchEngine: 'google',
                reformulationCount: 1,
                visitCount: 2,
                landingUrl: 'https://sqlite.org/lang.html',
                landingDomain: 'sqlite.org',
                firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
                lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
                maxDepth: 2,
                queries: ['sqlite wal', 'sqlite checkpoint'],
              },
            ],
          },
          {
            moduleIds: ['refind-pages', 'search-trails'],
            sourceTables: [
              'refind_pages',
              'visit_derived_facts',
              'search_trails',
            ],
          },
        ),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/refind/:canonicalUrl"
          element={<RefindPageInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/refind/https%3A%2F%2Fsqlite.org%2Flang.html?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: 'SQLite docs' }),
    ).toBeVisible()
    expect(detailSpy).toHaveBeenCalledWith(
      'https://sqlite.org/lang.html',
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
    )
    expect(screen.getByRole('link', { name: '2026-04-18' })).toHaveAttribute(
      'href',
      '/intelligence/day/2026-04-18?profileId=chrome%3ADefault',
    )
    expect(screen.getByRole('link', { name: /sqlite wal/i })).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault',
    )
  })

  test('renders compare-set insights as a first-class route with focused trail and day links', async () => {
    const { snapshot } = await seedArchiveState()
    const detailSpy = vi
      .spyOn(coreIntelligenceApi, 'getCompareSetDetail')
      .mockResolvedValue(
        wrapSection('compare-set-detail', {
          compareSet: {
            compareSetId: 'compare:trail-1:docs_page',
            trailId: 'trail-1',
            searchQuery: 'sqlite wal',
            pageCategory: 'docs_page',
            pages: [
              {
                canonicalUrl: 'https://sqlite.org/wal.html',
                url: 'https://sqlite.org/wal.html',
                title: 'WAL mode',
                registrableDomain: 'sqlite.org',
                visitCount: 2,
                isLanding: true,
              },
              {
                canonicalUrl: 'https://sqlite.org/checkpoint.html',
                url: 'https://sqlite.org/checkpoint.html',
                title: 'Checkpoint',
                registrableDomain: 'sqlite.org',
                visitCount: 2,
                isLanding: false,
              },
            ],
          },
          trail: {
            trailId: 'trail-1',
            sessionId: 'session-1',
            initialQuery: 'sqlite wal',
            searchEngine: 'google',
            reformulationCount: 2,
            visitCount: 4,
            landingUrl: 'https://sqlite.org/wal.html',
            landingDomain: 'sqlite.org',
            firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
            lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
            maxDepth: 2,
            queries: ['sqlite wal', 'sqlite checkpoint'],
          },
          session: {
            sessionId: 'session-1',
            firstVisitMs: Date.parse('2026-04-18T00:00:00Z'),
            lastVisitMs: Date.parse('2026-04-18T00:10:00Z'),
            visitCount: 5,
            searchCount: 2,
            domainCount: 1,
            isDeepDive: false,
            autoTitle: 'SQLite compare',
          },
          recentDays: ['2026-04-18', '2026-04-12'],
        }),
      )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/compare-set/:compareSetId"
          element={<CompareSetInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/compare-set/compare%3Atrail-1%3Adocs_page?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /sqlite wal/i }),
    ).toBeVisible()
    expect(detailSpy).toHaveBeenCalledWith(
      'compare:trail-1:docs_page',
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
    )
    expect(
      screen.getByRole('link', { name: 'Open trail insights' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
    )
    expect(
      screen
        .getAllByRole('link', { name: 'sqlite.org' })
        .every(
          (link) =>
            link.getAttribute('href') ===
            '/intelligence/domain/sqlite.org?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
        ),
    ).toBe(true)
    expect(
      screen
        .getAllByRole('link', { name: '2026-04-18' })
        .some(
          (link) =>
            link.getAttribute('href') ===
            '/intelligence/day/2026-04-18?profileId=chrome%3ADefault&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
        ),
    ).toBe(true)
  })

  test('renders session insights as a route-first destination while keeping Explorer inline sessions', async () => {
    const { snapshot } = await seedArchiveState()
    vi.spyOn(coreIntelligenceApi, 'getSessionDetail').mockResolvedValue({
      session: {
        sessionId: 'session-1',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:30:00Z'),
        visitCount: 3,
        searchCount: 1,
        domainCount: 2,
        isDeepDive: true,
        autoTitle: 'SQLite WAL research',
      },
      visits: [
        {
          visitId: 101,
          url: 'https://www.sqlite.org/wal.html',
          title: 'SQLite WAL',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:05:00Z'),
          isSearchEvent: false,
          searchQuery: null,
          searchEngine: null,
          trailId: 'trail-1',
          transitionType: 'link',
        },
      ],
      trails: [
        {
          trailId: 'trail-1',
          sessionId: 'session-1',
          initialQuery: 'sqlite wal',
          searchEngine: 'google',
          reformulationCount: 1,
          visitCount: 2,
          landingUrl: 'https://sqlite.org/wal.html',
          landingDomain: 'sqlite.org',
          firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
          lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
          maxDepth: 2,
          queries: ['sqlite wal', 'sqlite checkpoint'],
        },
      ],
    })

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/session/:sessionId"
          element={<SessionInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/session/session-1?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /SQLite WAL research/i }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Open evidence in Explorer' }),
    ).toHaveAttribute(
      'href',
      '/explorer?profileId=chrome%3ADefault&start=2026-04-05&end=2026-04-05',
    )
    expect(screen.getByRole('link', { name: /sqlite wal/i })).toHaveAttribute(
      'href',
      '/intelligence/trail/trail-1?range=custom&start=2026-04-05&end=2026-04-05&profileId=chrome%3ADefault',
    )
  })

  test('renders trail insights with session handoff and member entity links', async () => {
    const { snapshot } = await seedArchiveState()
    vi.spyOn(coreIntelligenceApi, 'getTrailDetail').mockResolvedValue({
      trail: {
        trailId: 'trail-1',
        sessionId: 'session-1',
        initialQuery: 'sqlite wal checkpoint',
        searchEngine: 'Google',
        reformulationCount: 1,
        visitCount: 2,
        landingUrl: 'https://www.sqlite.org/pragma.html',
        landingDomain: 'sqlite.org',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
        maxDepth: 2,
        queries: ['sqlite wal checkpoint', 'sqlite wal checkpoint passive'],
      },
      members: [
        {
          trailId: 'trail-1',
          visitId: 202,
          ordinal: 1,
          role: 'landing',
          url: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          searchQuery: null,
        },
      ],
    })

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/trail/:trailId"
          element={<TrailInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default',
        snapshot,
      },
    )

    expect(
      await screen.findByRole('heading', { name: /sqlite wal checkpoint/i }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Open session insights' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/session/session-1?range=custom&start=2026-04-05&end=2026-04-05&profileId=chrome%3ADefault',
    )
    expect(screen.getByRole('link', { name: 'sqlite.org' })).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-05&end=2026-04-05&profileId=chrome%3ADefault',
    )
  })

  test('shows compare-set focus context inside trail insights and highlights matching members', async () => {
    const { snapshot } = await seedArchiveState()
    vi.spyOn(coreIntelligenceApi, 'getTrailDetail').mockResolvedValue({
      trail: {
        trailId: 'trail-1',
        sessionId: 'session-1',
        initialQuery: 'sqlite wal checkpoint',
        searchEngine: 'Google',
        reformulationCount: 1,
        visitCount: 2,
        landingUrl: 'https://www.sqlite.org/pragma.html',
        landingDomain: 'sqlite.org',
        firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
        lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
        maxDepth: 2,
        queries: ['sqlite wal checkpoint', 'sqlite wal checkpoint passive'],
      },
      members: [
        {
          trailId: 'trail-1',
          visitId: 202,
          ordinal: 1,
          role: 'landing',
          url: 'https://www.sqlite.org/pragma.html',
          canonicalUrl: 'https://www.sqlite.org/pragma.html',
          title: 'PRAGMA wal_checkpoint',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:03:00Z'),
          searchQuery: null,
        },
        {
          trailId: 'trail-1',
          visitId: 203,
          ordinal: 2,
          role: 'click',
          url: 'https://www.sqlite.org/wal.html',
          canonicalUrl: 'https://www.sqlite.org/wal.html',
          title: 'WAL docs',
          registrableDomain: 'sqlite.org',
          visitTimeMs: Date.parse('2026-04-05T14:06:00Z'),
          searchQuery: null,
        },
      ],
    })
    vi.spyOn(coreIntelligenceApi, 'getCompareSetDetail').mockResolvedValue(
      wrapSection('compare-set-detail', {
        compareSet: {
          compareSetId: 'compare:trail-1:docs_page',
          trailId: 'trail-1',
          searchQuery: 'sqlite wal',
          pageCategory: 'docs_page',
          pages: [
            {
              canonicalUrl: 'https://www.sqlite.org/pragma.html',
              url: 'https://www.sqlite.org/pragma.html',
              title: 'PRAGMA wal_checkpoint',
              registrableDomain: 'sqlite.org',
              visitCount: 2,
              isLanding: true,
            },
          ],
        },
        trail: {
          trailId: 'trail-1',
          sessionId: 'session-1',
          initialQuery: 'sqlite wal checkpoint',
          searchEngine: 'Google',
          reformulationCount: 1,
          visitCount: 2,
          landingUrl: 'https://www.sqlite.org/pragma.html',
          landingDomain: 'sqlite.org',
          firstVisitMs: Date.parse('2026-04-05T14:00:00Z'),
          lastVisitMs: Date.parse('2026-04-05T14:20:00Z'),
          maxDepth: 2,
          queries: ['sqlite wal checkpoint', 'sqlite wal checkpoint passive'],
        },
        session: null,
        recentDays: ['2026-04-05'],
      }),
    )

    renderSurface(
      <Routes>
        <Route
          path="/intelligence/trail/:trailId"
          element={<TrailInsightsRoutePage />}
        />
      </Routes>,
      {
        route:
          '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-07&profileId=chrome:Default&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
        snapshot,
      },
    )

    expect(await screen.findByText('Focused compare set')).toBeVisible()
    expect(
      screen
        .getAllByRole('link', { name: '2026-04-05' })
        .some(
          (link) =>
            link.getAttribute('href') ===
            '/intelligence/day/2026-04-05?profileId=chrome%3ADefault&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
        ),
    ).toBe(true)
    expect(
      screen.getByText('PRAGMA wal_checkpoint').closest('.trail-member-row'),
    ).toHaveClass('trail-member-row--focused')
  })

  test('limits path-flow steps to supported values and wires explainability to supported intelligence entities', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
      wrapSection('refind-pages', [
        {
          canonicalUrl: 'https://example.com/reference',
          url: 'https://example.com/reference',
          title: 'Reference page',
          registrableDomain: 'example.com',
          crossDayCount: 4,
          trailCount: 3,
          searchArrivalCount: 2,
          typedRevisitCount: 1,
          refindScore: 0.9,
          firstSeenAt: '2026-04-01T00:00:00Z',
          lastSeenAt: '2026-04-07T00:00:00Z',
        },
      ]),
    )
    vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
      wrapSection('search-activity', {
        families: [],
        total: 0,
        page: 0,
        pageSize: 10,
      }),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getReopenedInvestigations',
    ).mockResolvedValue(wrapSection('reopened-investigations', []))
    vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
      wrapSection('habits', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
      wrapSection('habits', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
      wrapSection('path-flows', []),
    )
    const explainSpy = vi
      .spyOn(coreIntelligenceApi, 'explainEntity')
      .mockResolvedValue({
        entityType: 'refind_page',
        entityId: 'https://example.com/reference',
        triggerRule: 'Refind score >= 0.7',
        factors: [],
        participatingVisitIds: [1, 2],
      })

    renderSurface(<IntelligencePage />, {
      route: '/intelligence?profileId=chrome:Default',
      snapshot,
    })

    expect(
      await screen.findByRole('button', {
        name: intelligenceT('explainTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.queryByText(intelligenceT('pathFlowsStep4')),
    ).not.toBeInTheDocument()

    const refindSection = screen
      .getByText(intelligenceT('refindTitle'))
      .closest('section')
    expect(refindSection).not.toBeNull()
    if (!(refindSection instanceof HTMLElement)) {
      throw new Error('expected refind section')
    }

    await user.click(
      within(refindSection).getByRole('button', {
        name: intelligenceT('explainTitle'),
      }),
    )

    await waitFor(() =>
      expect(explainSpy).toHaveBeenCalledWith(
        'refind_page',
        'https://example.com/reference',
      ),
    )
  })

  test('clears both explorer date bounds in a single interaction', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer?start=2026-04-01&end=2026-04-07',
      snapshot,
    })

    const startInput = await screen.findByLabelText(explorerT('filterStart'))
    const endInput = await screen.findByLabelText(explorerT('filterEnd'))

    expect(startInput).toHaveValue('2026-04-01')
    expect(endInput).toHaveValue('2026-04-07')
    expect(
      screen.getByRole('button', {
        name: explorerT('removeFilter', {
          label: explorerT('filterStart'),
          value: '2026-04-01',
        }),
      }),
    ).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Clear range' }))

    await waitFor(() => {
      expect(startInput).toHaveValue('')
      expect(endInput).toHaveValue('')
    })
  })

  test('keeps the explorer profile control aligned with the shared profile scope', async () => {
    window.localStorage.setItem('pathkeep.profile-scope', 'chrome:Default')

    try {
      const { snapshot } = await seedArchiveState()
      const explorerT = createNamespaceTranslator('en', 'explorer')

      renderSurface(<ExplorerPage />, {
        language: 'en',
        route: '/explorer',
        snapshot,
      })

      expect(
        await screen.findByLabelText(explorerT('filterProfileAria')),
      ).toHaveValue('chrome:Default')
      expect(await screen.findByText(explorerT('scopeInherited'))).toBeVisible()
    } finally {
      window.localStorage.removeItem('pathkeep.profile-scope')
    }
  })

  test('debounces explorer keyword query commits while the user is typing', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const querySpy = vi.spyOn(backend, 'queryHistory')

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()
    await waitFor(() => expect(querySpy).toHaveBeenCalledTimes(1))

    await user.type(
      screen.getByLabelText(explorerT('filterKeywordAria')),
      'sqlite',
    )

    expect(querySpy).toHaveBeenCalledTimes(1)

    await new Promise((resolve) => window.setTimeout(resolve, 220))
    await waitFor(() => expect(querySpy).toHaveBeenCalledTimes(2))
  })

  test('shows the current page count and lets users change explorer rows per page', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedArchiveState()
    const explorerT = createNamespaceTranslator('en', 'explorer')
    const scrollToSpy = vi.fn()
    window.scrollTo = scrollToSpy
    const querySpy = vi
      .spyOn(backend, 'queryHistory')
      .mockImplementation((query) =>
        Promise.resolve({
          total: 240,
          page: query.page ?? 1,
          pageSize: query.limit ?? 50,
          pageCount: Math.ceil(240 / (query.limit ?? 50)),
          hasPrevious: (query.page ?? 1) > 1,
          hasNext: (query.page ?? 1) < Math.ceil(240 / (query.limit ?? 50)),
          nextCursor: null,
          items: [
            {
              id: 1,
              profileId: 'chrome:Default',
              url: 'https://example.com/alpha',
              title: 'Alpha',
              domain: 'example.com',
              visitedAt: '2026-04-17T10:00:00Z',
              visitTime: Date.parse('2026-04-17T10:00:00Z'),
              transition: null,
              favicon: null,
              sourceVisitId: 1,
            },
            {
              id: 2,
              profileId: 'chrome:Default',
              url: 'https://example.com/beta',
              title: 'Beta',
              domain: 'example.com',
              visitedAt: '2026-04-17T11:00:00Z',
              visitTime: Date.parse('2026-04-17T11:00:00Z'),
              transition: null,
              favicon: null,
              sourceVisitId: 2,
            },
          ],
        }),
      )

    renderSurface(<ExplorerPage />, {
      language: 'en',
      route: '/explorer',
      snapshot,
    })

    expect((await screen.findAllByText('Page 1 of 5')).length).toBeGreaterThan(
      2,
    )
    expect(
      screen.getAllByText('Showing 2 of 240 results on this page').length,
    ).toBeGreaterThan(2)
    await waitFor(() =>
      expect(querySpy).toHaveBeenLastCalledWith(
        expect.objectContaining({ limit: 50 }),
      ),
    )

    await user.click(
      screen.getByRole('button', { name: explorerT('nextPage') }),
    )

    await waitFor(() =>
      expect(querySpy).toHaveBeenLastCalledWith(
        expect.objectContaining({ page: 2, limit: 50 }),
      ),
    )
    expect((await screen.findAllByText('Page 2 of 5')).length).toBeGreaterThan(
      2,
    )
    expect(scrollToSpy).not.toHaveBeenCalled()

    await user.selectOptions(
      screen.getByRole('combobox', { name: explorerT('pageSizeLabel') }),
      '100',
    )

    await waitFor(() =>
      expect(querySpy).toHaveBeenLastCalledWith(
        expect.objectContaining({ limit: 100 }),
      ),
    )
    expect((await screen.findAllByText('Page 1 of 3')).length).toBeGreaterThan(
      2,
    )
    expect(scrollToSpy).not.toHaveBeenCalled()
  })

  test('renders search effectiveness as plain-language summaries', async () => {
    const { snapshot } = await seedArchiveState()
    const intelligenceT = createNamespaceTranslator('en', 'intelligence')

    vi.spyOn(coreIntelligenceApi, 'getDigestSummary').mockResolvedValue(
      wrapSection('digest-summary', {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: { value: 10, deltaPct: 0, trend: 'up' },
        totalSearches: { value: 4, deltaPct: 0, trend: 'up' },
        newDomains: { value: 3, deltaPct: 0, trend: 'up' },
        deepReadPages: { value: 2, deltaPct: 0, trend: 'up' },
        refindPages: { value: 1, deltaPct: 0, trend: 'up' },
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getOnThisDay').mockResolvedValue(
      wrapSection('on-this-day', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSites').mockResolvedValue(
      wrapSection('top-sites', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEngineRanking').mockResolvedValue(
      wrapSection('engine-ranking', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getTopSearchConcepts').mockResolvedValue(
      wrapSection('search-concepts', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getQueryFamilies').mockResolvedValue(
      wrapSection('query-families', {
        page: 0,
        pageSize: 20,
        total: 0,
        families: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getRefindPages').mockResolvedValue(
      wrapSection('refind-pages', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getActivityMix').mockResolvedValue(
      wrapSection('activity-mix', {
        categories: [],
        changeVsPrevious: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getBrowsingRhythm').mockResolvedValue(
      wrapSection('browsing-rhythm', {
        cells: [],
        maxCount: 0,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getStableSources').mockResolvedValue(
      wrapSection('stable-sources', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getSearchEffectiveness').mockResolvedValue(
      wrapSection('search-effectiveness', {
        engineStats: [
          {
            searchEngine: 'google',
            displayName: 'Google',
            avgReformulations: 1.2,
            totalTrails: 18,
            avgDepth: 2.4,
          },
        ],
        topResolvingSources: [
          {
            registrableDomain: 'developer.mozilla.org',
            displayName: 'MDN',
            sourceRole: 'landing',
            trailCount: 6,
            stableLandingCount: 6,
            effectivenessScore: 0.9,
          },
        ],
        hardestTopics: [
          {
            familyId: 'family-hardest-1',
            queryFamily: 'sqlite wal checkpoint',
            reformulationCount: 3,
            reSearchLagDays: 2.5,
          },
        ],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getFrictionSignals').mockResolvedValue(
      wrapSection('friction-signals', []),
    )
    vi.spyOn(
      coreIntelligenceApi,
      'getReopenedInvestigations',
    ).mockResolvedValue(wrapSection('reopened-investigations', []))
    vi.spyOn(coreIntelligenceApi, 'getDiscoveryTrend').mockResolvedValue(
      wrapSection('discovery-trend', { points: [], availableYears: [] }),
    )
    vi.spyOn(coreIntelligenceApi, 'getBreadthIndex').mockResolvedValue(
      wrapSection('breadth-index', {
        breadthScore: 42,
        hhi: 0.42,
        concentrationDomainCount: 5,
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue(
      wrapSection('path-flows', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getHabitPatterns').mockResolvedValue(
      wrapSection('habit-patterns', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getInterruptedHabits').mockResolvedValue(
      wrapSection('interrupted-habits', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getCompareSets').mockResolvedValue(
      wrapSection('compare-sets', []),
    )
    vi.spyOn(coreIntelligenceApi, 'getMultiBrowserDiff').mockResolvedValue(
      wrapSection('multi-browser-diff', {
        profiles: [],
        sharedDomains: [],
        exclusiveDomains: [],
        categoryDistributions: [],
      }),
    )
    vi.spyOn(coreIntelligenceApi, 'getObservedInteractions').mockResolvedValue(
      wrapSection('observed-interactions', []),
    )

    renderSurface(<IntelligencePage />, {
      language: 'en',
      route: '/intelligence',
      snapshot,
    })

    expect(
      await screen.findByRole('heading', {
        name: intelligenceT('searchEffectivenessTitle'),
      }),
    ).toBeVisible()
    expect(
      await screen.findByText(
        'Each trail was rewritten about 1.2 times on average.',
      ),
    ).toBeVisible()
    expect(
      await screen.findByText('People usually stopped around depth 2.4.'),
    ).toBeVisible()
    expect(
      await screen.findByText('This window produced 18 search trails.'),
    ).toBeVisible()
    expect(screen.getByText('MDN')).toBeVisible()
    expect(screen.getByText('"sqlite wal checkpoint"')).toBeVisible()
  })
})
