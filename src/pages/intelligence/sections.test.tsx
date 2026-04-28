/**
 * @file sections.test.tsx
 * @description Coordinator-level coverage for the Core Intelligence overview sections.
 * @module pages/intelligence
 *
 * ## Responsibilities
 * - Verify page-level section orchestration that is not owned by child sections.
 * - Cover digest fallback, secondary readiness skeletons, and Top Sites controls.
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
  TopSite,
} from '../../lib/core-intelligence'
import * as api from '../../lib/core-intelligence/api'
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
  })

  test('renders digest fallback and secondary skeletons while deferred sections warm', () => {
    vi.mocked(api.peekDigestSummary).mockReturnValue(
      section(null as unknown as DigestSummary, 'digest-summary'),
    )

    const { container } = renderSections({ secondaryReady: false })

    expect(screen.getByText('Digest')).toBeInTheDocument()
    expect(screen.getByText('Digest unavailable')).toBeInTheDocument()
    expect(screen.getByText('Top sites')).toBeInTheDocument()
    expect(screen.getByText('Refind pages')).toBeInTheDocument()
    expect(screen.queryByText('stable sources')).not.toBeInTheDocument()
    expect(
      container.querySelectorAll(
        '.intelligence-secondary-grid .intelligence-skeleton--card',
      ),
    ).toHaveLength(4)
  })

  test('filters and sorts Top Sites without changing the route factories', async () => {
    const user = userEvent.setup()
    renderSections({ secondaryReady: true })

    expect(screen.getByRole('link', { name: /Example/ })).toHaveAttribute(
      'href',
      '/domain/example.com',
    )
    expect(screen.getByRole('link', { name: /Docs/ })).toHaveAttribute(
      'href',
      '/domain/docs.example',
    )
    expect(screen.getByText('stable sources')).toBeInTheDocument()

    await user.type(
      screen.getByRole('searchbox', { name: 'Search sites' }),
      'docs',
    )
    expect(
      screen.queryByRole('link', { name: /Example/ }),
    ).not.toBeInTheDocument()
    expect(screen.getByRole('link', { name: /Docs/ })).toBeInTheDocument()

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

    const { container } = renderSections({ secondaryReady: true })
    await user.type(
      screen.getByRole('searchbox', { name: 'Search sites' }),
      'zero.example',
    )

    expect(
      screen
        .getAllByRole('link', { name: /zero\.example/ })
        .some((link) => link.getAttribute('href') === '/domain/zero.example'),
    ).toBe(true)
    expect(
      container.querySelector<HTMLElement>('.top-site-row__bar-fill')?.style
        .width,
    ).toBe('0%')
    expect(
      screen.getByRole('link', { name: 'https://zero.example/page' }),
    ).toHaveAttribute('href', '/refind/https%3A%2F%2Fzero.example%2Fpage')

    await user.clear(screen.getByRole('searchbox', { name: 'Search sites' }))
    await user.type(
      screen.getByRole('searchbox', { name: 'Search sites' }),
      'missing',
    )
    expect(
      screen
        .queryAllByRole('link', { name: /zero\.example/ })
        .some((link) => link.getAttribute('href') === '/domain/zero.example'),
    ).toBe(false)
    expect(
      screen
        .getAllByRole('link', { name: /zero\.example/ })
        .some(
          (link) =>
            link.getAttribute('href') ===
            '/refind/https%3A%2F%2Fzero.example%2Fpage',
        ),
    ).toBe(true)
  })
})

describe('IntelligenceSectionsSkeleton', () => {
  test('renders the full page skeleton structure', () => {
    const { container } = render(<IntelligenceSectionsSkeleton />)

    expect(container.querySelector('.digest-section')).toBeInTheDocument()
    expect(container.querySelector('.rhythm-section')).toBeInTheDocument()
    expect(
      container.querySelectorAll(
        '.intelligence-secondary-grid .intelligence-skeleton--card',
      ),
    ).toHaveLength(6)
  })
})

function renderSections({ secondaryReady }: { secondaryReady: boolean }) {
  return render(
    <MemoryRouter>
      <IntelligenceSections
        compareSetHref={(compareSetId) => `/compare/${compareSetId}`}
        dashboard={null}
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
        secondaryReady={secondaryReady}
        scopeLabel="Chrome Default"
        trailHref={(trailId) => `/trail/${trailId}`}
        t={translate}
      />
    </MemoryRouter>,
  )
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
    default:
      return key
  }
}
