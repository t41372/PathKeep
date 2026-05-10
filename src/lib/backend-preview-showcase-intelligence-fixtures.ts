/**
 * @file backend-preview-showcase-intelligence-fixtures.ts
 * @description Core Intelligence read fixtures for the synthetic browser-preview showcase dataset.
 *
 * ## Responsibilities
 * - Build meaningful deterministic Intelligence payloads for public static preview deployments.
 * - Keep section metadata explicit that the data is synthetic and fixture-backed.
 *
 * ## Not responsible for
 * - Mutating preview state or dispatching backend commands.
 * - Supplying optional AI, semantic, embedding, MCP, or readable-content behavior.
 *
 * ## Dependencies
 * - Depends on `backend-preview-showcase-fixtures.ts` for public synthetic domains and date helpers.
 *
 * ## Performance notes
 * - Payloads are small, synchronous, and bounded so preview routes cannot grow with modeled archive size.
 */

import type {
  CoreIntelligencePrimaryOverview,
  CoreIntelligenceSecondaryOverview,
  CoreIntelligenceSectionResult,
  DateRange,
  DigestSummary,
  SearchQueryListResult,
  SearchQuerySort,
} from './core-intelligence'
import {
  SHOWCASE_ACTIVE_HOURS,
  SHOWCASE_PROFILE_IDS,
  SHOWCASE_SITES,
  isoAt,
  localDateKey,
  nowMs,
} from './backend-preview-showcase-fixtures'

function defaultDateRange(): DateRange {
  return {
    start: localDateKey(nowMs() - 30 * 86_400_000),
    end: localDateKey(nowMs()),
  }
}

function dateRangeFromArgs(args: Record<string, unknown> | undefined) {
  const request = args?.request as { dateRange?: DateRange } | undefined
  return request?.dateRange ?? defaultDateRange()
}

function readySection<T>(
  sectionId: string,
  dateRange: DateRange,
  data: T,
  moduleIds: string[],
  sourceTables: string[],
): CoreIntelligenceSectionResult<T> {
  return {
    data,
    meta: {
      sectionId,
      generatedAt: isoAt(nowMs() - 45 * 60_000),
      window: { kind: 'date-range', dateRange },
      moduleIds,
      sourceTables,
      includesEnrichment: false,
      state: 'ready',
      stateReason: null,
      notes: [
        'Synthetic browser-preview showcase data; no local archive rows are bundled.',
      ],
    },
  }
}

function digestSummary(dateRange: DateRange): DigestSummary {
  return {
    dateRange,
    totalVisits: {
      value: 6_071,
      previousValue: 5_840,
      changePercent: 4,
      trend: 'up',
    },
    totalSearches: {
      value: 126,
      previousValue: 118,
      changePercent: 7,
      trend: 'up',
    },
    newDomains: {
      value: 184,
      previousValue: 171,
      changePercent: 8,
      trend: 'up',
    },
    deepReadPages: {
      value: 48,
      previousValue: 52,
      changePercent: -8,
      trend: 'down',
    },
    refindPages: {
      value: 31,
      previousValue: 30,
      changePercent: 3,
      trend: 'flat',
    },
  }
}

function topSites() {
  return SHOWCASE_SITES.slice(0, 8).map((site, index) => ({
    registrableDomain: site.domain,
    displayName: site.displayName,
    domainCategory: site.category,
    visitCount: [742, 680, 534, 421, 386, 312, 294, 260][index] ?? 120,
    uniqueDays: [27, 24, 22, 19, 18, 16, 14, 12][index] ?? 8,
    averageDailyVisits:
      [24.7, 22.6, 17.8, 14, 12.8, 10.4, 9.8, 8.6][index] ?? 4,
    uniqueUrls: [320, 190, 88, 126, 96, 74, 62, 55][index] ?? 20,
  }))
}

function refindPages() {
  return [
    ['https://sqlite.org/wal.html', 'Write-ahead logging', 'sqlite.org'],
    ['https://tauri.app/start/', 'Tauri v2 getting started', 'tauri.app'],
    [
      'https://react.dev/reference/react/Suspense',
      'Suspense reference',
      'react.dev',
    ],
    [
      'https://vercel.com/docs/deployments/overview',
      'Deployment overview',
      'vercel.com',
    ],
  ].map(([url, title, domain], index) => ({
    canonicalUrl: url,
    url,
    title,
    registrableDomain: domain,
    crossDayCount: [18, 15, 11, 9][index],
    trailCount: [6, 5, 3, 3][index],
    searchArrivalCount: [4, 3, 2, 2][index],
    typedRevisitCount: [3, 2, 1, 1][index],
    refindScore: [0.92, 0.86, 0.74, 0.68][index],
    firstSeenAt: isoAt(nowMs() - (50 - index * 8) * 86_400_000),
    lastSeenAt: isoAt(nowMs() - (index + 1) * 86_400_000),
  }))
}

function queryFamilies() {
  const families = [
    [
      'family-local-first-archive',
      'local first browser history',
      [
        'local first browser history',
        'history sqlite archive',
        'tauri local database',
      ],
    ],
    [
      'family-tauri-storage',
      'tauri sqlite storage',
      [
        'tauri sqlite storage',
        'tauri command state',
        'tauri app data directory',
      ],
    ],
    [
      'family-preview-deploy',
      'vercel static preview data',
      [
        'vercel vite static build',
        'vite env static preview',
        'hash router vercel',
      ],
    ],
    [
      'family-rust-sqlite',
      'rusqlite wal checkpoint',
      [
        'rusqlite wal checkpoint',
        'sqlite query planner',
        'sqlite date functions',
      ],
    ],
  ].map(([familyId, anchorQuery, queries], index) => ({
    familyId: familyId as string,
    anchorQuery: anchorQuery as string,
    memberCount: [9, 7, 5, 4][index],
    searchEngine: index === 3 ? 'github' : 'google',
    queries: queries as string[],
    firstSeenAt: isoAt(nowMs() - (18 + index * 4) * 86_400_000),
    lastSeenAt: isoAt(nowMs() - (index + 1) * 86_400_000),
  }))
  return { families, total: families.length, page: 0, pageSize: 10 }
}

function activityMix() {
  return {
    categories: [
      { domainCategory: 'docs', visitCount: 2_180, share: 0.36 },
      { domainCategory: 'developer', visitCount: 1_760, share: 0.29 },
      { domainCategory: 'community', visitCount: 910, share: 0.15 },
      { domainCategory: 'ai', visitCount: 720, share: 0.12 },
      { domainCategory: 'video', visitCount: 501, share: 0.08 },
    ],
    changeVsPrevious: [
      {
        domainCategory: 'docs',
        currentShare: 0.36,
        previousShare: 0.32,
        changePoints: 4,
      },
      {
        domainCategory: 'developer',
        currentShare: 0.29,
        previousShare: 0.31,
        changePoints: -2,
      },
      {
        domainCategory: 'ai',
        currentShare: 0.12,
        previousShare: 0.1,
        changePoints: 2,
      },
    ],
  }
}

function discoveryTrend(granularity: string | undefined) {
  const count = granularity === 'day' ? 30 : 12
  const unitMs = granularity === 'day' ? 86_400_000 : 7 * 86_400_000
  return {
    points: Array.from({ length: count }, (_, index) => {
      const ms = nowMs() - (count - index - 1) * unitMs
      return {
        dateKey: localDateKey(ms),
        discoveryRate: 0.18 + (index % 5) * 0.025,
        newDomainCount: 8 + (index % 6),
        totalVisits: 120 + (index % 8) * 18,
      }
    }),
    availableYears: [2025, 2026],
  }
}

function searchRows(): SearchQueryListResult['rows'] {
  const queries = [
    [
      'local first browser history',
      'google',
      'family-local-first-archive',
      'trail-local-first',
    ],
    [
      'tauri sqlite storage',
      'google',
      'family-tauri-storage',
      'trail-tauri-storage',
    ],
    [
      'vercel vite static build',
      'google',
      'family-preview-deploy',
      'trail-preview-deploy',
    ],
    [
      'rusqlite wal checkpoint',
      'github',
      'family-rust-sqlite',
      'trail-rust-sqlite',
    ],
    [
      'browser history sqlite schema',
      'google',
      'family-local-first-archive',
      'trail-local-first',
    ],
    ['react suspense route preload', 'google', null, null],
  ] as const

  return queries.map(([query, engine, familyId, trailId], index) => ({
    visitId: 30_000 + index,
    profileId: SHOWCASE_PROFILE_IDS[index % SHOWCASE_PROFILE_IDS.length],
    browserKind: index % 3 === 2 ? 'comet' : 'chrome',
    searchEngine: engine,
    displayName: engine === 'github' ? 'GitHub' : 'Google',
    rawQuery: query,
    normalizedQuery: query,
    searchedAt: isoAt(nowMs() - (index + 1) * 7_200_000),
    searchedAtMs: nowMs() - (index + 1) * 7_200_000,
    exactRepeatCount: [4, 3, 2, 2, 1, 1][index],
    familyCount: [9, 7, 5, 4, 9, 1][index],
    familyId,
    trailId,
    trailInitialQuery: familyId ? query : null,
    trailReformulationCount: familyId ? [4, 3, 2, 2, 4, 0][index] : null,
  }))
}

function compareSearchRows(
  sort: SearchQuerySort | undefined,
  left: SearchQueryListResult['rows'][number],
  right: SearchQueryListResult['rows'][number],
) {
  if (sort === 'alphabetical') {
    return left.normalizedQuery.localeCompare(right.normalizedQuery)
  }
  if (sort === 'exact-frequency')
    return right.exactRepeatCount - left.exactRepeatCount
  if (sort === 'family-frequency') return right.familyCount - left.familyCount
  return right.searchedAtMs - left.searchedAtMs
}

export function buildShowcaseSearchQueries(request?: {
  profileId?: string | null
  browserKind?: string | null
  engine?: string | null
  query?: string | null
  sort?: SearchQuerySort
  page?: number
  pageSize?: number
}): SearchQueryListResult {
  const query = request?.query?.trim().toLowerCase()
  const filtered = searchRows()
    .filter((row) => !request?.profileId || row.profileId === request.profileId)
    .filter(
      (row) => !request?.browserKind || row.browserKind === request.browserKind,
    )
    .filter((row) => !request?.engine || row.searchEngine === request.engine)
    .filter((row) => !query || row.normalizedQuery.includes(query))
    .sort((left, right) => compareSearchRows(request?.sort, left, right))
  const page = request?.page ?? 0
  const pageSize = request?.pageSize ?? 20
  const start = page * pageSize
  return {
    rows: filtered.slice(start, start + pageSize),
    total: filtered.length,
    page,
    pageSize,
  }
}

export function buildShowcasePrimaryOverview(
  args: Record<string, unknown> | undefined,
): CoreIntelligencePrimaryOverview {
  const dateRange = dateRangeFromArgs(args)
  return {
    digestSummary: readySection(
      'digest-summary',
      dateRange,
      digestSummary(dateRange),
      ['daily-rollups'],
      ['daily_summary_rollups'],
    ),
    onThisDay: readySection(
      'on-this-day',
      dateRange,
      [
        {
          year: new Date().getFullYear() - 1,
          date: `${new Date().getFullYear() - 1}-${localDateKey(nowMs()).slice(5)}`,
          totalVisits: 86,
          topDomains: ['sqlite.org', 'tauri.app', 'github.com'],
          summary: 'Reviewed local storage and desktop packaging references.',
          deepDiveSessions: 3,
        },
      ],
      ['daily-rollups'],
      ['visits', 'urls'],
    ),
    topSites: readySection(
      'top-sites',
      dateRange,
      topSites(),
      ['daily-rollups'],
      ['domain_daily_rollups'],
    ),
    refindPages: readySection(
      'refind-pages',
      dateRange,
      refindPages(),
      ['refind-pages'],
      ['refind_pages'],
    ),
    searchEngineRanking: readySection(
      'search-activity',
      dateRange,
      [
        { searchEngine: 'google', displayName: 'Google', searchCount: 92 },
        { searchEngine: 'github', displayName: 'GitHub', searchCount: 21 },
        {
          searchEngine: 'perplexity',
          displayName: 'Perplexity',
          searchCount: 13,
        },
      ],
      ['search-trails'],
      ['search_events'],
    ),
    topSearchConcepts: readySection(
      'search-activity',
      dateRange,
      [
        { term: 'sqlite', frequency: 34, engines: ['google', 'github'] },
        { term: 'tauri', frequency: 28, engines: ['google'] },
        {
          term: 'local-first',
          frequency: 21,
          engines: ['google', 'perplexity'],
        },
        { term: 'vercel', frequency: 14, engines: ['google'] },
      ],
      ['search-trails'],
      ['search_event_terms'],
    ),
    queryFamilies: readySection(
      'search-activity',
      dateRange,
      queryFamilies(),
      ['search-trails'],
      ['query_families'],
    ),
    activityMix: readySection(
      'activity-mix',
      dateRange,
      activityMix(),
      ['activity-mix'],
      ['category_daily_rollups'],
    ),
    discoveryTrendDay: readySection(
      'browsing-rhythm',
      dateRange,
      discoveryTrend('day'),
      ['daily-rollups'],
      ['domain_daily_rollups'],
    ),
    habitPatterns: readySection(
      'habits',
      dateRange,
      [
        {
          registrableDomain: 'github.com',
          displayName: 'GitHub',
          habitType: 'daily_habit',
          meanIntervalDays: 1.1,
          cv: 0.18,
          visitCount: 742,
          lastVisitedAt: isoAt(nowMs() - 3_600_000),
          isInterrupted: false,
        },
      ],
      ['domain-deep-dive'],
      ['habit_patterns'],
    ),
    interruptedHabits: readySection(
      'habits',
      dateRange,
      [],
      ['domain-deep-dive'],
      ['habit_patterns'],
    ),
    timings: [
      { sectionId: 'digest-summary', durationMs: 8 },
      { sectionId: 'top-sites', durationMs: 11 },
      { sectionId: 'search-activity', durationMs: 14 },
    ],
    totalDurationMs: 47,
  }
}

export function buildShowcaseSecondaryOverview(
  args: Record<string, unknown> | undefined,
): CoreIntelligenceSecondaryOverview {
  const dateRange = dateRangeFromArgs(args)
  return {
    stableSources: readySection(
      'stable-sources',
      dateRange,
      [
        {
          registrableDomain: 'sqlite.org',
          displayName: 'SQLite',
          sourceRole: 'reference',
          trailCount: 12,
          stableLandingCount: 8,
          effectivenessScore: 0.86,
        },
        {
          registrableDomain: 'tauri.app',
          displayName: 'Tauri',
          sourceRole: 'landing',
          trailCount: 9,
          stableLandingCount: 6,
          effectivenessScore: 0.78,
        },
      ],
      ['search-effectiveness'],
      ['source_effectiveness'],
    ),
    searchEffectiveness: readySection(
      'search-effectiveness',
      dateRange,
      {
        engineStats: [
          {
            searchEngine: 'google',
            displayName: 'Google',
            avgReformulations: 2.1,
            totalTrails: 38,
            avgDepth: 4.6,
          },
          {
            searchEngine: 'github',
            displayName: 'GitHub',
            avgReformulations: 1.4,
            totalTrails: 11,
            avgDepth: 3.1,
          },
        ],
        topResolvingSources: [
          {
            registrableDomain: 'sqlite.org',
            displayName: 'SQLite',
            sourceRole: 'reference',
            trailCount: 12,
            stableLandingCount: 8,
            effectivenessScore: 0.86,
          },
        ],
        hardestTopics: [
          {
            familyId: 'family-preview-deploy',
            queryFamily: 'vercel static preview data',
            reformulationCount: 5,
            reSearchLagDays: 3,
          },
        ],
      },
      ['search-effectiveness'],
      ['search_trails', 'source_effectiveness'],
    ),
    frictionSignals: readySection(
      'friction-signals',
      dateRange,
      [],
      ['search-effectiveness'],
      ['reopened_investigations'],
    ),
    reopenedInvestigations: readySection(
      'reopened-investigations',
      dateRange,
      [
        {
          investigationId: 'reopen-local-first',
          anchorType: 'query_family',
          anchorId: 'family-local-first-archive',
          anchorLabel: 'local first browser history',
          occurrenceCount: 4,
          distinctDays: 9,
          firstSeenAt: isoAt(nowMs() - 28 * 86_400_000),
          lastSeenAt: isoAt(nowMs() - 2 * 86_400_000),
        },
      ],
      ['search-effectiveness'],
      ['reopened_investigations'],
    ),
    discoveryTrendWeek: readySection(
      'discovery-trend',
      dateRange,
      discoveryTrend('week'),
      ['daily-rollups'],
      ['domain_daily_rollups'],
    ),
    breadthIndex: readySection(
      'breadth-index',
      dateRange,
      { hhi: 0.18, breadthScore: 82, concentrationDomainCount: 9 },
      ['daily-rollups'],
      ['domain_daily_rollups'],
    ),
    pathFlows: readySection(
      'path-flows',
      dateRange,
      [
        {
          flowId: 'flow-search-docs-github',
          flowPattern: 'Search -> Docs -> GitHub',
          stepCount: 3,
          occurrenceCount: 16,
          lastSeenAt: isoAt(nowMs() - 86_400_000),
          steps: [
            { index: 0, label: 'Search', registrableDomain: 'google.com' },
            { index: 1, label: 'Docs', registrableDomain: 'tauri.app' },
            { index: 2, label: 'Source', registrableDomain: 'github.com' },
          ],
        },
      ],
      ['domain-deep-dive'],
      ['path_flows'],
    ),
    compareSets: readySection(
      'compare-sets',
      dateRange,
      [
        {
          compareSetId: 'compare-preview-hosting',
          trailId: 'trail-preview-deploy',
          searchQuery: 'vercel vite static build',
          pageCategory: 'deployment',
          pages: [
            {
              canonicalUrl: 'https://vercel.com/docs/deployments/overview',
              url: 'https://vercel.com/docs/deployments/overview',
              title: 'Deployment overview',
              registrableDomain: 'vercel.com',
              visitCount: 7,
              isLanding: true,
            },
            {
              canonicalUrl: 'https://vite.dev/guide/static-deploy.html',
              url: 'https://vite.dev/guide/static-deploy.html',
              title: 'Static deploy',
              registrableDomain: 'vite.dev',
              visitCount: 4,
              isLanding: false,
            },
          ],
        },
      ],
      ['search-trails'],
      ['compare_sets'],
    ),
    multiBrowserDiff: readySection(
      'multi-browser-diff',
      dateRange,
      {
        profiles: [
          {
            profileId: 'chrome:Default',
            profileName: 'Primary',
            browserFamily: 'chromium',
            domainCount: 6_800,
            visitCount: 221_000,
          },
          {
            profileId: 'comet:Default',
            profileName: 'Research',
            browserFamily: 'chromium',
            domainCount: 2_400,
            visitCount: 74_000,
          },
          {
            profileId: 'safari:default',
            profileName: 'Safari',
            browserFamily: 'safari',
            domainCount: 1_200,
            visitCount: 35_000,
          },
        ],
        exclusiveDomains: [
          {
            registrableDomain: 'perplexity.ai',
            profileId: 'comet:Default',
            visitCount: 720,
          },
          {
            registrableDomain: 'developer.apple.com',
            profileId: 'safari:default',
            visitCount: 180,
          },
        ],
        sharedDomains: ['github.com', 'sqlite.org', 'tauri.app'],
        categoryDistributions: [],
      },
      ['daily-rollups'],
      ['domain_daily_rollups'],
    ),
    observedInteractions: readySection(
      'observed-interactions',
      dateRange,
      [],
      ['visit-derived-facts'],
      ['visit_derived_facts'],
    ),
    timings: [
      { sectionId: 'stable-sources', durationMs: 13 },
      { sectionId: 'search-effectiveness', durationMs: 17 },
      { sectionId: 'multi-browser-diff', durationMs: 12 },
    ],
    totalDurationMs: 63,
  }
}

export function buildShowcaseBrowsingRhythm() {
  const cells = Array.from({ length: 7 * 24 }, (_, index) => {
    const dow = Math.floor(index / 24)
    const hour = index % 24
    const active = SHOWCASE_ACTIVE_HOURS.includes(hour)
    return {
      dow,
      hour,
      visitCount: active ? 18 + ((dow + hour) % 9) * 4 : (dow + hour) % 5,
    }
  })
  return { cells, maxCount: Math.max(...cells.map((cell) => cell.visitCount)) }
}

export function buildShowcaseDayInsights(
  args: Record<string, unknown> | undefined,
) {
  const request = args?.request as
    | { date?: string; profileId?: string | null }
    | undefined
  const date = request?.date ?? localDateKey(nowMs())
  const dateRange = { start: date, end: date }
  return readySection(
    'day-insights',
    dateRange,
    {
      date,
      digestSummary: digestSummary(dateRange),
      topSites: topSites().slice(0, 5),
      activityMix: activityMix(),
      refindPages: refindPages().slice(0, 3),
      queryFamilies: queryFamilies(),
      hourlyActivity: Array.from({ length: 24 }, (_, hour) => ({
        hour,
        visitCount: SHOWCASE_ACTIVE_HOURS.includes(hour)
          ? 10 + (hour % 5) * 3
          : hour % 3,
      })),
      drilldown: { explorerDateRange: dateRange },
    },
    ['daily-rollups', 'search-trails'],
    ['visits', 'daily_summary_rollups'],
  )
}

export function buildShowcaseDomainTrend(
  args: Record<string, unknown> | undefined,
) {
  const request = args?.request as
    | { registrableDomain?: string; dateRange?: DateRange }
    | undefined
  const dateRange = request?.dateRange ?? defaultDateRange()
  return {
    registrableDomain: request?.registrableDomain ?? 'github.com',
    points: discoveryTrend('week').points.map((point) => ({
      dateKey: point.dateKey,
      visitCount: Math.max(8, Math.round(point.totalVisits * 0.18)),
    })),
    dateRange,
  }
}

export function buildShowcaseDomainDeepDive(
  args: Record<string, unknown> | undefined,
) {
  const request = args?.request as
    | { registrableDomain?: string; dateRange?: DateRange }
    | undefined
  const dateRange = request?.dateRange ?? defaultDateRange()
  const domain = request?.registrableDomain ?? 'github.com'
  const site =
    SHOWCASE_SITES.find((item) => item.domain === domain) ?? SHOWCASE_SITES[0]
  return readySection(
    'domain-deep-dive',
    dateRange,
    {
      registrableDomain: site.domain,
      displayName: site.displayName,
      domainCategory: site.category,
      totalVisits: 742,
      activeDays: 27,
      trailCount: 12,
      arrivalBreakdown: { search: 42, link: 31, typed: 18, other: 9 },
      topPages: site.paths.slice(0, 3).map((path, index) => ({
        path,
        visitCount: [210, 144, 88][index] ?? 42,
      })),
      topReferrers: [
        { domain: 'google.com', displayName: 'Google', count: 38 },
        { domain: 'tauri.app', displayName: 'Tauri', count: 14 },
      ],
      topExits: [
        { domain: 'sqlite.org', displayName: 'SQLite', count: 18 },
        { domain: 'react.dev', displayName: 'React', count: 12 },
      ],
      visitTrend: buildShowcaseDomainTrend(args).points,
    },
    ['domain-deep-dive', 'daily-rollups'],
    ['domain_daily_rollups', 'path_flows'],
  )
}
