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
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { ExplorerPage } from './index'

const {
  aiStatusMetaMock,
  optionalAiFeaturesAvailableState,
  selectedAiProviderMock,
  useExplorerDataMock,
  useExplorerFaviconsMock,
  useExplorerUrlStateMock,
  useProfileScopeMock,
  useShellDataMock,
} = vi.hoisted(() => ({
  aiStatusMetaMock: vi.fn(),
  optionalAiFeaturesAvailableState: { value: false },
  selectedAiProviderMock: vi.fn(),
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
  aiStatusMeta: aiStatusMetaMock,
  selectedAiProvider: selectedAiProviderMock,
}))

vi.mock('../../lib/release-capabilities', () => ({
  get optionalAiFeaturesAvailable() {
    return optionalAiFeaturesAvailableState.value
  },
}))

vi.mock('../../lib/backend-client', () => ({
  backend: {
    cancelAiJob: vi.fn(),
    openPathInFileManager: vi.fn(),
    replayAiJob: vi.fn(),
    runAiQueueJobs: vi.fn(),
    loadHistoryOgImages: vi.fn().mockResolvedValue([]),
    markOgImagesShown: vi.fn().mockResolvedValue(undefined),
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

// Mock the paper surfaces so each test can fire their callbacks via
// dedicated trigger buttons. The real components are exhaustively tested
// in src/components/explorer-paper/.
vi.mock('./paper-view', () => ({
  PaperExplorerView: (props: {
    onSelectEntry: (entry: { id: number }) => void
    onJumpToDate: (iso: string) => void
    onClearTarget: () => void
  }) => (
    <div data-testid="explorer-paper-view">
      <button
        type="button"
        data-testid="paper-view-select"
        onClick={() => props.onSelectEntry({ id: 42 })}
      >
        select
      </button>
      <button
        type="button"
        data-testid="paper-view-jump"
        onClick={() => props.onJumpToDate('2026-04-15')}
      >
        jump
      </button>
      <button
        type="button"
        data-testid="paper-view-clear-target"
        onClick={() => props.onClearTarget()}
      >
        clear
      </button>
    </div>
  ),
}))

vi.mock('./paper-search-panel', () => ({
  PaperSearchPanel: (props: {
    onQueryChange: (next: string) => void
    onModeChange: (next: { mode: string; regexMode: boolean }) => void
    onSubmit: (query: string) => void
    onSelectEntry: (id: number) => void
    onSeeInContext: (entry: { id: string }, dayDate: string) => void
  }) => (
    <div data-testid="paper-search-panel">
      <button
        type="button"
        data-testid="paper-search-change"
        onClick={() => props.onQueryChange('next-query')}
      >
        change
      </button>
      <button
        type="button"
        data-testid="paper-search-mode"
        onClick={() =>
          props.onModeChange({ mode: 'semantic', regexMode: true })
        }
      >
        mode
      </button>
      <button
        type="button"
        data-testid="paper-search-mode-keyword"
        onClick={() =>
          props.onModeChange({ mode: 'keyword', regexMode: false })
        }
      >
        keyword
      </button>
      <button
        type="button"
        data-testid="paper-search-submit"
        onClick={() => props.onSubmit('committed')}
      >
        submit
      </button>
      <button
        type="button"
        data-testid="paper-search-select"
        onClick={() => props.onSelectEntry(7)}
      >
        select
      </button>
      <button
        type="button"
        data-testid="paper-search-see-in-context"
        onClick={() => props.onSeeInContext({ id: '17' }, '2026-04-15')}
      >
        see-in-context
      </button>
    </div>
  ),
}))

describe('ExplorerPage route shell', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    optionalAiFeaturesAvailableState.value = false
    aiStatusMetaMock.mockReturnValue({ label: 'AI ready', tone: 'info' })
    selectedAiProviderMock.mockReturnValue({
      id: 'provider-1',
      label: 'Local AI',
    })
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
    expect(screen.queryByTestId('explorer-paper-view')).not.toBeInTheDocument()
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
    // Time view defaults to the paper contact-sheet now.
    expect(screen.getByTestId('explorer-paper-view')).toBeVisible()

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

  test('defaults to the paper contact-sheet view as the only Browse surface', () => {
    renderExplorer()

    expect(screen.getByTestId('explorer-paper-view')).toBeVisible()
  })

  test('shows the paper Search panel when surface=search is in the URL', () => {
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        searchParams: new URLSearchParams('surface=search'),
      }),
    )

    renderExplorer()

    // The PaperSearchPanel renders its own paper-search-view shell instead
    // of the contact-sheet Browse layout.
    expect(screen.queryByTestId('explorer-paper-view')).not.toBeInTheDocument()
    expect(screen.getByTestId('paper-search-panel')).toBeVisible()
  })

  test('PaperSearchPanel callbacks drive the route url-state + selection setters', async () => {
    const user = userEvent.setup()
    const setQueryInput = vi.fn()
    const updateParam = vi.fn()
    const setSearchParams = vi.fn()
    const setSelectedId = vi.fn()
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        searchParams: new URLSearchParams('surface=search&q=initial'),
        setQueryInput,
        updateParam,
        setSearchParams,
      }),
    )
    useExplorerDataMock.mockImplementation(
      (options: Parameters<typeof defaultExplorerData>[0]) =>
        defaultExplorerData(options, {
          setSelectedId,
        }),
    )

    renderExplorer()
    expect(screen.getByTestId('paper-search-panel')).toBeVisible()

    await user.click(screen.getByTestId('paper-search-change'))
    expect(setQueryInput).toHaveBeenCalledWith('next-query')
    expect(updateParam).toHaveBeenCalledWith('q', 'next-query')

    await user.click(screen.getByTestId('paper-search-mode'))
    expect(updateParam).toHaveBeenCalledWith('mode', 'semantic')
    expect(updateParam).toHaveBeenCalledWith('regex', '1')

    await user.click(screen.getByTestId('paper-search-mode-keyword'))
    // keyword mode collapses back to the default (passes null on both).
    expect(updateParam).toHaveBeenCalledWith('mode', null)
    expect(updateParam).toHaveBeenCalledWith('regex', null)

    await user.click(screen.getByTestId('paper-search-submit'))
    expect(setQueryInput).toHaveBeenCalledWith('committed')
    expect(updateParam).toHaveBeenCalledWith('q', 'committed')

    await user.click(screen.getByTestId('paper-search-select'))
    expect(setSelectedId).toHaveBeenCalledWith(7)

    await user.click(screen.getByTestId('paper-search-see-in-context'))
    expect(setSearchParams).toHaveBeenCalled()
    const nextParams = setSearchParams.mock.calls.at(-1)?.[0] as URLSearchParams
    expect(nextParams.get('date')).toBe('2026-04-15')
    expect(nextParams.get('source')).toBe('search')
    expect(nextParams.get('q')).toBeNull()
    expect(nextParams.get('surface')).toBeNull()
    expect(setSelectedId).toHaveBeenCalledWith(17)
  })

  test('PaperExplorerView callbacks drive the route selection + url-state setters', async () => {
    const user = userEvent.setup()
    const setSelectedId = vi.fn()
    const setSearchParams = vi.fn()
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        searchParams: new URLSearchParams('date=2026-04-01&source=search'),
        setSearchParams,
      }),
    )
    useExplorerDataMock.mockImplementation(
      (options: Parameters<typeof defaultExplorerData>[0]) =>
        defaultExplorerData(options, {
          setSelectedId,
        }),
    )
    renderExplorer()
    expect(screen.getByTestId('explorer-paper-view')).toBeVisible()

    await user.click(screen.getByTestId('paper-view-select'))
    expect(setSelectedId).toHaveBeenCalledWith(42)

    await user.click(screen.getByTestId('paper-view-jump'))
    const jumpedParams = setSearchParams.mock.calls.at(
      -1,
    )?.[0] as URLSearchParams
    expect(jumpedParams.get('date')).toBe('2026-04-15')

    await user.click(screen.getByTestId('paper-view-clear-target'))
    const cleared = setSearchParams.mock.calls.at(-1)?.[0] as URLSearchParams
    expect(cleared.get('date')).toBeNull()
    expect(cleared.get('source')).toBeNull()
  })

  test('shows the deferred semantic callout when optional AI is unavailable', () => {
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        mode: 'semantic',
      }),
    )

    renderExplorer()

    expect(screen.getByText('explorer.optionalAiDeferredTitle')).toBeVisible()
    expect(screen.queryByTestId('runtime-panel')).not.toBeInTheDocument()
    expect(screen.queryByTestId('semantic-panel')).not.toBeInTheDocument()
  })

  test('shows fixable optional-AI repair copy for missing, failed, and disabled providers', () => {
    optionalAiFeaturesAvailableState.value = true
    selectedAiProviderMock.mockReturnValue(null)

    const { rerender } = renderExplorer()

    expect(screen.getByText('explorer.optionalAiNoProviderTitle')).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'explorer.optionalAiOpenSettings' }),
    ).toHaveAttribute('href', '/settings')

    selectedAiProviderMock.mockReturnValue({
      id: 'provider-1',
      label: 'Local AI',
    })
    useShellDataMock.mockReturnValue(
      defaultShellData({
        snapshot: {
          ...defaultShellData().snapshot,
          aiStatus: { state: 'failed' },
        },
      }),
    )
    rerender(<ExplorerWrapper />)
    expect(
      screen.getByText('explorer.optionalAiProviderErrorTitle'),
    ).toBeVisible()

    useShellDataMock.mockReturnValue(
      defaultShellData({
        snapshot: {
          ...defaultShellData().snapshot,
          config: {
            ...defaultShellData().snapshot.config,
            ai: {
              ...defaultShellData().snapshot.config.ai,
              enabled: false,
              semanticIndexEnabled: true,
            },
          },
        },
      }),
    )
    rerender(<ExplorerWrapper />)
    expect(screen.getByText('explorer.optionalAiDisabledTitle')).toBeVisible()
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

function defaultShellData(overrides: Record<string, unknown> = {}) {
  const shellData = {
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
        ai: {
          enabled: true,
          providers: [],
          semanticIndexEnabled: true,
        },
        explorerBackgroundPrefetchPages: 1,
        initialized: true,
        selectedProfileIds: ['chrome:Default'],
      },
    },
  }
  return {
    ...shellData,
    ...overrides,
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
