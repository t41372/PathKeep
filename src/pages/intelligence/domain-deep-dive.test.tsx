import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  CompareSetDetail,
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
  DomainDeepDive,
  InsightRouteFocus,
  PathFlow,
} from '../../lib/core-intelligence'
import { DomainDeepDivePage, DomainDeepDiveRoutePage } from './domain-deep-dive'

const {
  getCompareSetDetailMock,
  getDomainDeepDiveMock,
  getPathFlowsMock,
  routeStateMock,
  starsToggleMock,
  starsHydrateMock,
} = vi.hoisted(() => ({
  getCompareSetDetailMock: vi.fn(),
  getDomainDeepDiveMock: vi.fn(),
  getPathFlowsMock: vi.fn(),
  routeStateMock: vi.fn(),
  starsToggleMock: vi.fn(),
  starsHydrateMock: vi.fn(),
}))

vi.mock('../explorer/use-desktop-stars', () => ({
  useDesktopStars: () => ({
    isStarred: () => false,
    hydrate: starsHydrateMock,
    toggle: starsToggleMock,
    lastError: null,
  }),
}))

vi.mock('../../lib/i18n/hooks', () => ({
  useI18n: () => ({
    language: 'en',
    ns: () => (key: string, vars?: Record<string, string | number>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
    t: (key: string, vars?: Record<string, string | number>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key,
  }),
}))

vi.mock('../../components/intelligence/search-keywords-browser', () => ({
  SearchKeywordsBrowser: ({
    queryFamilyHref,
    title,
    trailHref,
  }: {
    queryFamilyHref: (familyId: string, profileId: string | null) => string
    title: string
    trailHref: (trailId: string, profileId: string | null) => string
  }) => (
    <div data-testid="search-keywords-browser">
      {title}
      <a href={queryFamilyHref('family-1', null)}>family-current</a>
      <a href={queryFamilyHref('family-2', 'chrome:Other')}>family-other</a>
      <a href={trailHref('trail-1', null)}>trail-current</a>
      <a href={trailHref('trail-2', 'chrome:Other')}>trail-other</a>
    </div>
  ),
}))

vi.mock('../../components/intelligence/time-range-selector', () => ({
  TimeRangeSelector: () => <div data-testid="time-range-selector" />,
}))

vi.mock('../../lib/core-intelligence/api', () => ({
  getCompareSetDetail: getCompareSetDetailMock,
  getDomainDeepDive: getDomainDeepDiveMock,
  getPathFlows: getPathFlowsMock,
}))

vi.mock('./route-state', () => ({
  useIntelligenceRouteState: routeStateMock,
}))

const dateRange: DateRange = {
  start: '2026-04-01',
  end: '2026-04-30',
}

describe('DomainDeepDive route and page', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    routeStateMock.mockReturnValue({
      dateRange,
      effectiveProfileId: null,
      focus: null,
      preset: 'custom',
      profileScopeLabel: null,
      setCustomRange: vi.fn(),
      setPreset: vi.fn(),
      withCurrentRouteSearch: () => '?range=custom',
    })
    getCompareSetDetailMock.mockResolvedValue(null)
    getDomainDeepDiveMock.mockResolvedValue(section(domainFixture()))
    getPathFlowsMock.mockResolvedValue(section([]))
  })

  test('shows the route empty state when no domain parameter is available', () => {
    render(
      <MemoryRouter initialEntries={['/intelligence/domain']}>
        <Routes>
          <Route
            path="/intelligence/domain"
            element={<DomainDeepDiveRoutePage />}
          />
        </Routes>
      </MemoryRouter>,
    )

    expect(screen.getByText('domainDeepDiveEmpty')).toBeVisible()
  })

  test('renders the scoped route strip with the active profile label', async () => {
    routeStateMock.mockReturnValue({
      dateRange,
      effectiveProfileId: 'chrome:Default',
      focus: null,
      preset: 'custom',
      profileScopeLabel: 'Default profile',
      setCustomRange: vi.fn(),
      setPreset: vi.fn(),
      withCurrentRouteSearch: () => '?range=custom&profile=chrome%3ADefault',
    })

    render(
      <MemoryRouter initialEntries={['/intelligence/domain/example.com']}>
        <Routes>
          <Route
            path="/intelligence/domain/:domain"
            element={<DomainDeepDiveRoutePage />}
          />
        </Routes>
      </MemoryRouter>,
    )

    expect(await screen.findByText('scopedViewTitle')).toBeVisible()
    expect(
      screen.getByText('scopedViewBody:{"profile":"Default profile"}'),
    ).toBeVisible()
  })

  test('uses the profile id when a scoped route has no display label', async () => {
    routeStateMock.mockReturnValue({
      dateRange,
      effectiveProfileId: 'chrome:Default',
      focus: null,
      preset: 'custom',
      profileScopeLabel: null,
      setCustomRange: vi.fn(),
      setPreset: vi.fn(),
      withCurrentRouteSearch: () => '?range=custom&profile=chrome%3ADefault',
    })

    render(
      <MemoryRouter initialEntries={['/intelligence/domain/example.com']}>
        <Routes>
          <Route
            path="/intelligence/domain/:domain"
            element={<DomainDeepDiveRoutePage />}
          />
        </Routes>
      </MemoryRouter>,
    )

    expect(await screen.findByText('scopedViewTitle')).toBeVisible()
    expect(
      screen.getByText('scopedViewBody:{"profile":"chrome:Default"}'),
    ).toBeVisible()
  })

  test('the domain hero exposes a star toggle wired to useDesktopStars', async () => {
    const user = userEvent.setup()
    render(
      <MemoryRouter initialEntries={['/intelligence/domain/example.com']}>
        <Routes>
          <Route
            path="/intelligence/domain/:domain"
            element={<DomainDeepDiveRoutePage />}
          />
        </Routes>
      </MemoryRouter>,
    )
    const star = await screen.findByTestId('domain-deep-dive-star-example.com')
    // The single domain is hydrated on mount, never the whole archive.
    await waitFor(() =>
      expect(starsHydrateMock).toHaveBeenCalledWith('domain', ['example.com']),
    )
    await user.click(star)
    expect(starsToggleMock).toHaveBeenCalledWith('domain', 'example.com')
  })

  test('shows loading and error states from the domain detail request', async () => {
    getDomainDeepDiveMock.mockRejectedValue(new Error('domain unavailable'))

    const { container } = renderPage()

    expect(container.querySelector('.intelligence-skeleton')).not.toBeNull()
    expect(await screen.findByText('domain unavailable')).toBeVisible()
  })

  test('shows the empty state when the domain detail payload is empty', async () => {
    getDomainDeepDiveMock.mockResolvedValue(section(null))

    renderPage()

    expect(await screen.findByText('domainDeepDiveEmpty')).toBeVisible()
  })

  test('highlights compare-set pages and keeps invalid compare URLs out of focused paths', async () => {
    getDomainDeepDiveMock.mockResolvedValue(
      section(
        domainFixture({
          topPages: [
            {
              path: '/docs',
              visitCount: 2_500,
            },
            {
              path: '/not-focused',
              visitCount: 12,
            },
          ],
        }),
      ),
    )
    getCompareSetDetailMock.mockResolvedValue(
      section(
        compareSetDetailFixture([
          {
            canonicalUrl: 'https://example.com/docs',
            registrableDomain: 'example.com',
          },
          {
            canonicalUrl: null,
            registrableDomain: 'example.com',
          },
          {
            canonicalUrl: 'not a valid url',
            registrableDomain: 'example.com',
          },
          {
            canonicalUrl: 'https://other.test/docs',
            registrableDomain: 'other.test',
          },
        ]),
      ),
    )

    renderPage({
      focus: {
        focusType: 'compare-set',
        focusId: 'compare-1',
      },
    })

    expect(await screen.findByText('Example')).toBeVisible()
    expect(screen.getByText('1.2M')).toBeVisible()
    expect(screen.getByText('2.5K')).toBeVisible()
    expect(screen.getByText('compareSetFocusTitle')).toBeVisible()
    expect(screen.getByText('compareSetFocusBadge')).toBeVisible()
    expect(getCompareSetDetailMock).toHaveBeenCalledWith(
      'compare-1',
      dateRange,
      null,
    )
  })

  test('shows path-flow focus when the focused flow includes the current domain', async () => {
    getPathFlowsMock.mockResolvedValue(
      section<PathFlow[]>([
        {
          flowId: 'path-flow:example:5:abc',
          flowPattern: 'Search -> Example -> Docs',
          stepCount: 5,
          occurrenceCount: 3,
          lastSeenAt: '2026-04-20T12:00:00Z',
          steps: [
            {
              index: 0,
              label: 'Search',
              registrableDomain: 'search.test',
            },
            {
              index: 1,
              label: 'Example',
              registrableDomain: 'example.com',
            },
          ],
        },
      ]),
    )

    renderPage({
      focus: {
        focusType: 'path-flow',
        focusId: 'path-flow:example:5:abc',
      },
      profileId: 'chrome:Default',
    })

    expect(await screen.findByText('pathFlowFocusTitle')).toBeVisible()
    expect(
      screen.getByText(
        'pathFlowFocusBody:{"flow":"Search -> Example -> Docs"}',
      ),
    ).toBeVisible()
    expect(getPathFlowsMock).toHaveBeenCalledWith(
      dateRange,
      'chrome:Default',
      5,
      50,
    )
  })

  test('falls back for optional domain fields and malformed path-flow focus ids', async () => {
    getDomainDeepDiveMock.mockResolvedValue(
      section(
        domainFixture({
          arrivalBreakdown: {
            search: 2,
            link: 1,
            typed: 1,
            other: 0,
          },
          displayName: null,
          totalVisits: 0,
          topExits: [
            {
              domain: 'fallback-exit.test',
              displayName: null,
              count: 1_500,
            },
          ],
          topPages: [
            {
              path: '/',
              visitCount: 999,
            },
            {
              path: '/k',
              visitCount: 1_000,
            },
          ],
        }),
      ),
    )
    getPathFlowsMock.mockResolvedValue(
      section<PathFlow[]>([
        {
          flowId: 'path-flow:example:not-a-number:abc',
          flowPattern: 'Example malformed flow',
          stepCount: 3,
          occurrenceCount: 2,
          lastSeenAt: '2026-04-20T12:00:00Z',
          steps: [
            {
              index: 0,
              label: 'Example',
              registrableDomain: 'example.com',
            },
          ],
        },
      ]),
    )

    renderPage({
      focus: {
        focusType: 'path-flow',
        focusId: 'path-flow:example:not-a-number:abc',
      },
      profileId: 'chrome:Default',
    })

    expect(await screen.findByText('example.com')).toBeVisible()
    expect(screen.getByText('domainDeepDiveArrival')).toBeVisible()
    expect(screen.getByText('/')).toBeVisible()
    expect(screen.getByText('1.0K')).toBeVisible()
    expect(screen.getByText('fallback-exit.test')).toBeVisible()
    expect(screen.getByText('1.5K')).toBeVisible()
    expect(screen.getByText('pathFlowFocusTitle')).toBeVisible()
    expect(getPathFlowsMock).toHaveBeenCalledWith(
      dateRange,
      'chrome:Default',
      3,
      50,
    )
  })
})

function renderPage({
  focus = null,
  profileId = null,
}: {
  focus?: InsightRouteFocus | null
  profileId?: string | null
} = {}) {
  return render(
    <MemoryRouter>
      <DomainDeepDivePage
        backHref="/intelligence"
        dateRange={dateRange}
        dayHref={(date) => `/intelligence/day/${date}`}
        domain="example.com"
        domainHref={(domain) => `/intelligence/domain/${domain}`}
        focus={focus}
        profileId={profileId}
        scopeLabel={profileId ?? 'Archive-wide'}
      />
    </MemoryRouter>,
  )
}

function section<T>(data: T): CoreIntelligenceSectionResult<T> {
  return {
    data,
    meta: meta(),
  }
}

function meta(): CoreIntelligenceSectionMeta {
  return {
    sectionId: 'domain-deep-dive',
    generatedAt: '2026-04-25T12:00:00Z',
    window: {
      kind: 'date-range',
      dateRange,
    },
    moduleIds: ['domain-deep-dive'],
    sourceTables: ['domain_daily_rollups'],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: [],
  }
}

function domainFixture(
  overrides: Partial<DomainDeepDive> = {},
): DomainDeepDive {
  return {
    registrableDomain: 'example.com',
    displayName: 'Example',
    domainCategory: 'reference',
    totalVisits: 1_200_000,
    activeDays: 12,
    trailCount: 4,
    arrivalBreakdown: {
      search: 0,
      link: 0,
      typed: 0,
      other: 0,
    },
    topPages: [],
    topReferrers: [
      {
        domain: 'referrer.test',
        displayName: null,
        count: 4,
      },
    ],
    topExits: [
      {
        domain: 'exit.test',
        displayName: 'Exit',
        count: 3,
      },
    ],
    visitTrend: [
      {
        dateKey: '2026-04-20',
        visitCount: 6,
      },
    ],
    ...overrides,
  }
}

function compareSetDetailFixture(
  pages: Array<{
    canonicalUrl: string | null
    registrableDomain: string
  }>,
): CompareSetDetail {
  return {
    compareSet: {
      compareSetId: 'compare-1',
      trailId: 'trail-1',
      searchQuery: 'example docs',
      pageCategory: 'reference',
      pages: pages.map((page, index) => ({
        canonicalUrl: page.canonicalUrl as string,
        url: page.canonicalUrl ?? `https://example.com/fallback-${index}`,
        title: `Page ${index + 1}`,
        registrableDomain: page.registrableDomain,
        visitCount: index + 1,
        isLanding: index === 0,
      })),
    },
    recentDays: ['2026-04-20'],
    trail: {
      trailId: 'trail-1',
      sessionId: 'session-1',
      initialQuery: 'example docs',
      searchEngine: 'Google',
      reformulationCount: 0,
      visitCount: 2,
      landingUrl: 'https://example.com/docs',
      landingDomain: 'example.com',
      firstVisitMs: 1,
      lastVisitMs: 2,
      maxDepth: 2,
      queries: ['example docs'],
    },
    session: null,
  }
}
