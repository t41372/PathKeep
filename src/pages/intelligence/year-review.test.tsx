/**
 * @file year-review.test.tsx
 * @description Tests for the Year in Review narrative summary page.
 * @module pages/intelligence
 */

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { describe, expect, test, vi, beforeEach } from 'vitest'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
  DigestSummary,
  TopSite,
  QueryFamilyResult,
  DiscoveryTrend,
  ActivityMix,
  HabitPattern,
  RefindPage,
} from '../../lib/core-intelligence'
import { ProfileScopeContext } from '../../lib/profile-scope-context'
import { I18nProvider } from '../../lib/i18n'
import { YearReviewPage } from './year-review'

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const {
  getDigestSummaryMock,
  getTopSitesMock,
  getQueryFamiliesMock,
  getDiscoveryTrendMock,
  getActivityMixMock,
  getHabitPatternsMock,
  getRefindPagesMock,
} = vi.hoisted(() => ({
  getDigestSummaryMock: vi.fn(),
  getTopSitesMock: vi.fn(),
  getQueryFamiliesMock: vi.fn(),
  getDiscoveryTrendMock: vi.fn(),
  getActivityMixMock: vi.fn(),
  getHabitPatternsMock: vi.fn(),
  getRefindPagesMock: vi.fn(),
}))

vi.mock('../../lib/core-intelligence/api', () => ({
  getDigestSummary: getDigestSummaryMock,
  getTopSites: getTopSitesMock,
  getQueryFamilies: getQueryFamiliesMock,
  getDiscoveryTrend: getDiscoveryTrendMock,
  getActivityMix: getActivityMixMock,
  getHabitPatterns: getHabitPatternsMock,
  getRefindPages: getRefindPagesMock,
}))

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function meta(
  dateRange: DateRange,
  sectionId = 'digest-summary',
): CoreIntelligenceSectionMeta {
  return {
    sectionId,
    generatedAt: '2026-01-01T00:00:00Z',
    window: { kind: 'date-range', dateRange },
    moduleIds: ['test'],
    sourceTables: ['test'],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: [],
  }
}

function section<T>(
  data: T,
  dateRange: DateRange,
  sectionId?: string,
): CoreIntelligenceSectionResult<T> {
  return { data, meta: meta(dateRange, sectionId) }
}

const yearRange: DateRange = { start: '2025-01-01', end: '2025-12-31' }

function stubDigest(totalVisits = 5000): DigestSummary {
  return {
    dateRange: yearRange,
    totalVisits: { value: totalVisits, trend: 'up' },
    totalSearches: { value: 200, trend: 'up' },
    newDomains: { value: 150, trend: 'up' },
    deepReadPages: { value: 80, trend: 'flat' },
    refindPages: { value: 30, trend: 'up' },
  }
}

function stubTopSites(): TopSite[] {
  return [
    {
      registrableDomain: 'github.com',
      displayName: 'GitHub',
      domainCategory: 'development',
      visitCount: 1200,
      uniqueDays: 200,
      averageDailyVisits: 6,
      uniqueUrls: 300,
    },
    {
      registrableDomain: 'stackoverflow.com',
      displayName: null,
      domainCategory: 'development',
      visitCount: 800,
      uniqueDays: 150,
      averageDailyVisits: 5.3,
      uniqueUrls: 200,
    },
  ]
}

function stubQueryFamilies(): QueryFamilyResult {
  return {
    families: [
      {
        familyId: 'f1',
        anchorQuery: 'rust async',
        memberCount: 3,
        searchEngine: 'google',
        queries: ['rust async', 'rust tokio', 'rust async/await'],
        firstSeenAt: '2025-03-01',
        lastSeenAt: '2025-06-15',
      },
    ],
    total: 15,
    page: 0,
    pageSize: 3,
  }
}

function stubDiscoveryTrend(): DiscoveryTrend {
  return {
    points: [
      {
        dateKey: '2025-01-15',
        discoveryRate: 0.3,
        newDomainCount: 5,
        totalVisits: 50,
      },
      {
        dateKey: '2025-06-20',
        discoveryRate: 0.2,
        newDomainCount: 3,
        totalVisits: 80,
      },
    ],
    availableYears: [2024, 2025],
  }
}

function stubActivityMix(): ActivityMix {
  return {
    categories: [
      { domainCategory: 'development', visitCount: 3000, share: 0.6 },
      { domainCategory: 'reference', visitCount: 1000, share: 0.2 },
      { domainCategory: 'social', visitCount: 500, share: 0.1 },
      { domainCategory: 'news', visitCount: 500, share: 0.1 },
    ],
    changeVsPrevious: [],
  }
}

function stubHabits(): HabitPattern[] {
  return [
    {
      registrableDomain: 'github.com',
      displayName: 'GitHub',
      habitType: 'daily_habit',
      meanIntervalDays: 1,
      cv: 0.3,
      visitCount: 300,
      lastVisitedAt: '2025-12-30',
      isInterrupted: false,
    },
  ]
}

function stubRefindPages(): RefindPage[] {
  return [
    {
      canonicalUrl: 'https://doc.rust-lang.org/book/',
      url: 'https://doc.rust-lang.org/book/',
      title: 'The Rust Programming Language',
      registrableDomain: 'rust-lang.org',
      crossDayCount: 20,
      trailCount: 5,
      searchArrivalCount: 3,
      typedRevisitCount: 12,
      refindScore: 85,
      firstSeenAt: '2025-01-10',
      lastSeenAt: '2025-12-28',
    },
  ]
}

function setupAllMocksResolving() {
  getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
  getTopSitesMock.mockResolvedValue(
    section(stubTopSites(), yearRange, 'top-sites'),
  )
  getQueryFamiliesMock.mockResolvedValue(
    section(stubQueryFamilies(), yearRange, 'search-activity'),
  )
  getDiscoveryTrendMock.mockResolvedValue(
    section(stubDiscoveryTrend(), yearRange, 'discovery-trend'),
  )
  getActivityMixMock.mockResolvedValue(
    section(stubActivityMix(), yearRange, 'activity-mix'),
  )
  getHabitPatternsMock.mockResolvedValue(
    section(stubHabits(), yearRange, 'habits'),
  )
  getRefindPagesMock.mockResolvedValue(
    section(stubRefindPages(), yearRange, 'refind-pages'),
  )
}

function setupEmptyMocks() {
  getDigestSummaryMock.mockResolvedValue(
    section(
      { ...stubDigest(), totalVisits: { value: 0, trend: 'flat' } },
      yearRange,
    ),
  )
  getTopSitesMock.mockResolvedValue(section([], yearRange, 'top-sites'))
  getQueryFamiliesMock.mockResolvedValue(
    section(
      { families: [], total: 0, page: 0, pageSize: 3 },
      yearRange,
      'search-activity',
    ),
  )
  getDiscoveryTrendMock.mockResolvedValue(
    section({ points: [], availableYears: [] }, yearRange, 'discovery-trend'),
  )
  getActivityMixMock.mockResolvedValue(
    section(
      { categories: [], changeVsPrevious: [] },
      yearRange,
      'activity-mix',
    ),
  )
  getHabitPatternsMock.mockResolvedValue(section([], yearRange, 'habits'))
  getRefindPagesMock.mockResolvedValue(section([], yearRange, 'refind-pages'))
}

/**
 * Stub IntersectionObserver as a proper class so that `new IntersectionObserver()`
 * works inside FadeInSection. Each observed element is immediately reported as
 * intersecting so sections become visible without scroll simulation.
 */
class StubIntersectionObserver {
  private callback: IntersectionObserverCallback
  readonly root = null
  readonly rootMargin = ''
  readonly thresholds: number[] = []
  constructor(callback: IntersectionObserverCallback) {
    this.callback = callback
  }
  observe(target: Element) {
    this.callback(
      [{ isIntersecting: true, target } as IntersectionObserverEntry],
      this as unknown as IntersectionObserver,
    )
  }
  unobserve() {}
  disconnect() {}
  takeRecords(): IntersectionObserverEntry[] {
    return []
  }
}

function renderWithRoute(year = '2025') {
  const originalIO = globalThis.IntersectionObserver
  globalThis.IntersectionObserver =
    StubIntersectionObserver as unknown as typeof IntersectionObserver

  const router = createMemoryRouter(
    [
      {
        path: '/intelligence/year/:year',
        element: <YearReviewPage />,
      },
    ],
    { initialEntries: [`/intelligence/year/${year}`] },
  )

  const result = render(
    <I18nProvider>
      <ProfileScopeContext.Provider
        value={{
          activeProfileId: null,
          setActiveProfileId: vi.fn(),
        }}
      >
        <RouterProvider router={router} />
      </ProfileScopeContext.Provider>
    </I18nProvider>,
  )

  return {
    ...result,
    cleanup: () => {
      globalThis.IntersectionObserver = originalIO
    },
  }
}

/**
 * Like {@link renderWithRoute} but registers catch-all destination routes and
 * returns the `router` so navigation-triggering interactions (pager buttons,
 * heatmap cell clicks, footer CTA) can be asserted via
 * `router.state.location.pathname`. The destination routes render simple
 * markers so React Router does not blow up on a "no route matches" path.
 *
 * Accepts an optional `intersecting` flag so the IntersectionObserver stub can
 * report a non-intersecting entry (covers the FadeInSection "not yet visible"
 * branch).
 */
function renderWithRouter(year = '2025', intersecting = true) {
  const originalIO = globalThis.IntersectionObserver
  globalThis.IntersectionObserver = (intersecting
    ? StubIntersectionObserver
    : NonIntersectingObserver) as unknown as typeof IntersectionObserver

  const router = createMemoryRouter(
    [
      {
        path: '/intelligence/year/:year',
        element: <YearReviewPage />,
      },
      {
        path: '/intelligence/day/:date',
        element: <div data-testid="day-route">day route</div>,
      },
      {
        path: '/intelligence',
        element: <div data-testid="intelligence-route">intelligence route</div>,
      },
    ],
    { initialEntries: [`/intelligence/year/${year}`] },
  )

  const result = render(
    <I18nProvider>
      <ProfileScopeContext.Provider
        value={{
          activeProfileId: null,
          setActiveProfileId: vi.fn(),
        }}
      >
        <RouterProvider router={router} />
      </ProfileScopeContext.Provider>
    </I18nProvider>,
  )

  return {
    ...result,
    router,
    cleanup: () => {
      globalThis.IntersectionObserver = originalIO
    },
  }
}

/**
 * IntersectionObserver stub that reports its observed element as NOT
 * intersecting, so FadeInSection never flips to visible. Exercises the
 * `entry.isIntersecting === false` branch.
 */
class NonIntersectingObserver {
  private callback: IntersectionObserverCallback
  readonly root = null
  readonly rootMargin = ''
  readonly thresholds: number[] = []
  constructor(callback: IntersectionObserverCallback) {
    this.callback = callback
  }
  observe(target: Element) {
    this.callback(
      [{ isIntersecting: false, target } as IntersectionObserverEntry],
      this as unknown as IntersectionObserver,
    )
  }
  unobserve() {}
  disconnect() {}
  takeRecords(): IntersectionObserverEntry[] {
    return []
  }
}

/**
 * Builds a manually-resolvable promise so a test can unmount the component
 * before the in-flight fetch settles, exercising the `cancelled` guard paths.
 */
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.resetAllMocks()
})

describe('YearReviewPage', () => {
  test('shows loading state initially', () => {
    // Never resolve the mocks
    getDigestSummaryMock.mockReturnValue(new Promise(() => {}))
    getTopSitesMock.mockReturnValue(new Promise(() => {}))
    getQueryFamiliesMock.mockReturnValue(new Promise(() => {}))
    getDiscoveryTrendMock.mockReturnValue(new Promise(() => {}))
    getActivityMixMock.mockReturnValue(new Promise(() => {}))
    getHabitPatternsMock.mockReturnValue(new Promise(() => {}))
    getRefindPagesMock.mockReturnValue(new Promise(() => {}))

    const { cleanup } = renderWithRoute()
    expect(screen.getByTestId('year-review-loading')).toBeInTheDocument()
    cleanup()
  })

  test('shows empty state when digest has zero visits', async () => {
    setupEmptyMocks()
    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-empty')).toBeInTheDocument()
    })
    expect(screen.getByText('No data for 2025')).toBeInTheDocument()
    cleanup()
  })

  test('renders hero section with stats when data is present', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    expect(screen.getByText('Your 2025 in Pages')).toBeInTheDocument()
    expect(screen.getByTestId('yr-stat-visits')).toHaveTextContent('5.0K')
    cleanup()
  })

  test('renders top sites podium', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-podium')).toBeInTheDocument()
    })
    const podium = screen.getByTestId('year-review-podium')
    expect(podium).toHaveTextContent('GitHub')
    expect(podium).toHaveTextContent('stackoverflow.com')
    cleanup()
  })

  test('renders research section with query families', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-research')).toBeInTheDocument()
    })
    expect(screen.getByText('rust async')).toBeInTheDocument()
    cleanup()
  })

  test('renders habits section', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-habits')).toBeInTheDocument()
    })
    cleanup()
  })

  test('renders refind section with page titles', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-refind')).toBeInTheDocument()
    })
    expect(
      screen.getByText('The Rust Programming Language'),
    ).toBeInTheDocument()
    cleanup()
  })

  test('renders content mix section', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-mix')).toBeInTheDocument()
    })
    cleanup()
  })

  test('renders year pager with navigation buttons', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-pager')).toBeInTheDocument()
    })
    cleanup()
  })

  test('shows "(so far)" in title for current year', async () => {
    setupAllMocksResolving()
    const currentYear = new Date().getFullYear()
    const { cleanup } = renderWithRoute(String(currentYear))

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    expect(
      screen.getByText(`Your ${currentYear} in Pages (so far)`),
    ).toBeInTheDocument()
    cleanup()
  })

  test('shows error state when API fails', async () => {
    getDigestSummaryMock.mockRejectedValue(new Error('Network error'))
    getTopSitesMock.mockRejectedValue(new Error('Network error'))
    getQueryFamiliesMock.mockRejectedValue(new Error('Network error'))
    getDiscoveryTrendMock.mockRejectedValue(new Error('Network error'))
    getActivityMixMock.mockRejectedValue(new Error('Network error'))
    getHabitPatternsMock.mockRejectedValue(new Error('Network error'))
    getRefindPagesMock.mockRejectedValue(new Error('Network error'))

    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-error')).toBeInTheDocument()
    })
    cleanup()
  })

  test('renders footer CTA linking to full intelligence view', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-footer-cta')).toBeInTheDocument()
    })
    cleanup()
  })

  test('falls back to the current year when the route param is not numeric', async () => {
    setupAllMocksResolving()
    const currentYear = new Date().getFullYear()
    // "abc" => Number(NaN) => falsy => `|| currentYear` branch.
    const { cleanup } = renderWithRoute('abc')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    // Current-year fallback also flips isCurrentYear => "(so far)" title.
    expect(
      screen.getByText(`Your ${currentYear} in Pages (so far)`),
    ).toBeInTheDocument()
    cleanup()
  })

  test('past year shows the plain title without "(so far)"', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute('2025')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    expect(screen.getByText('Your 2025 in Pages')).toBeInTheDocument()
    expect(
      screen.queryByText('Your 2025 in Pages (so far)'),
    ).not.toBeInTheDocument()
    cleanup()
  })

  test('compactNumber renders millions with an M suffix', async () => {
    getDigestSummaryMock.mockResolvedValue(
      section(stubDigest(2_500_000), yearRange),
    )
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section(stubDiscoveryTrend(), yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('yr-stat-visits')).toBeInTheDocument()
    })
    expect(screen.getByTestId('yr-stat-visits')).toHaveTextContent('2.5M')
    cleanup()
  })

  // -- Year pager navigation -------------------------------------------------

  test('year pager "previous" navigates to the prior year', async () => {
    setupAllMocksResolving()
    const { cleanup, router } = renderWithRouter('2025')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    // Two pagers render (empty-state has one; hero has one). The hero pager is
    // the one inside the rendered page; grab the first "Previous year" button.
    const prevButtons = screen.getAllByLabelText('Previous year')
    fireEvent.click(prevButtons[0])
    expect(router.state.location.pathname).toBe('/intelligence/year/2024')
    cleanup()
  })

  test('year pager "next" navigates forward for a past year', async () => {
    setupAllMocksResolving()
    const { cleanup, router } = renderWithRouter('2024')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    const nextButton = screen.getByLabelText('Next year')
    expect(nextButton).not.toBeDisabled()
    fireEvent.click(nextButton)
    expect(router.state.location.pathname).toBe('/intelligence/year/2025')
    cleanup()
  })

  test('year pager "next" is disabled on the current year', async () => {
    setupAllMocksResolving()
    const currentYear = new Date().getFullYear()
    const { cleanup } = renderWithRouter(String(currentYear))

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    expect(screen.getByLabelText('Next year')).toBeDisabled()
    cleanup()
  })

  test('empty-state pager still navigates between years', async () => {
    setupEmptyMocks()
    const { cleanup, router } = renderWithRouter('2025')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-empty')).toBeInTheDocument()
    })
    fireEvent.click(screen.getByLabelText('Previous year'))
    expect(router.state.location.pathname).toBe('/intelligence/year/2024')
    cleanup()
  })

  // -- Heatmap cell selection ------------------------------------------------

  test('clicking a heatmap cell navigates to that day insights route', async () => {
    setupAllMocksResolving()
    const { cleanup, router } = renderWithRouter('2025')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-heatmap')).toBeInTheDocument()
    })
    // 2025-01-15 has 50 visits in the discovery trend stub => clickable cell.
    const heatmap = screen.getByTestId('year-review-heatmap')
    const cell = heatmap.querySelector<HTMLButtonElement>(
      'button[data-date="2025-01-15"]',
    )
    expect(cell).not.toBeNull()
    expect(cell).not.toBeDisabled()
    fireEvent.click(cell as HTMLButtonElement)
    expect(router.state.location.pathname).toBe('/intelligence/day/2025-01-15')
    cleanup()
  })

  // -- Footer CTA navigation -------------------------------------------------

  test('footer CTA navigates to the custom-range intelligence view', async () => {
    setupAllMocksResolving()
    const { cleanup, router } = renderWithRouter('2025')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-footer-cta')).toBeInTheDocument()
    })
    fireEvent.click(screen.getByTestId('year-review-footer-cta'))
    expect(router.state.location.pathname).toBe('/intelligence')
    expect(router.state.location.search).toBe(
      '?range=custom&start=2025-01-01&end=2025-12-31',
    )
    cleanup()
  })

  // -- Section empty-state guards (each section returns null) -----------------

  test('omits sections whose data is empty', async () => {
    // Digest has visits (so we render the page) but every other section is empty.
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(section([], yearRange, 'top-sites'))
    getQueryFamiliesMock.mockResolvedValue(
      section(
        { families: [], total: 0, page: 0, pageSize: 3 },
        yearRange,
        'search-activity',
      ),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section({ points: [], availableYears: [] }, yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(
        { categories: [], changeVsPrevious: [] },
        yearRange,
        'activity-mix',
      ),
    )
    getHabitPatternsMock.mockResolvedValue(section([], yearRange, 'habits'))
    getRefindPagesMock.mockResolvedValue(section([], yearRange, 'refind-pages'))

    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    // Volume section always renders (heatmap); the rest guard on empty data.
    expect(screen.getByTestId('year-review-volume')).toBeInTheDocument()
    expect(screen.queryByTestId('year-review-podium')).not.toBeInTheDocument()
    expect(screen.queryByTestId('year-review-research')).not.toBeInTheDocument()
    expect(
      screen.queryByTestId('year-review-discovery'),
    ).not.toBeInTheDocument()
    expect(screen.queryByTestId('year-review-mix')).not.toBeInTheDocument()
    expect(screen.queryByTestId('year-review-habits')).not.toBeInTheDocument()
    expect(screen.queryByTestId('year-review-refind')).not.toBeInTheDocument()
    cleanup()
  })

  test('omits research section when queryFamilies payload is null', async () => {
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    // null queryFamilies => first half of `!queryFamilies || total === 0`.
    getQueryFamiliesMock.mockResolvedValue(
      section(null, yearRange, 'search-activity'),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section(stubDiscoveryTrend(), yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    expect(screen.queryByTestId('year-review-research')).not.toBeInTheDocument()
    cleanup()
  })

  test('omits content mix section when activityMix payload is null', async () => {
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section(stubDiscoveryTrend(), yearRange, 'discovery-trend'),
    )
    // null activityMix => first half of `!activityMix || categories.length === 0`.
    getActivityMixMock.mockResolvedValue(
      section(null, yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-hero')).toBeInTheDocument()
    })
    expect(screen.queryByTestId('year-review-mix')).not.toBeInTheDocument()
    cleanup()
  })

  test('volume section copes with a null discovery trend and renders zero busiest-day', async () => {
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    // null discoveryTrend => `discoveryTrend?.points ?? []` empty in both
    // VolumeSection and DiscoverySection.
    getDiscoveryTrendMock.mockResolvedValue(
      section(null, yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-volume')).toBeInTheDocument()
    })
    // No points => no busiest-day line, but the active-days line still renders
    // (0 of 365 in 2025, a non-leap year).
    expect(
      screen.getByText('You were active on 0 of 365 days.'),
    ).toBeInTheDocument()
    // Discovery section guards on empty points => omitted.
    expect(
      screen.queryByTestId('year-review-discovery'),
    ).not.toBeInTheDocument()
    cleanup()
  })

  test('volume section shows the busiest day when points exist', async () => {
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute('2025')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-volume')).toBeInTheDocument()
    })
    // 2025-06-20 has 80 visits (the max of the two stub points).
    expect(
      screen.getByText('Your busiest day was 2025-06-20 with 80 pages.'),
    ).toBeInTheDocument()
    cleanup()
  })

  // -- Optional-field fallbacks ----------------------------------------------

  test('habits section labels weekly and periodic types and falls back to domain', async () => {
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section(stubDiscoveryTrend(), yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    const habits: HabitPattern[] = [
      {
        registrableDomain: 'news.example',
        // null displayName => `habit.displayName ?? habit.registrableDomain`.
        displayName: null,
        habitType: 'weekly_habit',
        meanIntervalDays: 7,
        cv: 0.2,
        visitCount: 40,
        lastVisitedAt: '2025-12-20',
        isInterrupted: false,
      },
      {
        registrableDomain: 'irs.example',
        displayName: 'Tax Portal',
        habitType: 'periodic_reference',
        meanIntervalDays: 90,
        cv: 0.5,
        visitCount: 8,
        lastVisitedAt: '2025-11-01',
        isInterrupted: false,
      },
    ]
    getHabitPatternsMock.mockResolvedValue(section(habits, yearRange, 'habits'))
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-habits')).toBeInTheDocument()
    })
    const habitsSection = screen.getByTestId('year-review-habits')
    // Domain fallback for the null-displayName habit.
    expect(habitsSection).toHaveTextContent('news.example')
    expect(habitsSection).toHaveTextContent('Tax Portal')
    // Weekly + periodic labels.
    expect(habitsSection).toHaveTextContent('weekly')
    expect(habitsSection).toHaveTextContent('periodic')
    cleanup()
  })

  test('refind section falls back to the URL when a page has no title', async () => {
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section(stubDiscoveryTrend(), yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    const refind: RefindPage[] = [
      {
        canonicalUrl: 'https://untitled.example/page',
        url: 'https://untitled.example/page',
        // null title => `page.title ?? page.url`.
        title: null,
        registrableDomain: 'untitled.example',
        crossDayCount: 9,
        trailCount: 2,
        searchArrivalCount: 1,
        typedRevisitCount: 4,
        refindScore: 50,
        firstSeenAt: '2025-02-01',
        lastSeenAt: '2025-10-10',
      },
    ]
    getRefindPagesMock.mockResolvedValue(
      section(refind, yearRange, 'refind-pages'),
    )

    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-refind')).toBeInTheDocument()
    })
    expect(
      screen.getByText('https://untitled.example/page'),
    ).toBeInTheDocument()
    cleanup()
  })

  // -- Localized formatting (non-English) ------------------------------------

  test('formats the discovery month label in zh-CN for a non-English locale', async () => {
    window.localStorage.setItem('pathkeep-language-preference', 'zh-CN')
    setupAllMocksResolving()
    const { cleanup } = renderWithRoute('2025')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-discovery')).toBeInTheDocument()
    })
    // zh-CN discovery copy with the localized "exploratory month".
    // The stub's busiest discovery month is 2025-06 (8 new domains total in
    // the two stub points, both distinct months; January has 5, June has 3,
    // so January wins). Assert the zh-CN sentence shell is used.
    const discovery = screen.getByTestId('year-review-discovery')
    expect(discovery).toHaveTextContent('你发现了')
    expect(discovery).toHaveTextContent('最爱探索的月份是')
    // zh-CN "long month + year" formatting renders CJK era characters.
    expect(discovery.textContent ?? '').toMatch(/年/)
    cleanup()
  })

  test('discovery section omits the exploratory-month line when no month dominates', async () => {
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    // Points exist (section renders) but every newDomainCount is 0 => bestCount
    // stays 0 => bestMonth '' => bestMonthLabel '' => exploratory line omitted.
    getDiscoveryTrendMock.mockResolvedValue(
      section(
        {
          points: [
            {
              dateKey: '2025-01-15',
              discoveryRate: 0,
              newDomainCount: 0,
              totalVisits: 50,
            },
            {
              dateKey: '2025-06-20',
              discoveryRate: 0,
              newDomainCount: 0,
              totalVisits: 80,
            },
          ],
          availableYears: [2025],
        },
        yearRange,
        'discovery-trend',
      ),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    const { cleanup } = renderWithRoute()

    await waitFor(() => {
      expect(screen.getByTestId('year-review-discovery')).toBeInTheDocument()
    })
    const discovery = screen.getByTestId('year-review-discovery')
    expect(discovery).toHaveTextContent('You discovered 0 new websites.')
    expect(discovery.textContent ?? '').not.toMatch(/most exploratory month/)
    cleanup()
  })

  // -- FadeInSection observer branches ---------------------------------------

  test('sections stay hidden until the observer reports intersection', async () => {
    setupAllMocksResolving()
    // NonIntersectingObserver never flips visibility => no --visible modifier.
    const { cleanup } = renderWithRouter('2025', false)

    await waitFor(() => {
      expect(screen.getByTestId('year-review-podium')).toBeInTheDocument()
    })
    const podium = screen.getByTestId('year-review-podium')
    expect(podium.className).not.toMatch(/year-review__section--visible/)
    cleanup()
  })

  // -- Leap year heatmap -----------------------------------------------------

  test('uses 366 days for a leap year', async () => {
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    // No points so the active-days line reports "0 of <total>" cleanly.
    getDiscoveryTrendMock.mockResolvedValue(
      section({ points: [], availableYears: [] }, yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    // 2024 is a leap year (div by 4, not by 100).
    const { cleanup } = renderWithRoute('2024')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-volume')).toBeInTheDocument()
    })
    expect(
      screen.getByText('You were active on 0 of 366 days.'),
    ).toBeInTheDocument()
    cleanup()
  })

  test('treats a divisible-by-100-but-not-400 year as non-leap (365 days)', async () => {
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section({ points: [], availableYears: [] }, yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    // 1900: divisible by 4 and by 100 but NOT by 400 => not a leap year.
    const { cleanup } = renderWithRoute('1900')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-volume')).toBeInTheDocument()
    })
    expect(
      screen.getByText('You were active on 0 of 365 days.'),
    ).toBeInTheDocument()
    cleanup()
  })

  test('treats a divisible-by-400 year as a leap year (366 days)', async () => {
    getDigestSummaryMock.mockResolvedValue(section(stubDigest(), yearRange))
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section({ points: [], availableYears: [] }, yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    // 2000: divisible by 400 => leap year.
    const { cleanup } = renderWithRoute('2000')

    await waitFor(() => {
      expect(screen.getByTestId('year-review-volume')).toBeInTheDocument()
    })
    expect(
      screen.getByText('You were active on 0 of 366 days.'),
    ).toBeInTheDocument()
    cleanup()
  })

  // -- cancelled-guard paths (unmount before fetch settles) ------------------

  test('ignores a resolving fetch after the component unmounts', async () => {
    const digest = deferred<ReturnType<typeof section<DigestSummary>>>()
    getDigestSummaryMock.mockReturnValue(digest.promise)
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section(stubDiscoveryTrend(), yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    const { cleanup, unmount } = renderWithRoute()
    // Still loading (digest promise pending).
    expect(screen.getByTestId('year-review-loading')).toBeInTheDocument()
    // Unmount sets `cancelled = true` before the promise settles.
    unmount()
    // Now settle the fetch; the `if (cancelled) return` guard must swallow it
    // without a "set state on unmounted component" warning or throw.
    digest.resolve(section(stubDigest(), yearRange))
    await Promise.resolve()
    await Promise.resolve()
    cleanup()
  })

  test('ignores a rejecting fetch after the component unmounts', async () => {
    const digest = deferred<ReturnType<typeof section<DigestSummary>>>()
    getDigestSummaryMock.mockReturnValue(digest.promise)
    getTopSitesMock.mockResolvedValue(
      section(stubTopSites(), yearRange, 'top-sites'),
    )
    getQueryFamiliesMock.mockResolvedValue(
      section(stubQueryFamilies(), yearRange, 'search-activity'),
    )
    getDiscoveryTrendMock.mockResolvedValue(
      section(stubDiscoveryTrend(), yearRange, 'discovery-trend'),
    )
    getActivityMixMock.mockResolvedValue(
      section(stubActivityMix(), yearRange, 'activity-mix'),
    )
    getHabitPatternsMock.mockResolvedValue(
      section(stubHabits(), yearRange, 'habits'),
    )
    getRefindPagesMock.mockResolvedValue(
      section(stubRefindPages(), yearRange, 'refind-pages'),
    )

    const { cleanup, unmount } = renderWithRoute()
    expect(screen.getByTestId('year-review-loading')).toBeInTheDocument()
    unmount()
    // Reject after unmount: catch's `if (!cancelled)` and finally's
    // `if (!cancelled)` must both take the false branch (no error/loading set).
    digest.reject(new Error('late failure'))
    await Promise.resolve()
    await Promise.resolve()
    cleanup()
  })
})
