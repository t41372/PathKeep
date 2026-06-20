/**
 * @file sections.test.tsx
 * @description Coordinator-level coverage for the newspaper-style Intelligence hub layout.
 * @module pages/intelligence
 *
 * ## Responsibilities
 * - Verify the 3-layer hub orchestration: digest above fold, axis cards, the
 *   always-visible inline secondary grid.
 * - Cover digest fallback, secondary readiness skeletons, and Top Sites controls.
 * - Verify time-pattern strips render when rhythm data is available.
 * - Keep expensive secondary sections mocked so this suite stays focused and fast.
 *
 * ## Not responsible for
 * - Re-testing every secondary section's internal rendering.
 * - Re-testing shared metric, meta, and workbench components.
 *
 * ## Dependencies
 * - Mocks Core Intelligence API cache/read functions.
 * - Uses React Router because the coordinator owns deep-link factories.
 *
 * ## Performance notes
 * - Cached section payloads avoid async waterfall work while still exercising the
 *   route-owned filtering and sorting state transitions.
 */

import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
  DigestSummary,
  QueryFamily,
  QueryFamilyResult,
  TopSite,
} from '../../lib/core-intelligence'
import type { StableSource } from '../../lib/core-intelligence/types-analysis'
import * as api from '../../lib/core-intelligence/api'
import type { DashboardSnapshot } from '../../lib/types'
import { IntelligenceSections, IntelligenceSectionsSkeleton } from './sections'

vi.mock('../../components/intelligence/section-meta', () => ({
  IntelligenceSectionMeta: ({ scopeLabel }: { scopeLabel: string }) => (
    <span>{scopeLabel}</span>
  ),
}))

vi.mock('./sections/health', () => ({
  GrowthSignalSection: () => <section>growth signal</section>,
  StorageAnalyticsSection: () => <section>storage analytics</section>,
}))

vi.mock('./sections/search-and-activity-section', () => ({
  ActivityMixSection: () => <section>activity mix</section>,
  SearchActivitySection: () => <section>search activity</section>,
}))

vi.mock('./sections/browsing-rhythm-section', () => ({
  BrowsingRhythmSection: () => <section>browsing rhythm</section>,
}))

vi.mock('./sections/secondary-sections', () => ({
  BreadthIndexSection: () => <section>breadth index</section>,
  CompareSetsSection: () => <section>compare sets</section>,
  DiscoveryTrendSection: () => <section>discovery trend</section>,
  FrictionDetectionSection: () => <section>friction detection</section>,
  HabitsSection: () => <section>habits</section>,
  MultiBrowserDiffSection: () => <section>multi-browser diff</section>,
  ObservedInteractionsSection: () => <section>observed interactions</section>,
  PathFlowsSection: () => <section>path flows</section>,
  ReopenedInvestigationsSection: () => (
    <section>reopened investigations</section>
  ),
  SearchEffectivenessSection: () => <section>search effectiveness</section>,
  StableSourcesSection: () => <section>stable sources</section>,
}))

const dateRange: DateRange = { start: '2026-04-01', end: '2026-04-30' }

describe('IntelligenceSections', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'getDigestSummary').mockResolvedValue(
      section(digestFixture(), 'digest-summary'),
    )
    vi.spyOn(api, 'peekDigestSummary').mockReturnValue(
      section(digestFixture(), 'digest-summary'),
    )
    vi.spyOn(api, 'getTopSites').mockResolvedValue(
      section(topSitesFixture(), 'top-sites'),
    )
    vi.spyOn(api, 'peekTopSites').mockReturnValue(
      section(topSitesFixture(), 'top-sites'),
    )
    vi.spyOn(api, 'getRefindPages').mockResolvedValue(
      section([], 'refind-pages'),
    )
    vi.spyOn(api, 'peekRefindPages').mockReturnValue(
      section([], 'refind-pages'),
    )
    vi.spyOn(api, 'getBrowsingRhythm').mockResolvedValue(
      section({ cells: [], maxCount: 0 }, 'browsing-rhythm'),
    )
    vi.spyOn(api, 'peekStableSources').mockReturnValue(null)
    vi.spyOn(api, 'peekSearchEffectiveness').mockReturnValue(null)
    vi.spyOn(api, 'peekFrictionSignals').mockReturnValue(null)
    vi.spyOn(api, 'peekHabitPatterns').mockReturnValue(null)
    vi.spyOn(api, 'peekInterruptedHabits').mockReturnValue(null)
    vi.spyOn(api, 'peekReopenedInvestigations').mockReturnValue(null)
    vi.spyOn(api, 'peekDiscoveryTrend').mockReturnValue(null)
    vi.spyOn(api, 'peekBreadthIndex').mockReturnValue(null)
    vi.spyOn(api, 'peekPathFlows').mockReturnValue(null)
    vi.spyOn(api, 'peekCompareSets').mockReturnValue(null)
    vi.spyOn(api, 'peekMultiBrowserDiff').mockReturnValue(null)
    vi.spyOn(api, 'peekObservedInteractions').mockReturnValue(null)
    vi.spyOn(api, 'peekQueryFamilies').mockReturnValue(null)
  })

  test('keeps not-ready secondary sections as skeletons until data warms', () => {
    vi.mocked(api.peekDigestSummary).mockReturnValue(
      section(null as unknown as DigestSummary, 'digest-summary'),
    )

    renderSections({ secondaryReady: false })

    expect(screen.getByText('Digest')).toBeInTheDocument()
    expect(screen.getByText('Digest unavailable')).toBeInTheDocument()
    expect(screen.getByText('Top sites')).toBeInTheDocument()
    expect(screen.getByText('Refind pages')).toBeInTheDocument()
    // Sections render inline (no disclosure), but with secondaryReady=false and
    // no cached peek the section keeps showing its skeleton placeholder, so the
    // heavy real section is not mounted yet.
    expect(screen.queryByText('search effectiveness')).not.toBeInTheDocument()
    expect(
      screen.getByTestId('secondary-section-skeleton-search-effectiveness'),
    ).toBeInTheDocument()
    // The disclosure toggle no longer exists — everything is inline.
    expect(screen.queryByTestId('hub-secondary-toggle')).not.toBeInTheDocument()
  })

  test('shows an error and retry surface (not a silent skeleton) when the secondary batch fails with no warm cache', async () => {
    const user = userEvent.setup()
    const onRetrySecondary = vi.fn()

    renderSections({
      secondaryReady: false,
      secondaryError: 'secondary batch failed',
      onRetrySecondary,
    })

    // The no-cache slot surfaces an announced error with a retry instead of an
    // inert, never-resolving skeleton.
    const errorSurface = screen.getByTestId(
      'secondary-section-error-search-effectiveness',
    )
    expect(errorSurface).toBeInTheDocument()
    expect(
      screen.queryByTestId('secondary-section-skeleton-search-effectiveness'),
    ).not.toBeInTheDocument()
    expect(within(errorSurface).getByRole('alert')).toBeInTheDocument()

    await user.click(
      within(errorSurface).getByRole('button', { name: 'Retry' }),
    )
    expect(onRetrySecondary).toHaveBeenCalledTimes(1)
  })

  test('keeps a cached secondary section mounted even when the batch reports an error', () => {
    // A slot with warm cache must still render its real node, not the error
    // surface, because the batch error does not affect its already-loaded data.
    vi.mocked(api.peekBreadthIndex).mockReturnValue(
      section({} as never, 'breadth-index'),
    )

    renderSections({
      secondaryReady: false,
      secondaryError: 'secondary batch failed',
    })

    expect(screen.getByText('breadth index')).toBeInTheDocument()
    expect(
      screen.queryByTestId('secondary-section-error-breadth-index'),
    ).not.toBeInTheDocument()
  })

  test('renders all secondary sections inline once secondaryReady flips', () => {
    vi.spyOn(api, 'peekStableSources').mockReturnValue(
      section([], 'stable-sources'),
    )

    renderSections({ secondaryReady: true })

    // stable-sources is in the main axis stack and the secondary grid.
    expect(screen.getAllByText('stable sources').length).toBeGreaterThan(0)
    // No expand step needed — secondary sections are visible inline. (jsdom has
    // no IntersectionObserver, so LazySection mounts children immediately.)
    expect(screen.getByText('search effectiveness')).toBeInTheDocument()
    expect(screen.getByText('friction detection')).toBeInTheDocument()
  })

  test('drops a hide-when-empty section from the grid when its cache is ready and empty', () => {
    // search-effectiveness reports a ready snapshot with every list empty, so its
    // hasContent predicate returns false and the slot is dropped entirely — no
    // card and no skeleton — rather than leaving a blank grid cell. A sibling
    // with content still renders, proving neighbors fill the vacated space.
    vi.mocked(api.peekSearchEffectiveness).mockReturnValue(
      section(
        {
          engineStats: [],
          topResolvingSources: [],
          hardestTopics: [],
        } as never,
        'search-effectiveness',
      ),
    )
    vi.mocked(api.peekBreadthIndex).mockReturnValue(
      section({} as never, 'breadth-index'),
    )

    renderSections({ secondaryReady: true })

    expect(screen.queryByText('search effectiveness')).not.toBeInTheDocument()
    expect(
      screen.queryByTestId('secondary-section-skeleton-search-effectiveness'),
    ).not.toBeInTheDocument()
    expect(screen.getByText('breadth index')).toBeInTheDocument()
  })

  test('mounts a cached secondary section even when secondaryReady is false', () => {
    // breadth-index has a cached peek, so its slot.isReady() is true and it
    // mounts inline up front while the still-warming siblings stay skeletons.
    vi.mocked(api.peekBreadthIndex).mockReturnValue(
      section({} as never, 'breadth-index'),
    )

    renderSections({ secondaryReady: false })

    expect(screen.getByText('breadth index')).toBeInTheDocument()
    expect(screen.queryByText('search effectiveness')).not.toBeInTheDocument()
  })

  test('renders cached health cards independently while deferred secondary cards warm', () => {
    renderSections({
      dashboard: dashboardFixture(),
      secondaryReady: false,
    })

    // Health sections render inline; with a dashboard the primary health slots
    // report ready (isReady === true) and mount immediately.
    expect(screen.getByText('storage analytics')).toBeInTheDocument()
    expect(screen.getByText('growth signal')).toBeInTheDocument()
  })

  test('filters and sorts Top Sites without changing the route factories', async () => {
    const user = userEvent.setup()
    renderSections({ secondaryReady: true })

    // Links appear in both the preview card and the full section
    const exampleLinks = screen.getAllByRole('link', { name: /Example/ })
    expect(
      exampleLinks.some(
        (link) => link.getAttribute('href') === '/domain/example.com',
      ),
    ).toBe(true)
    const docsLinks = screen.getAllByRole('link', { name: /Docs/ })
    expect(
      docsLinks.some(
        (link) => link.getAttribute('href') === '/domain/docs.example',
      ),
    ).toBe(true)

    await user.type(
      screen.getByRole('searchbox', { name: 'Search sites' }),
      'docs',
    )
    // After filtering, the full section hides "Example" but the preview card
    // still shows it. Find the top-sites section scope and verify within it.
    const topSitesSection = screen
      .getByRole('heading', { name: 'Top sites' })
      .closest('section')!
    expect(
      within(topSitesSection)
        .queryAllByRole('link')
        .some((link) => link.textContent?.includes('Example')),
    ).toBe(false)
    expect(
      within(topSitesSection)
        .queryAllByRole('link')
        .some((link) => link.textContent?.includes('Docs')),
    ).toBe(true)

    await user.clear(screen.getByRole('searchbox', { name: 'Search sites' }))
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Sort sites' }),
      ['unique_days'],
    )
    const topSites = screen
      .getByRole('heading', { name: 'Top sites' })
      .closest('section')
    if (!topSites) {
      throw new Error('Expected Top Sites section to render')
    }
    expect(within(topSites).getByText('5 days')).toBeInTheDocument()

    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Sort sites' }),
      ['avg_daily'],
    )
    expect(within(topSites).getByText(/2\.4\s+\/day/)).toBeInTheDocument()
  })

  test('renders Top Sites and Refind fallback labels for sparse data', async () => {
    const user = userEvent.setup()
    vi.mocked(api.peekTopSites).mockReturnValue(
      section(
        [
          {
            averageDailyVisits: 0,
            displayName: null,
            domainCategory: 'uncategorized',
            registrableDomain: 'zero.example',
            uniqueDays: 0,
            uniqueUrls: 0,
            visitCount: 0,
          },
        ],
        'top-sites',
      ),
    )
    vi.mocked(api.getTopSites).mockResolvedValue(
      section(
        [
          {
            averageDailyVisits: 0,
            displayName: null,
            domainCategory: 'uncategorized',
            registrableDomain: 'zero.example',
            uniqueDays: 0,
            uniqueUrls: 0,
            visitCount: 0,
          },
        ],
        'top-sites',
      ),
    )
    vi.mocked(api.peekRefindPages).mockReturnValue(
      section([refindPageFixture()], 'refind-pages'),
    )
    vi.mocked(api.getRefindPages).mockResolvedValue(
      section([refindPageFixture()], 'refind-pages'),
    )

    renderSections({ secondaryReady: true })
    await user.type(
      screen.getByRole('searchbox', { name: 'Search sites' }),
      'zero.example',
    )

    expect(
      screen
        .getAllByRole('link', { name: /zero\.example/ })
        .some((link) => link.getAttribute('href') === '/domain/zero.example'),
    ).toBe(true)

    // Refind page link appears in both the preview card and the full section
    const refindLinks = screen.getAllByRole('link', {
      name: 'https://zero.example/page',
    })
    expect(
      refindLinks.some(
        (link) =>
          link.getAttribute('href') ===
          '/refind/https%3A%2F%2Fzero.example%2Fpage',
      ),
    ).toBe(true)

    await user.clear(screen.getByRole('searchbox', { name: 'Search sites' }))
    await user.type(
      screen.getByRole('searchbox', { name: 'Search sites' }),
      'missing',
    )
    // The full Top Sites section hides the domain link when filtering by "missing"
    const topSitesSectionAfter = screen
      .getByRole('heading', { name: 'Top sites' })
      .closest('section')!
    expect(
      within(topSitesSectionAfter)
        .queryAllByRole('link', { name: /zero\.example/ })
        .some((link) => link.getAttribute('href') === '/domain/zero.example'),
    ).toBe(false)
    // Refind links remain visible (in axis card preview and full section)
    const refindLinksAfterFilter = screen
      .getAllByRole('link', { name: /zero\.example/ })
      .filter(
        (link) =>
          link.getAttribute('href') ===
          '/refind/https%3A%2F%2Fzero.example%2Fpage',
      )
    expect(refindLinksAfterFilter.length).toBeGreaterThan(0)
  })

  test('renders axis cards for Time, Sources, and Research', () => {
    renderSections({ secondaryReady: true })

    expect(screen.getByTestId('hub-axis-time')).toBeInTheDocument()
    expect(screen.getByTestId('hub-axis-sources')).toBeInTheDocument()
    expect(screen.getByTestId('hub-axis-research')).toBeInTheDocument()
  })

  test('renders the stable-sources preview, falling back to the domain when a source has no display name', () => {
    vi.mocked(api.peekStableSources).mockReturnValue(
      section(
        [
          stableSourceFixture('docs.example', 'Docs Hub'),
          stableSourceFixture('api.example', null),
        ],
        'stable-sources',
      ),
    )

    renderSections({ secondaryReady: true })

    const sourcesAxis = screen.getByTestId('hub-axis-sources')
    // Named source renders its display name; the unnamed one falls back to the
    // registrable domain (the `displayName ?? registrableDomain` branch).
    const named = within(sourcesAxis).getByRole('link', { name: 'Docs Hub' })
    expect(named).toHaveAttribute('href', '/domain/docs.example')
    const fallback = within(sourcesAxis).getByRole('link', {
      name: 'api.example',
    })
    expect(fallback).toHaveAttribute('href', '/domain/api.example')
  })

  test('renders the query-families preview inside the Research axis', () => {
    vi.mocked(api.peekQueryFamilies).mockReturnValue(
      section(
        queryFamilyResultFixture([
          queryFamilyFixture('family-7', 'tauri sqlite', 2),
        ]),
        'query-families',
      ),
    )

    renderSections({ secondaryReady: true })

    const researchAxis = screen.getByTestId('hub-axis-research')
    const familyLink = within(researchAxis).getByRole('link', {
      name: 'tauri sqlite',
    })
    expect(familyLink).toHaveAttribute('href', '/query/family-7')
  })

  test('renders the spotlight query-family sentence when no refind page qualifies', () => {
    // Default refind fixture has crossDayCount 0, so the refind branch fails and
    // the spotlight falls through to the most-reformulated query family.
    vi.mocked(api.peekRefindPages).mockReturnValue(
      section([refindPageFixture()], 'refind-pages'),
    )
    vi.mocked(api.peekQueryFamilies).mockReturnValue(
      section(
        queryFamilyResultFixture([
          queryFamilyFixture('family-9', 'rust async', 4),
        ]),
        'query-families',
      ),
    )

    renderSections({ secondaryReady: true })

    expect(screen.getByTestId('hub-spotlight')).toBeInTheDocument()
    expect(
      screen.getByText('You searched for "rust async" 4 ways'),
    ).toBeInTheDocument()
  })

  test('renders SpotlightCard when a top refind page has crossDayCount >= 2', () => {
    vi.mocked(api.peekRefindPages).mockReturnValue(
      section(
        [
          {
            ...refindPageFixture(),
            title: 'My Favorite Page',
            crossDayCount: 5,
            refindScore: 8.5,
          },
        ],
        'refind-pages',
      ),
    )
    vi.mocked(api.getRefindPages).mockResolvedValue(
      section(
        [
          {
            ...refindPageFixture(),
            title: 'My Favorite Page',
            crossDayCount: 5,
            refindScore: 8.5,
          },
        ],
        'refind-pages',
      ),
    )

    renderSections({ secondaryReady: true })

    expect(screen.getByTestId('hub-spotlight')).toBeInTheDocument()
    expect(
      screen.getByText('You revisited "My Favorite Page" across 5 days'),
    ).toBeInTheDocument()
  })

  test('falls back to the refind URL in the spotlight when the page has no title', () => {
    vi.mocked(api.peekRefindPages).mockReturnValue(
      section(
        [
          {
            ...refindPageFixture(),
            title: null,
            crossDayCount: 3,
            refindScore: 7,
          },
        ],
        'refind-pages',
      ),
    )

    renderSections({ secondaryReady: true })

    expect(screen.getByTestId('hub-spotlight')).toBeInTheDocument()
    // title is null, so the spotlight sentence falls back to the canonical URL.
    expect(
      screen.getByText(
        'You revisited "https://zero.example/page" across 3 days',
      ),
    ).toBeInTheDocument()
  })

  test('does not render SpotlightCard when no data is compelling', () => {
    renderSections({ secondaryReady: true })

    expect(screen.queryByTestId('hub-spotlight')).not.toBeInTheDocument()
  })

  test('renders weekday/weekend and peak hours strips when rhythm data is empty', () => {
    renderSections({ secondaryReady: true })

    expect(screen.getByTestId('weekday-weekend-strip')).toBeInTheDocument()
    expect(screen.getByTestId('peak-hours-strip')).toBeInTheDocument()
  })
})

describe('IntelligenceSectionsSkeleton', () => {
  test('renders the full page skeleton structure', () => {
    const { container } = render(<IntelligenceSectionsSkeleton />)

    expect(container.querySelector('.digest-section')).toBeInTheDocument()
    expect(container.querySelector('.rhythm-section')).toBeInTheDocument()
  })
})

function renderSections({
  dashboard = null,
  secondaryReady,
  secondaryError = null,
  onRetrySecondary = vi.fn(),
}: {
  dashboard?: DashboardSnapshot | null
  secondaryReady: boolean
  secondaryError?: string | null
  onRetrySecondary?: () => void
}) {
  return render(
    <MemoryRouter>
      <IntelligenceSections
        compareSetHref={(compareSetId) => `/compare/${compareSetId}`}
        dashboard={dashboard}
        dateRange={dateRange}
        dayHref={(date) => `/day/${date}`}
        domainHref={(domain) => `/domain/${domain}`}
        focusedDomainHref={(domain, focus) =>
          `/domain/${domain}?focus=${focus.focusType}:${focus.focusId}`
        }
        language="en"
        preset="month"
        profileId="chrome:Default"
        queryFamilyHref={(familyId) => `/query/${familyId}`}
        refindHref={(canonicalUrl) =>
          `/refind/${encodeURIComponent(canonicalUrl)}`
        }
        secondaryError={secondaryError}
        secondaryReady={secondaryReady}
        onRetrySecondary={onRetrySecondary}
        scopeLabel="Chrome Default"
        trailHref={(trailId) => `/trail/${trailId}`}
        t={translate}
      />
    </MemoryRouter>,
  )
}

function dashboardFixture(): DashboardSnapshot {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    lastSuccessfulBackupAt: '2026-04-25T12:00:00Z',
    recentRuns: [
      {
        id: 42,
        finishedAt: '2026-04-25T12:00:00Z',
        newDownloads: 1,
        newUrls: 2,
        newVisits: 3,
        profilesProcessed: 1,
        startedAt: '2026-04-25T11:59:00Z',
        status: 'success',
      },
    ],
    storage: {
      archiveDatabaseBytes: 10,
      exportBytes: 0,
      intelligenceBlobBytes: 0,
      intelligenceDatabaseBytes: 0,
      manifestBytes: 0,
      quarantineBytes: 0,
      searchDatabaseBytes: 0,
      semanticSidecarBytes: 0,
      snapshotBytes: 0,
      sourceEvidenceDatabaseBytes: 5,
      stagingBytes: 0,
    },
    totalDownloads: 1,
    totalProfiles: 1,
    totalUrls: 2,
    totalVisits: 3,
  }
}

function section<T>(
  data: T,
  sectionId: string,
): CoreIntelligenceSectionResult<T> {
  return {
    data,
    meta: metaFixture(sectionId),
  }
}

function metaFixture(sectionId: string): CoreIntelligenceSectionMeta {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    includesEnrichment: false,
    moduleIds: [sectionId],
    notes: [],
    sectionId,
    sourceTables: ['core_intelligence'],
    state: 'ready',
    stateReason: null,
    window: {
      dateRange,
      kind: 'date-range',
    },
  }
}

function digestFixture(): DigestSummary {
  return {
    dateRange,
    deepReadPages: metric(4),
    newDomains: metric(3),
    refindPages: metric(2),
    totalSearches: metric(8),
    totalVisits: metric(42),
  }
}

function topSitesFixture(): TopSite[] {
  return [
    {
      averageDailyVisits: 2.4,
      displayName: 'Example',
      domainCategory: 'work',
      registrableDomain: 'example.com',
      uniqueDays: 5,
      uniqueUrls: 8,
      visitCount: 12,
    },
    {
      averageDailyVisits: 1.25,
      displayName: 'Docs',
      domainCategory: 'learning',
      registrableDomain: 'docs.example',
      uniqueDays: 4,
      uniqueUrls: 5,
      visitCount: 6,
    },
  ]
}

function refindPageFixture() {
  return {
    canonicalUrl: 'https://zero.example/page',
    crossDayCount: 0,
    firstSeenAt: '2026-04-01T00:00:00.000Z',
    lastSeenAt: '2026-04-02T00:00:00.000Z',
    refindScore: 0,
    registrableDomain: 'zero.example',
    searchArrivalCount: 0,
    title: null,
    trailCount: 0,
    typedRevisitCount: 0,
    url: 'https://zero.example/page',
  }
}

function stableSourceFixture(
  registrableDomain: string,
  displayName: string | null,
): StableSource {
  return {
    displayName,
    effectivenessScore: 0.8,
    registrableDomain,
    sourceRole: 'reference',
    stableLandingCount: 3,
    trailCount: 4,
  }
}

function queryFamilyFixture(
  familyId: string,
  anchorQuery: string,
  memberCount: number,
): QueryFamily {
  return {
    anchorQuery,
    familyId,
    firstSeenAt: '2026-04-01T00:00:00.000Z',
    lastSeenAt: '2026-04-25T00:00:00.000Z',
    memberCount,
    queries: [anchorQuery],
    searchEngine: 'google',
  }
}

function queryFamilyResultFixture(families: QueryFamily[]): QueryFamilyResult {
  return {
    families,
    page: 0,
    pageSize: 10,
    total: families.length,
  }
}

function metric(value: number) {
  return {
    changePercent: 0,
    previousValue: value,
    trend: 'flat' as const,
    value,
  }
}

function translate(key: string, vars?: Record<string, string | number>) {
  switch (key) {
    case 'digestTitle':
      return 'Digest'
    case 'digestUnavailable':
      return 'Digest unavailable'
    case 'digestVisits':
      return 'Visits'
    case 'digestSearches':
      return 'Searches'
    case 'digestNewSites':
      return 'New sites'
    case 'digestDeepRead':
      return 'Deep read'
    case 'digestRefind':
      return 'Refind'
    case 'topSitesTitle':
      return 'Top sites'
    case 'topSitesSearch':
      return 'Search sites'
    case 'topSitesSort':
      return 'Sort sites'
    case 'topSitesSortVisits':
      return 'Visits'
    case 'topSitesSortDays':
      return 'Days'
    case 'topSitesSortAvg':
      return 'Average'
    case 'topSitesDays':
      return 'days'
    case 'topSitesAvgSuffix':
      return '/day'
    case 'topSitesEmpty':
      return 'No sites'
    case 'visits':
      return 'visits'
    case 'refindTitle':
      return 'Refind pages'
    case 'refindEmpty':
      return 'No refind pages'
    case 'sectionGeneratedAt':
      return `Generated ${vars?.time ?? ''}`
    case 'archiveWideBadge':
      return 'Archive-wide'
    case 'scopedBadge':
      return String(vars?.scope ?? 'Scoped')
    case 'readyBadge':
      return 'Ready'
    case 'staleBadge':
      return 'Stale'
    case 'disabledBadge':
      return 'Disabled'
    case 'degradedBadge':
      return 'Degraded'
    case 'enrichmentBadge':
      return 'Enriched'
    case 'noEnrichmentBadge':
      return 'Deterministic'
    case 'hubTimeAxisTitle':
      return 'Time'
    case 'hubSourcesAxisTitle':
      return 'Sources'
    case 'hubResearchAxisTitle':
      return 'Research'
    case 'hubSeeAll':
      return 'See all'
    case 'hubWeekdayLabel':
      return 'Weekdays'
    case 'hubWeekendLabel':
      return 'Weekends'
    case 'hubPeakHoursTitle':
      return 'Peak Hours'
    case 'hubPeakHoursEmpty':
      return 'Not enough data for peak hours'
    case 'hubWeekdayWeekendEmpty':
      return 'Not enough daily data'
    case 'hubTopSitesPreview':
      return 'Top Sites'
    case 'hubStableSourcesPreview':
      return 'Stable Sources'
    case 'hubQueryFamiliesPreview':
      return 'Query Families'
    case 'hubRefindPreview':
      return 'Refind Pages'
    case 'hubSpotlightRefind':
      return `You revisited "${vars?.title}" across ${vars?.days} days`
    case 'hubSpotlightQueryFamily':
      return `You searched for "${vars?.query}" ${vars?.count} ways`
    case 'secondarySectionErrorTitle':
      return 'This insight could not load'
    case 'secondarySectionErrorBody':
      return 'PathKeep could not load this section.'
    case 'secondarySectionRetry':
      return 'Retry'
    default:
      return key
  }
}
