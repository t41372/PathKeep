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

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { ExplorerPage } from './index'

const {
  aiStatusMetaMock,
  optionalAiFeaturesAvailableState,
  desktopCommandTransportAvailable,
  desktopAnnotationsMock,
  localAnnotationsMock,
  selectedAiProviderMock,
  useExplorerDataMock,
  useExplorerFaviconsMock,
  useExplorerOgImagesMock,
  useExplorerUrlStateMock,
  useProfileScopeMock,
  useShellDataMock,
} = vi.hoisted(() => ({
  aiStatusMetaMock: vi.fn(),
  desktopAnnotationsMock: vi.fn(),
  desktopCommandTransportAvailable: { value: false },
  localAnnotationsMock: vi.fn(),
  optionalAiFeaturesAvailableState: { value: false },
  selectedAiProviderMock: vi.fn(),
  useExplorerDataMock: vi.fn(),
  useExplorerFaviconsMock: vi.fn(),
  useExplorerOgImagesMock: vi.fn(),
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

vi.mock('../../lib/runtime', () => ({
  hasDesktopCommandTransport: () => desktopCommandTransportAvailable.value,
}))

vi.mock('../../lib/backend-client', () => ({
  backend: {
    cancelAiJob: vi.fn(),
    openPathInFileManager: vi.fn(),
    replayAiJob: vi.fn(),
    runAiQueueJobs: vi.fn(),
    loadHistoryOgImages: vi.fn().mockResolvedValue([]),
    markOgImagesShown: vi.fn().mockResolvedValue(undefined),
    triggerOgImageRefetch: vi.fn().mockResolvedValue(0),
    getStarStatus: vi.fn().mockResolvedValue({}),
    setStar: vi.fn().mockResolvedValue(undefined),
    unsetStar: vi.fn().mockResolvedValue(undefined),
    listStars: vi.fn().mockResolvedValue([]),
    getStarCounts: vi.fn().mockResolvedValue({ urls: 0, domains: 0 }),
    listVisitEnrichment: vi.fn().mockResolvedValue([]),
    contentFetchNow: vi
      .fn()
      .mockResolvedValue({ jobId: 1, state: 'queued', note: 'queued' }),
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

vi.mock('./hooks/use-explorer-og-images', () => ({
  useExplorerOgImages: useExplorerOgImagesMock,
}))

vi.mock('./hooks/use-explorer-url-state', () => ({
  useExplorerUrlState: useExplorerUrlStateMock,
}))

vi.mock('./use-local-annotations', () => ({
  useLocalAnnotations: localAnnotationsMock,
}))

vi.mock('./use-desktop-annotations', () => ({
  useDesktopAnnotations: desktopAnnotationsMock,
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

vi.mock('../../components/explorer-paper', () => ({
  PaperFilterStrip: (props: {
    onApply: (next: {
      domain: string
      browserKind: string
      profileId: string
      start: string
      end: string
      regexMode: boolean
    }) => void
    onRemove: (id: string) => void
    onClearAll: () => void
  }) => (
    <div data-testid="paper-filter-strip">
      <button
        type="button"
        data-testid="paper-filter-apply-filled"
        onClick={() =>
          props.onApply({
            domain: ' example.com ',
            browserKind: ' chromium ',
            profileId: ' chrome:Default ',
            start: ' 2026-04-01 ',
            end: ' 2026-04-30 ',
            regexMode: true,
          })
        }
      >
        apply-filled
      </button>
      <button
        type="button"
        data-testid="paper-filter-apply-empty"
        onClick={() =>
          props.onApply({
            domain: ' ',
            browserKind: '',
            profileId: ' ',
            start: '',
            end: ' ',
            regexMode: false,
          })
        }
      >
        apply-empty
      </button>
      <button
        type="button"
        data-testid="paper-filter-remove-domain"
        onClick={() => props.onRemove('domain')}
      >
        remove
      </button>
      <button
        type="button"
        data-testid="paper-filter-clear"
        onClick={() => props.onClearAll()}
      >
        clear
      </button>
    </div>
  ),
  PaperStarredView: (props: {
    items: { entityKind: string; entityKey: string }[]
    onSelect?: (item: { entityKind: string; entityKey: string }) => void
    onToggleStar: (item: { entityKind: string; entityKey: string }) => void
  }) => (
    <div data-testid="paper-starred-view">
      <button
        type="button"
        data-testid="paper-starred-select-page"
        onClick={() =>
          props.onSelect?.({
            entityKind: 'url',
            entityKey: 'https://example.com/starred',
          })
        }
      >
        select-page
      </button>
      <button
        type="button"
        data-testid="paper-starred-select-source"
        onClick={() =>
          props.onSelect?.({ entityKind: 'domain', entityKey: 'example.com' })
        }
      >
        select-source
      </button>
      <button
        type="button"
        data-testid="paper-starred-toggle"
        onClick={() =>
          props.onToggleStar({
            entityKind: 'url',
            entityKey: 'https://example.com/starred',
          })
        }
      >
        toggle
      </button>
    </div>
  ),
}))

// Mock the paper surfaces so each test can fire their callbacks via
// dedicated trigger buttons. The real components are exhaustively tested
// in src/components/explorer-paper/.
vi.mock('./paper-view', () => ({
  PaperExplorerView: (props: {
    entries: Array<{
      id: number
      favicon?: string | null
      ogImage?: string | null
    }>
    filterStripSlot?: React.ReactNode
    infiniteScroll?: {
      loadingMore: boolean
      canLoadMore: boolean
      totalPages: number
      totalRows: number
    }
    onSelectEntry: (entry: { id: number }) => void
    onJumpToDate: (iso: string) => void
    onClearTarget: () => void
    entryStar?: {
      isStarred: (url: string) => boolean
      onToggle: (url: string) => void
      starLabel: string
      unstarLabel: string
    }
  }) => (
    <div
      data-testid="explorer-paper-view"
      data-entry-star-state={
        props.entryStar
          ? `${props.entryStar.isStarred('https://example.com/row')}:${props.entryStar.starLabel}`
          : 'no-star'
      }
    >
      {props.filterStripSlot}
      <button
        type="button"
        data-testid="paper-view-toggle-star"
        onClick={() => props.entryStar?.onToggle('https://example.com/row')}
      >
        star
      </button>
      <span data-testid="paper-view-entry-count">{props.entries.length}</span>
      <span data-testid="paper-view-first-og-image">
        {props.entries[0]?.ogImage ?? 'none'}
      </span>
      <span data-testid="paper-view-first-favicon">
        {props.entries[0]?.favicon ?? 'none'}
      </span>
      <span data-testid="paper-view-infinite-state">
        {props.infiniteScroll
          ? `infinite:${props.infiniteScroll.totalRows}/${props.infiniteScroll.totalPages}`
          : 'paginated'}
      </span>
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

vi.mock('./paper-detail-panel-mount', () => ({
  PaperDetailPanelMount: (props: {
    annotations: {
      notesFor: (url: string) => string
    }
    onClose: () => void
    onOpen: (url: string) => void
    onOpenDomain: (domain: string) => void
    stars?: {
      isStarred: (url: string) => boolean
      onToggleStar: (url: string) => void
    }
  }) => (
    <div
      data-testid="paper-detail-mount"
      data-annotation-source={props.annotations.notesFor('__source__')}
      data-detail-star={String(
        props.stars?.isStarred('https://example.com/open') ?? 'no-star',
      )}
    >
      <button
        type="button"
        data-testid="paper-detail-close"
        onClick={() => props.onClose()}
      >
        close
      </button>
      <button
        type="button"
        data-testid="paper-detail-open"
        onClick={() => props.onOpen('https://example.com/open')}
      >
        open
      </button>
      <button
        type="button"
        data-testid="paper-detail-toggle-star"
        onClick={() => props.stars?.onToggleStar('https://example.com/open')}
      >
        star
      </button>
      <button
        type="button"
        data-testid="paper-detail-open-domain"
        onClick={() => props.onOpenDomain('example.com')}
      >
        domain
      </button>
    </div>
  ),
}))

vi.mock('./paper-search-panel', () => ({
  PaperSearchPanel: (props: {
    entries: Array<{ id: number }>
    totalResults: number
    onQueryChange: (next: string) => void
    onModeChange: (next: { mode: string; regexMode: boolean }) => void
    onSubmit: (query: string) => void
    onSelectEntry: (id: number) => void
    onSeeInContext: (entry: { id: string }, dayDate: string) => void
    aboveResultsCallout?: {
      tone: string
      eyebrow: string
      title: string
      body: string
    } | null
    entryStar?: {
      isStarred: (url: string) => boolean
      onToggle: (url: string) => void
    }
  }) => (
    <div
      data-testid="paper-search-panel"
      data-search-entry-count={props.entries.length}
      data-search-total={props.totalResults}
      data-search-star={String(
        props.entryStar?.isStarred('https://example.com/result') ?? 'no-star',
      )}
    >
      <button
        type="button"
        data-testid="paper-search-toggle-star"
        onClick={() => props.entryStar?.onToggle('https://example.com/result')}
      >
        star
      </button>
      {props.aboveResultsCallout ? (
        <div data-testid="paper-search-above-results-callout">
          <span data-testid="paper-search-callout-title">
            {props.aboveResultsCallout.title}
          </span>
          <span data-testid="paper-search-callout-body">
            {props.aboveResultsCallout.body}
          </span>
          <span data-testid="paper-search-callout-tone">
            {props.aboveResultsCallout.tone}
          </span>
        </div>
      ) : null}
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
        data-testid="paper-search-select-starred"
        onClick={() => props.onSelectEntry(-1)}
      >
        select-starred
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
    desktopCommandTransportAvailable.value = false
    optionalAiFeaturesAvailableState.value = false
    aiStatusMetaMock.mockReturnValue({ label: 'AI ready', tone: 'info' })
    selectedAiProviderMock.mockReturnValue({
      id: 'provider-1',
      label: 'Local AI',
    })
    useShellDataMock.mockReturnValue(defaultShellData())
    useProfileScopeMock.mockReturnValue({ activeProfileId: 'chrome:Default' })
    localAnnotationsMock.mockReturnValue(annotationStore('local'))
    desktopAnnotationsMock.mockReturnValue(annotationStore('desktop'))
    useExplorerFaviconsMock.mockReturnValue({ faviconCache: new Map() })
    useExplorerOgImagesMock.mockReturnValue({ ogImageCache: new Map() })
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

  test('mounts the paper Search panel at /search even without ?surface=search', () => {
    // `/search` should mount the same ExplorerPage component but treat
    // the route pathname as the search-surface signal — without this,
    // clicking the sidebar Search item silently fell back to the
    // contact-sheet Browse layout.
    renderExplorer({ initialPath: '/search' })

    expect(screen.queryByTestId('explorer-paper-view')).not.toBeInTheDocument()
    expect(screen.getByTestId('paper-search-panel')).toBeVisible()
  })

  test('zero-result search keeps the composer mounted (no full-screen EmptyState hijack)', () => {
    // feedback-2026-05-25 §3.2 B — previously the empty-result branch
    // unmounted PaperSearchPanel and replaced it with a full-screen
    // EmptyState that trapped the user (they could not edit the
    // misspelt query because the composer was gone).
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        searchParams: new URLSearchParams('surface=search&q=missspelled'),
      }),
    )
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

    renderExplorer()

    expect(screen.getByTestId('paper-search-panel')).toBeVisible()
    expect(
      screen.queryByText('explorer.noMatchesTitle'),
    ).not.toBeInTheDocument()
  })

  test('query error on the search surface renders an in-place callout, not a full-screen ErrorState', () => {
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        searchParams: new URLSearchParams('surface=search&q=boom'),
      }),
    )
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

    renderExplorer()

    expect(screen.getByTestId('paper-search-panel')).toBeVisible()
    expect(screen.getByTestId('paper-search-callout-body')).toHaveTextContent(
      'query exploded',
    )
    expect(screen.getByTestId('paper-search-callout-tone')).toHaveTextContent(
      'blocked',
    )
  })

  test('invalid regex on the search surface renders an in-place callout, not a full-screen StatusCallout', () => {
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        queryInput: '(',
        regexMode: true,
        regexValid: false,
        searchParams: new URLSearchParams('surface=search&q=('),
      }),
    )

    renderExplorer()

    expect(screen.getByTestId('paper-search-panel')).toBeVisible()
    expect(screen.getByTestId('paper-search-callout-title')).toHaveTextContent(
      'explorer.regexInvalid',
    )
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
    // Sticky filters from earlier links (Dashboard's "On this day", palette
    // search) must NOT survive a calendar jump — the user expects the full
    // day, not the 50-row filtered slice.
    expect(jumpedParams.get('source')).toBeNull()
    expect(jumpedParams.get('q')).toBeNull()

    await user.click(screen.getByTestId('paper-view-clear-target'))
    const cleared = setSearchParams.mock.calls.at(-1)?.[0] as URLSearchParams
    expect(cleared.get('date')).toBeNull()
    expect(cleared.get('source')).toBeNull()
  })

  test('Browse rows expose a star toggle that writes through set_star', async () => {
    const user = userEvent.setup()
    renderExplorer()
    expect(screen.getByTestId('explorer-paper-view')).toHaveAttribute(
      'data-entry-star-state',
      'false:explorer.star.starPageAria',
    )
    await user.click(screen.getByTestId('paper-view-toggle-star'))
    await waitFor(() =>
      expect(backend.setStar).toHaveBeenCalledWith({
        entityKind: 'url',
        entityKey: 'https://example.com/row',
      }),
    )
  })

  test('the detail panel mount receives a star provider', async () => {
    const user = userEvent.setup()
    useExplorerDataMock.mockImplementation(
      (options: Parameters<typeof defaultExplorerData>[0]) =>
        defaultExplorerData(options, { selectedId: 42 }),
    )
    renderExplorer()
    // Open the panel so the mount renders.
    await user.click(screen.getByTestId('paper-view-select'))
    const mount = await screen.findByTestId('paper-detail-mount')
    expect(mount).toHaveAttribute('data-detail-star', 'false')
    await user.click(screen.getByTestId('paper-detail-toggle-star'))
    await waitFor(() =>
      expect(backend.setStar).toHaveBeenCalledWith({
        entityKind: 'url',
        entityKey: 'https://example.com/open',
      }),
    )
  })

  test('the Search panel receives a star provider', async () => {
    const user = userEvent.setup()
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        searchParams: new URLSearchParams('surface=search&q=initial'),
      }),
    )
    renderExplorer()
    const panel = await screen.findByTestId('paper-search-panel')
    expect(panel).toHaveAttribute('data-search-star', 'false')
    await user.click(screen.getByTestId('paper-search-toggle-star'))
    await waitFor(() =>
      expect(backend.setStar).toHaveBeenCalledWith({
        entityKind: 'url',
        entityKey: 'https://example.com/result',
      }),
    )
  })

  test('is:starred facet renders the TRUE starred set from list_stars with an honest total', async () => {
    const user = userEvent.setup()
    // No starred pages → the facet shows an empty, honest result (0 entries,
    // 0 total), not a misleading slice of the loaded keyword page.
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        queryInput: 'is:starred',
        searchParams: new URLSearchParams('surface=search&q=is%3Astarred'),
      }),
    )
    const { unmount } = renderExplorer()
    const panel = await screen.findByTestId('paper-search-panel')
    expect(panel).toHaveAttribute('data-search-entry-count', '0')
    expect(panel).toHaveAttribute('data-search-total', '0')
    unmount()

    // With two starred URL pages in `list_stars`, the facet renders BOTH (even
    // though neither is in the loaded keyword page) and reports an honest total
    // of 2 — the count is the size of the starred set, never a wrong page slice.
    ;(backend.listStars as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        entityKind: 'url',
        entityKey: 'https://starred.test/one',
        starredAt: '2026-04-02T00:00:00Z',
        domain: 'starred.test',
        title: 'One',
        visitCount: 3,
      },
      // Degenerate item: no title, no domain, unparseable starredAt — the
      // HistoryEntry adapter must fall back to the key for title/domain and to 0
      // for visitTime without throwing.
      {
        entityKind: 'url',
        entityKey: 'https://starred.test/two',
        starredAt: 'not-a-date',
        domain: '',
        title: '',
        visitCount: 9,
      },
      // A domain star is excluded from the page facet.
      {
        entityKind: 'domain',
        entityKey: 'starred.test',
        starredAt: '2026-04-03T00:00:00Z',
        domain: 'starred.test',
        title: '',
        visitCount: 12,
      },
    ])
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        queryInput: 'is:starred',
        searchParams: new URLSearchParams('surface=search&q=is%3Astarred'),
      }),
    )
    renderExplorer()
    await waitFor(() =>
      expect(screen.getByTestId('paper-search-panel')).toHaveAttribute(
        'data-search-entry-count',
        '2',
      ),
    )
    expect(screen.getByTestId('paper-search-panel')).toHaveAttribute(
      'data-search-total',
      '2',
    )
    // Selecting an id that isn't in the starred set is a safe no-op (no detail
    // panel binds to a synthetic id, and no visit fires).
    await user.click(screen.getByTestId('paper-search-select'))
    expect(screen.queryByTestId('paper-detail-mount')).toBeNull()
  })

  test('selecting an is:starred row opens it via handleVisit (synthetic ids)', async () => {
    const user = userEvent.setup()
    const handleVisit = vi.fn()
    ;(backend.listStars as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        entityKind: 'url',
        entityKey: 'https://starred.test/one',
        starredAt: '2026-04-02T00:00:00Z',
        domain: 'starred.test',
        title: 'One',
        visitCount: 3,
      },
    ])
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        queryInput: 'is:starred',
        searchParams: new URLSearchParams('surface=search&q=is%3Astarred'),
      }),
    )
    useExplorerDataMock.mockImplementation(
      (options: Parameters<typeof defaultExplorerData>[0]) =>
        defaultExplorerData(options, { handleVisit }),
    )
    renderExplorer()
    await waitFor(() =>
      expect(screen.getByTestId('paper-search-panel')).toHaveAttribute(
        'data-search-entry-count',
        '1',
      ),
    )
    // The synthetic starred row's id is -1; selecting it opens the URL via the
    // visit flow rather than the (impossible) detail-panel id lookup.
    await user.click(screen.getByTestId('paper-search-select-starred'))
    expect(handleVisit).toHaveBeenCalledWith('https://starred.test/one')
  })

  test('the Browse toolbar exposes a visible Starred entry point that opens the hub', async () => {
    const user = userEvent.setup()
    const setSearchParams = vi.fn()
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({ setSearchParams }),
    )
    renderExplorer()
    // The entry point lives in the Browse filter strip and is reachable by
    // click — the hub is no longer URL-typing-only.
    await user.click(screen.getByTestId('explorer-open-starred'))
    const next = setSearchParams.mock.calls.at(-1)?.[0] as URLSearchParams
    expect(next.get('surface')).toBe('starred')
  })

  test('the Starred hub back button returns to Browse (clears surface)', async () => {
    const user = userEvent.setup()
    const setSearchParams = vi.fn()
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        searchParams: new URLSearchParams('surface=starred'),
        setSearchParams,
      }),
    )
    renderExplorer()
    await user.click(screen.getByTestId('explorer-starred-back'))
    const next = setSearchParams.mock.calls.at(-1)?.[0] as URLSearchParams
    expect(next.get('surface')).toBeNull()
  })

  test('the Starred hub surfaces a load error callout instead of a blank hub', async () => {
    ;(backend.listStars as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error('hub read failed'),
    )
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        searchParams: new URLSearchParams('surface=starred'),
      }),
    )
    renderExplorer()
    expect(
      await screen.findByText('explorer.star.saveError'),
    ).toBeInTheDocument()
  })

  test('a failed star write surfaces a role=alert callout instead of reverting silently', async () => {
    const user = userEvent.setup()
    ;(backend.setStar as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error('archive locked'),
    )
    renderExplorer()
    await user.click(screen.getByTestId('paper-view-toggle-star'))
    const alert = await screen.findByTestId('explorer-star-error')
    expect(alert).toHaveAttribute('role', 'alert')
  })

  test('the Starred hub surface lists, opens, and un-stars items', async () => {
    const user = userEvent.setup()
    const setSearchParams = vi.fn()
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        searchParams: new URLSearchParams('surface=starred'),
        setSearchParams,
      }),
    )
    renderExplorer()
    expect(await screen.findByTestId('paper-starred-view')).toBeVisible()

    // Selecting a page opens it via the visit flow (no throw).
    await user.click(screen.getByTestId('paper-starred-select-page'))

    // Selecting a source rewrites the URL to a domain filter.
    await user.click(screen.getByTestId('paper-starred-select-source'))
    const domainParams = setSearchParams.mock.calls.at(
      -1,
    )?.[0] as URLSearchParams
    expect(domainParams.get('domain')).toBe('example.com')

    // Toggling writes through (the optimistic cache starts empty, so the
    // first toggle stars) and reloads the hub.
    await user.click(screen.getByTestId('paper-starred-toggle'))
    await waitFor(() =>
      expect(backend.setStar).toHaveBeenCalledWith({
        entityKind: 'url',
        entityKey: 'https://example.com/starred',
      }),
    )
  })

  test('paper filter strip trims filled values and deletes empty values from URL state', async () => {
    const user = userEvent.setup()
    const setSearchParams = vi.fn()
    const updateParam = vi.fn()
    const clearAllFilters = vi.fn()
    useExplorerUrlStateMock.mockReturnValue(
      defaultUrlState({
        activeFilters: [{ id: 'domain', label: 'example.com' }],
        clearAllFilters,
        searchParams: new URLSearchParams(
          'domain=old.test&browserKind=chrome&profileId=chrome%3AOld&start=2026-03-01&end=2026-03-31&regex=1&page=4',
        ),
        setSearchParams,
        updateParam,
      }),
    )

    renderExplorer()

    await user.click(screen.getByTestId('paper-filter-apply-filled'))
    const filled = setSearchParams.mock.calls.at(-1)?.[0] as URLSearchParams
    expect(filled.get('domain')).toBe('example.com')
    expect(filled.get('browserKind')).toBe('chromium')
    expect(filled.get('profileId')).toBe('chrome:Default')
    expect(filled.get('start')).toBe('2026-04-01')
    expect(filled.get('end')).toBe('2026-04-30')
    expect(filled.get('regex')).toBe('1')
    expect(filled.get('page')).toBeNull()

    await user.click(screen.getByTestId('paper-filter-apply-empty'))
    const emptied = setSearchParams.mock.calls.at(-1)?.[0] as URLSearchParams
    expect(emptied.get('domain')).toBeNull()
    expect(emptied.get('browserKind')).toBeNull()
    expect(emptied.get('profileId')).toBeNull()
    expect(emptied.get('start')).toBeNull()
    expect(emptied.get('end')).toBeNull()
    expect(emptied.get('regex')).toBeNull()
    expect(emptied.get('page')).toBeNull()

    await user.click(screen.getByTestId('paper-filter-remove-domain'))
    expect(updateParam).toHaveBeenCalledWith('domain', null)

    await user.click(screen.getByTestId('paper-filter-clear'))
    expect(clearAllFilters).toHaveBeenCalled()
  })

  test('legacy og:image fetch kill switch is folded into off mode for the hook', () => {
    useShellDataMock.mockReturnValue(
      defaultShellData({
        snapshot: {
          ...defaultShellData().snapshot,
          config: {
            ...defaultShellData().snapshot.config,
            ogImage: {
              fetchEnabled: false,
              fetchMode: 'on_demand',
            },
          },
        },
      }),
    )

    renderExplorer()

    expect(useExplorerOgImagesMock).toHaveBeenCalledWith(
      expect.objectContaining({ fetchMode: 'off' }),
    )
  })

  test('enabled og:image settings forward the configured fetch mode to the hook', () => {
    useShellDataMock.mockReturnValue(
      defaultShellData({
        snapshot: {
          ...defaultShellData().snapshot,
          config: {
            ...defaultShellData().snapshot.config,
            ogImage: {
              fetchEnabled: true,
              fetchMode: 'on_demand',
            },
          },
        },
      }),
    )

    renderExplorer()

    expect(useExplorerOgImagesMock).toHaveBeenCalledWith(
      expect.objectContaining({ fetchMode: 'on_demand' }),
    )
  })

  test('search-result URLs suppress misleading og:image hydration in Browse rows', () => {
    useExplorerDataMock.mockImplementation(
      (options: Parameters<typeof defaultExplorerData>[0]) =>
        defaultExplorerData(options, {
          queryState: {
            error: null,
            requestKey: options.requestKey,
            results: {
              items: [
                {
                  favicon: null,
                  id: 42,
                  ogImage: 'https://cdn.example.com/wrong-entity.png',
                  profileId: 'chrome:Default',
                  title: 'Search results',
                  url: 'https://www.google.com/search?q=yoshinoya',
                  visitTime: '2026-04-25T12:00:00.000Z',
                },
              ],
              page: 1,
              pageCount: 1,
              total: 1,
            },
          },
        }),
    )
    useExplorerOgImagesMock.mockReturnValue({
      ogImageCache: new Map([
        [
          'https://www.google.com/search?q=yoshinoya',
          'https://cdn.example.com/cached-wrong-entity.png',
        ],
      ]),
    })

    renderExplorer()

    expect(screen.getByTestId('paper-view-first-og-image')).toHaveTextContent(
      'none',
    )
  })

  test('paper detail mount onClose hides the panel and onOpen forwards to handleVisit', async () => {
    const user = userEvent.setup()
    const handleVisit = vi.fn()
    useExplorerDataMock.mockImplementation(
      (options: Parameters<typeof defaultExplorerData>[0]) =>
        defaultExplorerData(options, {
          handleVisit,
          // The route now refuses to bind the detail panel to a row that
          // isn't in the rendered pool (the `?? items[0]` fallback was
          // dropped on purpose so a filter mid-edit can't silently rebind
          // notes to the wrong URL). Preseed `selectedId` so the click on
          // the mock paper-view-select button actually opens the mount
          // against a real entry rather than getting auto-closed.
          selectedId: 42,
        }),
    )

    renderExplorer()

    // Selecting an entry in the paper view opens the detail mount.
    await user.click(screen.getByTestId('paper-view-select'))
    expect(screen.getByTestId('paper-detail-mount')).toBeVisible()

    // onOpen forwards through handleVisit.
    await user.click(screen.getByTestId('paper-detail-open'))
    expect(handleVisit).toHaveBeenCalledWith('https://example.com/open')

    await user.click(screen.getByTestId('paper-detail-open-domain'))
    const nextParams = setSearchParamsMockFromLatestUrlState()
    expect(nextParams.toString()).toBe('domain=example.com')

    // onClose drops the mount.
    await user.click(screen.getByTestId('paper-detail-close'))
    expect(screen.queryByTestId('paper-detail-mount')).toBeNull()
  })

  test('desktop command transport selects the desktop annotation store for paper details', async () => {
    const user = userEvent.setup()
    desktopCommandTransportAvailable.value = true
    useExplorerDataMock.mockImplementation(
      (options: Parameters<typeof defaultExplorerData>[0]) =>
        defaultExplorerData(options, {
          selectedId: 42,
        }),
    )

    renderExplorer()

    await user.click(screen.getByTestId('paper-view-select'))
    expect(screen.getByTestId('paper-detail-mount')).toHaveAttribute(
      'data-annotation-source',
      'desktop',
    )
  })

  test('shows fixable optional-AI repair copy for missing, failed, and disabled providers', () => {
    optionalAiFeaturesAvailableState.value = true
    selectedAiProviderMock.mockReturnValue(null)

    const { rerender } = renderExplorer()

    expect(screen.getByText('explorer.optionalAiNoProviderTitle')).toBeVisible()
    // The repair callout deep-links straight to the AI section so the fragment
    // scroll lands the user on the providers card.
    expect(
      screen.getByRole('link', { name: 'explorer.optionalAiOpenSettings' }),
    ).toHaveAttribute('href', '/settings#settings-ai')

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

function renderExplorer(options: { initialPath?: string } = {}) {
  return render(<ExplorerWrapper initialPath={options.initialPath} />)
}

function ExplorerWrapper({ initialPath }: { initialPath?: string }) {
  return (
    <MemoryRouter initialEntries={[initialPath ?? '/']}>
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

function setSearchParamsMockFromLatestUrlState() {
  const state = useExplorerUrlStateMock.mock.results.at(-1)?.value as {
    setSearchParams: ReturnType<typeof vi.fn>
  }
  return state.setSearchParams.mock.calls.at(-1)?.[0] as URLSearchParams
}

function annotationStore(source: string) {
  return {
    notesFor: () => source,
    tagsFor: () => [],
    updateNotes: vi.fn(),
    updateTags: vi.fn(),
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
            // Matches the id the mock paper-view emits via
            // `onSelectEntry({ id: 42 })` — without this, the route's
            // strict "entry must be in the rendered pool" lookup (no more
            // `?? items[0]` fallback) leaves `selectedEntry` null and the
            // auto-close guard immediately drops the detail mount.
            id: 42,
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
