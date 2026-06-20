import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  ActivityMix,
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
  EngineRanking,
  QueryFamilyResult,
  SearchConcept,
  TopSite,
} from '../../../lib/core-intelligence'
import { I18nProvider } from '../../../lib/i18n'
import { scheduleIdlePrefetch } from './idle-prefetch'
import {
  ActivityMixSection,
  SearchActivitySection,
} from './search-and-activity-section'

const apiMocks = vi.hoisted(() => ({
  getActivityMix: vi.fn(),
  getQueryFamilies: vi.fn(),
  getSearchEngineRanking: vi.fn(),
  getSearchQueries: vi.fn(),
  getTopSearchConcepts: vi.fn(),
  getTopSites: vi.fn(),
  peekActivityMix: vi.fn(),
  peekQueryFamilies: vi.fn(),
  peekSearchEngineRanking: vi.fn(),
  peekTopSearchConcepts: vi.fn(),
  peekTopSites: vi.fn(),
}))

vi.mock('../../../lib/core-intelligence/api', () => apiMocks)

vi.mock('../../../components/intelligence/search-keywords-browser', () => ({
  SearchKeywordsBrowser: ({
    engineOptions,
  }: {
    engineOptions: EngineRanking[]
  }) => (
    <div data-testid="search-keywords-browser">
      keywords:{engineOptions.map((engine) => engine.searchEngine).join(',')}
    </div>
  ),
}))

vi.mock('../../../components/intelligence/query-family-card', () => ({
  QueryFamilyCard: ({
    family,
    href,
    moreLabel,
  }: {
    family: { anchorQuery: string; memberCount: number }
    href: string
    moreLabel: (hiddenCount: number) => string
  }) => (
    <a data-testid="query-family-card" href={href}>
      {family.anchorQuery} {moreLabel(family.memberCount)}
    </a>
  ),
}))

const dateRange: DateRange = {
  start: '2026-04-01',
  end: '2026-04-30',
}

describe('SearchActivitySection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    const pendingRead = new Promise<never>(() => {})
    apiMocks.getActivityMix.mockReturnValue(pendingRead)
    apiMocks.getQueryFamilies.mockReturnValue(pendingRead)
    apiMocks.getSearchEngineRanking.mockReturnValue(pendingRead)
    apiMocks.getSearchQueries.mockResolvedValue({ rows: [], total: 0 })
    apiMocks.getTopSearchConcepts.mockReturnValue(pendingRead)
    apiMocks.getTopSites.mockReturnValue(pendingRead)
    apiMocks.peekActivityMix.mockReturnValue(activityMixSection())
    apiMocks.peekQueryFamilies.mockReturnValue(queryFamiliesSection())
    apiMocks.peekSearchEngineRanking.mockReturnValue(engineSection())
    apiMocks.peekTopSearchConcepts.mockReturnValue(conceptsSection())
    apiMocks.peekTopSites.mockReturnValue(topSitesSection())
    delete (window as Partial<Window>).requestIdleCallback
    delete (window as Partial<Window>).cancelIdleCallback
  })

  test('renders cached engine and concept tabs while prefetching deferred search data during idle time', () => {
    const requestIdleCallback = vi.fn(
      (callback: IdleRequestCallback, options?: IdleRequestOptions) => {
        expect(options?.timeout).toBe(1200)
        callback({ didTimeout: false, timeRemaining: () => 10 })
        return 42
      },
    )
    const cancelIdleCallback = vi.fn()
    Object.defineProperty(window, 'requestIdleCallback', {
      configurable: true,
      value: requestIdleCallback,
    })
    Object.defineProperty(window, 'cancelIdleCallback', {
      configurable: true,
      value: cancelIdleCallback,
    })

    const { unmount } = renderSearchActivity()

    expect(screen.getByText('Google')).toBeInTheDocument()
    expect(screen.getByText('1.5K')).toBeInTheDocument()
    expect(requestIdleCallback).toHaveBeenCalledWith(expect.any(Function), {
      timeout: 1200,
    })
    expect(apiMocks.getSearchQueries).toHaveBeenCalledWith(dateRange, {
      pagination: { page: 0, pageSize: 20 },
      profileId: 'chrome:Default',
    })

    fireEvent.click(screen.getByRole('tab', { name: 'searchTab_concepts' }))

    expect(screen.getByText('conceptChartSummary')).toBeInTheDocument()
    expect(screen.getByText('tauri')).toBeInTheDocument()
    expect(screen.getByText('google, kagi')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('tab', { name: 'searchTab_queries' }))

    expect(screen.getByTestId('search-keywords-browser')).toHaveTextContent(
      'google,kagi',
    )

    fireEvent.click(screen.getByRole('tab', { name: 'searchTab_families' }))

    expect(screen.getByTestId('query-family-card')).toHaveAttribute(
      'href',
      '/query-family/family-1?profile=',
    )
    expect(screen.getByTestId('query-family-card')).toHaveTextContent(
      'pathkeep +3 queryFamilyMore',
    )

    unmount()

    expect(cancelIdleCallback).toHaveBeenCalledWith(42)
  })

  test('renders empty and error states from cached section results', () => {
    apiMocks.peekSearchEngineRanking.mockReturnValue(
      section('search-activity', [] as EngineRanking[]),
    )
    apiMocks.peekTopSearchConcepts.mockReturnValue(
      section('search-activity', [] as SearchConcept[]),
    )
    apiMocks.peekQueryFamilies.mockReturnValue(
      section('search-activity', {
        families: [],
        page: 0,
        pageSize: 10,
        total: 0,
      }),
    )

    renderSearchActivity()

    expect(screen.getByText('engineRankingEmpty')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('tab', { name: 'searchTab_concepts' }))
    expect(screen.getByText('conceptCloudEmpty')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('tab', { name: 'searchTab_families' }))
    expect(screen.getByText('queryFamiliesPlaceholder')).toBeInTheDocument()
  })

  test('renders tab loading skeletons when no cached search payloads exist', () => {
    apiMocks.peekSearchEngineRanking.mockReturnValue(null)
    apiMocks.peekTopSearchConcepts.mockReturnValue(null)
    apiMocks.peekQueryFamilies.mockReturnValue(null)

    renderSearchActivity()

    expect(
      document.querySelector('.intelligence-skeleton--bar'),
    ).toBeInTheDocument()

    fireEvent.click(screen.getByRole('tab', { name: 'searchTab_concepts' }))
    expect(
      document.querySelector('.intelligence-skeleton--cloud'),
    ).toBeInTheDocument()

    fireEvent.click(screen.getByRole('tab', { name: 'searchTab_families' }))
    expect(
      document.querySelector('.intelligence-skeleton--list'),
    ).toBeInTheDocument()

    fireEvent.click(screen.getByRole('tab', { name: 'searchTab_queries' }))
    expect(screen.getByTestId('search-keywords-browser')).toHaveTextContent(
      'keywords:',
    )
  })

  test('turns idle prefetch into a no-op cleanup without a browser window', () => {
    const callback = vi.fn()
    const cleanup = scheduleIdlePrefetch(callback, null)

    cleanup()

    expect(callback).not.toHaveBeenCalled()
  })
})

describe('ActivityMixSection', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    const pendingRead = new Promise<never>(() => {})
    apiMocks.getActivityMix.mockReturnValue(pendingRead)
    apiMocks.getTopSites.mockReturnValue(pendingRead)
    apiMocks.peekActivityMix.mockReturnValue(activityMixSection())
    apiMocks.peekTopSites.mockReturnValue(topSitesSection())
  })

  test('renders stacked bar with segments, legend, deduped examples, and change indicators', () => {
    renderActivityMix()

    const stackedBar = screen.getByTestId('activity-mix-stacked-bar')
    expect(stackedBar).toBeInTheDocument()

    // The `quiet` fixture category has a 0% share and is filtered out, leaving
    // the two non-zero categories as rendered segments.
    const segments = stackedBar.querySelectorAll('.activity-mix__segment')
    expect(segments.length).toBe(2)
    expect(segments[0]).toHaveAttribute('data-category', 'work')
    expect(segments[1]).toHaveAttribute('data-category', 'community')

    expect(screen.getByText('category_work')).toBeInTheDocument()
    expect(screen.getByText('Community')).toBeInTheDocument()
    expect(screen.getByText('60%')).toBeInTheDocument()
    expect(screen.getByText('+12%')).toBeInTheDocument()
    expect(screen.getByText('-8%')).toBeInTheDocument()
    expect(screen.getByText('Docs')).toHaveAttribute('href', '/domain/docs.dev')
    expect(screen.getAllByText('Docs')).toHaveLength(1)
    expect(screen.getByText('Community Hub')).toHaveAttribute(
      'href',
      '/domain/community.dev',
    )
  })

  test('shows hover detail when a segment is hovered', () => {
    renderActivityMix()

    const stackedBar = screen.getByTestId('activity-mix-stacked-bar')
    const firstSegment = stackedBar.querySelector('.activity-mix__segment')!
    fireEvent.mouseEnter(firstSegment)

    const hoverDetail = screen.getByTestId('activity-mix-hover-detail')
    expect(hoverDetail).toBeInTheDocument()

    fireEvent.mouseLeave(firstSegment)
    expect(screen.queryByTestId('activity-mix-hover-detail')).toBeNull()
  })

  test('reveals and clears hover detail via keyboard focus and blur', () => {
    renderActivityMix()

    const stackedBar = screen.getByTestId('activity-mix-stacked-bar')
    const firstSegment = stackedBar.querySelector('.activity-mix__segment')!

    fireEvent.focus(firstSegment)

    const hoverDetail = screen.getByTestId('activity-mix-hover-detail')
    expect(hoverDetail).toBeInTheDocument()
    // The focused `work` segment (60% share / 60 visits) drives the detail copy.
    expect(
      hoverDetail.querySelector('.activity-mix__hover-category'),
    ).toHaveTextContent('category_work')
    expect(
      hoverDetail.querySelector('.activity-mix__hover-stats'),
    ).toHaveTextContent('60% · 60 visits')

    fireEvent.blur(firstSegment)
    expect(screen.queryByTestId('activity-mix-hover-detail')).toBeNull()
  })

  test('renders the empty activity mix state when no categories are available', () => {
    apiMocks.peekActivityMix.mockReturnValue(
      section('activity-mix', {
        categories: [],
        changeVsPrevious: [],
      }),
    )

    renderActivityMix()

    expect(screen.getByText('activityMixEmpty')).toBeInTheDocument()
  })
})

function renderSearchActivity() {
  return render(
    <MemoryRouter>
      <I18nProvider>
        <SearchActivitySection
          dateRange={dateRange}
          language="en"
          profileId="chrome:Default"
          queryFamilyHref={(familyId, profileId) =>
            `/query-family/${familyId}?profile=${encodeURIComponent(profileId ?? '')}`
          }
          scopeLabel="Chrome"
          t={testT}
          trailHref={(trailId) => `/trail/${trailId}`}
        />
      </I18nProvider>
    </MemoryRouter>,
  )
}

function renderActivityMix() {
  return render(
    <MemoryRouter>
      <I18nProvider>
        <ActivityMixSection
          dateRange={dateRange}
          domainHref={(domain) => `/domain/${domain}`}
          language="en"
          profileId="chrome:Default"
          scopeLabel="Chrome"
          t={testT}
        />
      </I18nProvider>
    </MemoryRouter>,
  )
}

function testT(key: string, vars?: Record<string, string | number>) {
  if (key === 'activityMixExamples' && vars?.domains) {
    return `examples:${vars.domains}`
  }
  if (key === 'conceptTooltip' && vars) {
    return `${vars.term}:${vars.count}`
  }
  return key
}

function engineSection() {
  return section<EngineRanking[]>('search-activity', [
    {
      displayName: 'Google',
      searchCount: 1500,
      searchEngine: 'google',
    },
    {
      displayName: null,
      searchCount: 300,
      searchEngine: 'kagi',
    },
  ])
}

function conceptsSection() {
  return section<SearchConcept[]>('search-activity', [
    {
      engines: ['google', 'kagi'],
      frequency: 8,
      term: 'tauri',
    },
    {
      engines: [],
      frequency: 4,
      term: 'sqlite',
    },
  ])
}

function queryFamiliesSection() {
  return section<QueryFamilyResult>('search-activity', {
    families: [
      {
        anchorQuery: 'pathkeep',
        familyId: 'family-1',
        firstSeenAt: '2026-04-01T00:00:00Z',
        lastSeenAt: '2026-04-25T00:00:00Z',
        memberCount: 3,
        queries: ['pathkeep', 'pathkeep tauri'],
        searchEngine: 'google',
      },
    ],
    page: 0,
    pageSize: 10,
    total: 1,
  })
}

function activityMixSection() {
  return section<ActivityMix>('activity-mix', {
    categories: [
      {
        domainCategory: 'work',
        share: 0.6,
        visitCount: 60,
      },
      {
        domainCategory: 'community',
        share: 0.4,
        visitCount: 40,
      },
      {
        domainCategory: 'quiet',
        share: 0,
        visitCount: 0,
      },
    ],
    changeVsPrevious: [
      {
        changePoints: 0.12,
        currentShare: 0.6,
        domainCategory: 'work',
        previousShare: 0.48,
      },
      {
        changePoints: -0.08,
        currentShare: 0.4,
        domainCategory: 'community',
        previousShare: 0.48,
      },
    ],
  })
}

function topSitesSection() {
  return section<TopSite[]>('top-sites', [
    topSite('docs.dev', 'Docs', 'work'),
    topSite('docs.dev', 'Docs duplicate', 'work'),
    topSite('api.dev', null, 'work'),
    topSite('community.dev', 'Community Hub', 'community'),
  ])
}

function topSite(
  registrableDomain: string,
  displayName: string | null,
  domainCategory: string,
): TopSite {
  return {
    averageDailyVisits: 2,
    displayName,
    domainCategory,
    registrableDomain,
    uniqueDays: 3,
    uniqueUrls: 5,
    visitCount: 10,
  }
}

function section<T>(
  sectionId: string,
  data: T,
): CoreIntelligenceSectionResult<T> {
  return {
    data,
    meta: meta(sectionId),
  }
}

function meta(sectionId: string): CoreIntelligenceSectionMeta {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    includesEnrichment: false,
    moduleIds: ['test'],
    notes: [],
    sectionId,
    sourceTables: ['test_table'],
    state: 'ready',
    stateReason: null,
    window: {
      dateRange,
      kind: 'date-range',
    },
  }
}
