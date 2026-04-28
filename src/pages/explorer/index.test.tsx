/**
 * @file index.test.tsx
 * @description Route-shell coverage for Explorer view selection and blocking states.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Verify the Explorer route shell chooses the right blocking, empty, time, session, and trail branches.
 * - Keep full-route wiring covered without mounting data-heavy child panels.
 *
 * ## Not responsible for
 * - Re-testing panel internals or backend Explorer loaders.
 *
 * ## Dependencies
 * - Mocks shell/profile/i18n hooks and route-local Explorer hooks.
 *
 * ## Performance notes
 * - Fixtures stay bounded to one history row and do not hit IPC.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { ExplorerPage } from './index'

const {
  useExplorerDataMock,
  useExplorerFaviconsMock,
  useExplorerUrlStateMock,
  useProfileScopeMock,
  useShellDataMock,
} = vi.hoisted(() => ({
  useExplorerDataMock: vi.fn(),
  useExplorerFaviconsMock: vi.fn(),
  useExplorerUrlStateMock: vi.fn(),
  useProfileScopeMock: vi.fn(),
  useShellDataMock: vi.fn(),
}))

vi.mock('../../app/shell-data-context', () => ({
  useShellData: useShellDataMock,
}))

vi.mock('../../lib/i18n', () => ({
  useI18n: () => ({
    language: 'en',
    ns: (namespace: string) => (key: string) => `${namespace}.${key}`,
    t: (key: string) => key,
  }),
}))

vi.mock('../../lib/intelligence-ai-presentation', () => ({
  aiStatusMeta: () => ({ label: 'AI ready', tone: 'info' }),
  selectedAiProvider: () => ({ id: 'provider-1', label: 'Local AI' }),
}))

vi.mock('../../lib/backend-client', () => ({
  backend: {
    cancelAiJob: vi.fn(),
    openPathInFileManager: vi.fn(),
    replayAiJob: vi.fn(),
    runAiQueueJobs: vi.fn(),
  },
}))

vi.mock('../../lib/profile-scope-context', () => ({
  profileIdLabel: (profileId: string) =>
    profileId.split(':').at(-1) ?? profileId,
  useProfileScope: useProfileScopeMock,
}))

vi.mock('./hooks/use-explorer-data', () => ({
  useExplorerData: useExplorerDataMock,
}))

vi.mock('./hooks/use-explorer-favicons', () => ({
  useExplorerFavicons: useExplorerFaviconsMock,
}))

vi.mock('./hooks/use-explorer-url-state', () => ({
  useExplorerUrlState: useExplorerUrlStateMock,
}))

vi.mock('./panels/results-panel', () => ({
  ExplorerResultsPanel: () => <div data-testid="results-panel">results</div>,
}))

vi.mock('./panels/runtime-panel', () => ({
  ExplorerRuntimePanel: () => <div data-testid="runtime-panel">runtime</div>,
}))

vi.mock('./panels/semantic-panel', () => ({
  ExplorerSemanticPanel: () => <div data-testid="semantic-panel">semantic</div>,
}))

vi.mock('./panels/session-group', () => ({
  SessionGroupPanel: () => <div data-testid="session-panel">session</div>,
}))

vi.mock('./panels/trail-group', () => ({
  TrailGroupPanel: () => <div data-testid="trail-panel">trail</div>,
}))

vi.mock('./panels/detail-panel', () => ({
  ExplorerDetailPanel: () => <div data-testid="detail-panel">detail</div>,
}))

vi.mock('./query-filters-panel', () => ({
  ExplorerQueryFiltersPanel: () => (
    <div data-testid="filters-panel">filters</div>
  ),
}))

vi.mock('./timeline-bar', () => ({
  ExplorerTimelineBar: () => <div data-testid="timeline-bar">timeline</div>,
}))

describe('ExplorerPage route shell', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useShellDataMock.mockReturnValue(defaultShellData())
    useProfileScopeMock.mockReturnValue({ activeProfileId: 'chrome:Default' })
    useExplorerFaviconsMock.mockReturnValue({ faviconCache: new Map() })
    useExplorerUrlStateMock.mockReturnValue(defaultUrlState())
    useExplorerDataMock.mockImplementation(defaultExplorerData)
  })

  test('renders invalid regex before history results', () => {
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        queryInput: '(',
        regexMode: true,
        regexValid: false,
      }),
    )

    renderExplorer()

    expect(screen.getByText('explorer.regexInvalid')).toBeVisible()
    expect(screen.queryByTestId('results-panel')).not.toBeInTheDocument()
  })

  test('renders query errors and empty history branches', () => {
    useExplorerDataMock.mockImplementation(
      (options: Parameters<typeof defaultExplorerData>[0]) =>
        defaultExplorerData(options, {
          queryState: {
            error: 'query exploded',
            requestKey: options.requestKey,
            results: null,
          },
        }),
    )
    const { rerender } = renderExplorer()

    expect(screen.getByText('query exploded')).toBeVisible()

    useExplorerDataMock.mockImplementation(
      (options: Parameters<typeof defaultExplorerData>[0]) =>
        defaultExplorerData(options, {
          queryState: {
            error: null,
            requestKey: options.requestKey,
            results: {
              items: [],
              page: 1,
              pageCount: 1,
              total: 0,
            },
          },
        }),
    )
    rerender(<ExplorerWrapper />)

    expect(screen.getByText('explorer.noMatchesTitle')).toBeVisible()
  })

  test('renders time, session, and trail branches', () => {
    const { rerender } = renderExplorer()
    expect(screen.getByTestId('results-panel')).toBeVisible()

    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({ view: 'session' }),
    )
    rerender(<ExplorerWrapper />)
    expect(screen.getByTestId('session-panel')).toBeVisible()
    expect(screen.getByTestId('detail-panel')).toBeVisible()

    useExplorerUrlStateMock.mockReturnValue(defaultUrlState({ view: 'trail' }))
    rerender(<ExplorerWrapper />)
    expect(screen.getByTestId('trail-panel')).toBeVisible()
    expect(screen.getByTestId('detail-panel')).toBeVisible()

    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({ view: 'unknown-view' }),
    )
    rerender(<ExplorerWrapper />)
    expect(screen.queryByTestId('trail-panel')).not.toBeInTheDocument()
    expect(screen.queryByTestId('session-panel')).not.toBeInTheDocument()
  })
})

function renderExplorer() {
  return render(<ExplorerWrapper />)
}

function ExplorerWrapper() {
  return (
    <MemoryRouter>
      <ExplorerPage />
    </MemoryRouter>
  )
}

function defaultShellData() {
  return {
    error: null,
    loading: false,
    refreshAppData: vi.fn(),
    refreshKey: 1,
    refreshRuntimeStatus: vi.fn(),
    runtimeStatus: {
      aiQueue: null,
      error: null,
      intelligence: null,
      loading: false,
    },
    snapshot: {
      aiStatus: { state: 'ready' },
      archiveStatus: { unlocked: true },
      config: {
        ai: { providers: [] },
        explorerBackgroundPrefetchPages: 1,
        initialized: true,
        selectedProfileIds: ['chrome:Default'],
      },
    },
  }
}

function defaultUrlState(overrides: Record<string, unknown> = {}) {
  return {
    activeDateShortcut: () => 'month',
    activeFilters: [],
    applyDateShortcut: vi.fn(),
    browserKinds: [],
    buildRecentSearchLabel: vi.fn(),
    clearAllFilters: vi.fn(),
    clearDateRange: vi.fn(),
    currentQuery: { query: 'sqlite' },
    end: '2026-04-30',
    explicitPage: null,
    explicitProfileId: null,
    groupedDateRange: { start: '2026-04-01', end: '2026-04-30' },
    handleFirstHistoryPage: vi.fn(),
    handleHistoryPageJump: vi.fn(),
    handleLastHistoryPage: vi.fn(),
    handleNextHistoryPage: vi.fn(),
    handleNextSemanticPage: vi.fn(),
    handlePreviousHistoryPage: vi.fn(),
    handlePreviousSemanticPage: vi.fn(),
    historyPageInput: '1',
    mode: 'keyword',
    pageSize: 20,
    persistRecentSearch: vi.fn(),
    profileId: null,
    queryInput: 'sqlite',
    recentSearches: [],
    regexMode: false,
    regexValid: true,
    searchParams: new URLSearchParams(),
    semanticQuery: { query: '' },
    semanticTrail: [],
    setHistoryPageInput: vi.fn(),
    setHistoryPageSize: vi.fn(),
    setQueryInput: vi.fn(),
    setRecentSearches: vi.fn(),
    setSearchParams: vi.fn(),
    setView: vi.fn(),
    start: '2026-04-01',
    updateParam: vi.fn(),
    view: 'time',
    ...overrides,
  }
}

function defaultExplorerData(
  options: { requestKey: string; semanticRequestKey: string },
  overrides: Record<string, unknown> = {},
) {
  return {
    actionError: null,
    cachedHistoryResults: null,
    copyFeedback: null,
    exportResult: null,
    handleCopyExportPath: vi.fn(),
    handleExport: vi.fn(),
    handleIndexAction: vi.fn(),
    handleProviderProbe: vi.fn(),
    handleQueueAction: vi.fn(),
    handleVisit: vi.fn(),
    indexAction: null,
    intelligenceError: null,
    providerProbe: null,
    queryState: {
      error: null,
      requestKey: options.requestKey,
      results: {
        items: [
          {
            favicon: null,
            id: 'visit-1',
            profileId: 'chrome:Default',
            title: 'SQLite notes',
            url: 'https://example.test/sqlite',
            visitTime: '2026-04-25T12:00:00.000Z',
          },
        ],
        page: 1,
        pageCount: 1,
        total: 1,
      },
    },
    queueAction: null,
    selectedId: null,
    semanticState: {
      error: null,
      requestKey: options.semanticRequestKey,
      results: null,
    },
    setQueueAction: vi.fn(),
    setSelectedId: vi.fn(),
    ...overrides,
  }
}
