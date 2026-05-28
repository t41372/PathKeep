import { describe, expect, test, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import type {
  CoreIntelligencePrimaryOverview,
  RefindPage,
  TopSite,
} from '../../lib/core-intelligence/types-overview'
import { PaperIntelligencePanel } from './paper-intelligence-panel'

function explorerT(key: string, vars?: Record<string, string | number>) {
  return vars ? `${key}:${JSON.stringify(vars)}` : key
}

function makeTopSite(over: Partial<TopSite> = {}): TopSite {
  return {
    registrableDomain: 'example.com',
    displayName: 'Example',
    visitCount: 120,
    weight: 1,
    profilesSeen: ['chrome:Default'],
    ...over,
  } as TopSite
}

function makeRefindPage(over: Partial<RefindPage> = {}): RefindPage {
  return {
    canonicalUrl: 'https://example.com/x',
    url: 'https://example.com/x',
    title: 'X',
    registrableDomain: 'example.com',
    crossDayCount: 3,
    trailCount: 2,
    profilesSeen: ['chrome:Default'],
    ...over,
  } as RefindPage
}

function makeOverview(
  topSites: TopSite[],
  refindPages: RefindPage[],
): CoreIntelligencePrimaryOverview {
  const section = <T,>(data: T) =>
    ({ data, status: 'ready', degradedReason: null, source: 'live' }) as never
  return {
    digestSummary: section({}),
    onThisDay: section([]),
    topSites: section(topSites),
    refindPages: section(refindPages),
    searchEngineRanking: section([]),
    topSearchConcepts: section([]),
    queryFamilies: section({ families: [], topQueries: [] }),
    activityMix: section({}),
    discoveryTrendDay: section({}),
    habitPatterns: section([]),
    interruptedHabits: section([]),
    timings: [],
    totalDurationMs: 0,
  } as unknown as CoreIntelligencePrimaryOverview
}

const baseDashboard = {
  totalVisits: 12_345,
  totalUrls: 4_321,
  storage: {},
  lastSuccessfulBackupAt: null,
  recentRuns: [{}, {}, {}],
} as never

describe('PaperIntelligencePanel', () => {
  test('renders 4 KPI cells from dashboard + domain stats', () => {
    const overview = makeOverview(
      [
        makeTopSite({ registrableDomain: 'rust-lang.org', visitCount: 60 }),
        makeTopSite({ registrableDomain: 'github.com', visitCount: 40 }),
      ],
      [makeRefindPage()],
    )
    render(
      <PaperIntelligencePanel
        primaryOverview={overview}
        dashboard={baseDashboard}
        onSelectDomain={() => {}}
        explorerT={explorerT}
      />,
    )
    expect(screen.getByTestId('paper-intelligence-panel')).toBeInTheDocument()
    expect(screen.getByTestId('paper-intelligence-view')).toBeInTheDocument()
    // totalVisits formatted with locale separators
    expect(screen.getByText('12,345')).toBeInTheDocument()
    // top domain wins the second KPI (also shows in the rank list, so >= 1)
    expect(screen.getAllByText('rust-lang.org').length).toBeGreaterThanOrEqual(
      1,
    )
    // The "1" appears in active-threads count + rank ordering — assert the
    // refind item count surfaces somewhere.
    expect(screen.getAllByText('1').length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByText('3').length).toBeGreaterThanOrEqual(1)
  })

  test('falls back to em-dash when there are no top domains', () => {
    const overview = makeOverview([], [])
    render(
      <PaperIntelligencePanel
        primaryOverview={overview}
        dashboard={baseDashboard}
        onSelectDomain={() => {}}
        explorerT={explorerT}
      />,
    )
    expect(screen.getByText('—')).toBeInTheDocument()
  })

  test('clicking a domain rank row calls onSelectDomain', () => {
    const overview = makeOverview(
      [makeTopSite({ registrableDomain: 'sqlite.org', visitCount: 20 })],
      [],
    )
    const onSelectDomain = vi.fn()
    render(
      <PaperIntelligencePanel
        primaryOverview={overview}
        dashboard={baseDashboard}
        onSelectDomain={onSelectDomain}
        explorerT={explorerT}
      />,
    )
    // The domain string appears in both the KPI cell and the rank list;
    // we want to click the one that's a button inside the rank list.
    const ranks = screen
      .getAllByText('sqlite.org')
      .map((node) => node.closest('button'))
      .filter((node): node is HTMLButtonElement => node !== null)
    expect(ranks.length).toBeGreaterThan(0)
    fireEvent.click(ranks[0])
    expect(onSelectDomain).toHaveBeenCalledWith('sqlite.org')
  })

  test('survives a null primaryOverview', () => {
    render(
      <PaperIntelligencePanel
        primaryOverview={null}
        dashboard={baseDashboard}
        onSelectDomain={() => {}}
        explorerT={explorerT}
      />,
    )
    expect(screen.getByTestId('paper-intelligence-panel')).toBeInTheDocument()
  })

  test('survives a null dashboard', () => {
    const overview = makeOverview([], [])
    render(
      <PaperIntelligencePanel
        primaryOverview={overview}
        dashboard={null}
        onSelectDomain={() => {}}
        explorerT={explorerT}
      />,
    )
    expect(screen.getByTestId('paper-intelligence-panel')).toBeInTheDocument()
    // dashboard.totalVisits → 0; recentRuns.length → 0; both render as "0"
    expect(screen.getAllByText('0').length).toBeGreaterThanOrEqual(1)
  })

  test('renders refindItems with formatted meta string', () => {
    const overview = makeOverview(
      [],
      [
        makeRefindPage({
          canonicalUrl: 'https://docs.rs/tokio',
          title: 'tokio docs',
          crossDayCount: 5,
          trailCount: 4,
        }),
      ],
    )
    render(
      <PaperIntelligencePanel
        primaryOverview={overview}
        dashboard={baseDashboard}
        onSelectDomain={() => {}}
        explorerT={explorerT}
      />,
    )
    expect(screen.getByText('tokio docs')).toBeInTheDocument()
    expect(screen.getByText('5 days · 4 sessions')).toBeInTheDocument()
  })

  test('falls back to the page URL when a refind item has no title', () => {
    const overview = makeOverview(
      [],
      [
        makeRefindPage({
          canonicalUrl: 'https://docs.rs/sqlx',
          url: 'https://docs.rs/sqlx',
          title: null,
        }),
      ],
    )
    render(
      <PaperIntelligencePanel
        primaryOverview={overview}
        dashboard={baseDashboard}
        onSelectDomain={() => {}}
        explorerT={explorerT}
      />,
    )

    expect(screen.getByText('https://docs.rs/sqlx')).toBeInTheDocument()
  })
})
