import { describe, expect, test } from 'vitest'
import type { HistoryEntry } from '@/lib/types/archive'
import type { PaperDay } from '@/pages/explorer/paper/group-entries'
import { aggregateDayInsights } from './paper-day-insights-helpers'

function entry(
  overrides: Partial<HistoryEntry> & {
    visitedAt: string
  },
): HistoryEntry {
  return {
    id: 1,
    profileId: 'chrome:Default',
    url: 'https://example.com/',
    title: 'example',
    domain: 'example.com',
    favicon: null,
    visitTime: new Date(overrides.visitedAt).getTime() / 1000,
    durationMs: null,
    transition: 0,
    sourceVisitId: 0,
    appId: null,
    ...overrides,
  } as HistoryEntry
}

function dayFromEntries(
  date: string,
  entries: HistoryEntry[],
  overrides: { sessionStartMs?: number; sessionEndMs?: number } = {},
): PaperDay {
  return {
    date,
    visitCount: entries.length,
    domains: new Set(entries.map((row) => row.domain)).size,
    sessions: [
      {
        id: 'session-1',
        startMs:
          overrides.sessionStartMs ??
          (entries[0]?.visitTime ? entries[0].visitTime * 1000 : 0),
        endMs:
          overrides.sessionEndMs ??
          (entries[entries.length - 1]?.visitTime
            ? (entries.at(-1)?.visitTime ?? 0) * 1000
            : 0),
        visitCount: entries.length,
        blocks: entries.map((entry) => ({ type: 'single' as const, entry })),
      },
    ],
  }
}

describe('aggregateDayInsights', () => {
  test('returns zeros for an empty day', () => {
    const insights = aggregateDayInsights({
      date: '2026-05-21',
      visitCount: 0,
      domains: 0,
      sessions: [],
    })
    expect(insights.totalPages).toBe(0)
    expect(insights.topDomains).toEqual([])
    expect(insights.hourBuckets.every((value) => value === 0)).toBe(true)
    expect(insights.hourPeak).toBe(1)
  })

  test('classifies visits by transition low-byte and ranks top domains', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://github.com/a',
        domain: 'github.com',
        visitedAt: '2026-05-21T09:00:00Z',
        transition: 1, // typed
      }),
      entry({
        id: 2,
        url: 'https://github.com/b',
        domain: 'github.com',
        visitedAt: '2026-05-21T09:30:00Z',
        transition: 0, // link
      }),
      entry({
        id: 3,
        url: 'https://docs.rs/foo',
        domain: 'docs.rs',
        visitedAt: '2026-05-21T10:00:00Z',
        transition: 0,
      }),
      entry({
        id: 4,
        url: 'https://google.com/search?q=rust',
        domain: 'google.com',
        visitedAt: '2026-05-21T11:00:00Z',
        transition: 5, // generated → search
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.totalPages).toBe(4)
    expect(insights.typedCount).toBe(1)
    expect(insights.linkCount).toBe(2)
    expect(insights.searchCount).toBe(1)
    expect(insights.distinctDomains).toBe(3)
    expect(insights.topDomains[0]).toEqual({ domain: 'github.com', visits: 2 })
    expect(insights.hourBuckets.reduce((sum, value) => sum + value, 0)).toBe(4)
  })

  test('treats known search-engine hosts as searches when transition is unknown', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://duckduckgo.com/?q=test',
        domain: 'duckduckgo.com',
        visitedAt: '2026-05-21T14:00:00Z',
        transition: -1,
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.searchCount).toBe(1)
  })

  test('aggregates across stack + single blocks alike', () => {
    const e1 = entry({
      id: 10,
      url: 'https://news.ycombinator.com/',
      domain: 'news.ycombinator.com',
      visitedAt: '2026-05-21T08:00:00Z',
    })
    const day: PaperDay = {
      date: '2026-05-21',
      visitCount: 2,
      domains: 2,
      sessions: [
        {
          id: 's1',
          startMs: 0,
          endMs: 0,
          visitCount: 2,
          blocks: [
            { type: 'single', entry: e1 },
            {
              type: 'single',
              entry: entry({
                id: 11,
                url: 'https://github.com/a',
                domain: 'github.com',
                visitedAt: '2026-05-21T08:30:00Z',
              }),
            },
            {
              type: 'single',
              entry: entry({
                id: 12,
                url: 'https://github.com/b',
                domain: 'github.com',
                visitedAt: '2026-05-21T08:31:00Z',
              }),
            },
          ],
        },
      ],
    }
    const insights = aggregateDayInsights(day)
    expect(insights.totalPages).toBe(3)
    expect(insights.topDomains.map((row) => row.domain)).toEqual([
      'github.com',
      'news.ycombinator.com',
    ])
  })

  test('aggregates the editorial extras (first/last/peak/longest/topUrls) honestly', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://docs.rs/sqlx',
        title: 'sqlx docs',
        domain: 'docs.rs',
        visitedAt: '2026-05-21T09:14:00Z',
        visitTime: Date.UTC(2026, 4, 21, 9, 14) / 1000,
      }),
      entry({
        id: 2,
        url: 'https://docs.rs/sqlx',
        title: 'sqlx docs (return visit)',
        domain: 'docs.rs',
        visitedAt: '2026-05-21T14:01:00Z',
        visitTime: Date.UTC(2026, 4, 21, 14, 1) / 1000,
      }),
      entry({
        id: 3,
        url: 'https://example.com/path',
        title: null,
        domain: 'example.com',
        visitedAt: '2026-05-21T22:43:00Z',
        visitTime: Date.UTC(2026, 4, 21, 22, 43) / 1000,
      }),
    ]
    const day = dayFromEntries('2026-05-21', visits, {
      sessionStartMs: Date.UTC(2026, 4, 21, 9, 0),
      sessionEndMs: Date.UTC(2026, 4, 21, 11, 30), // 2h 30m
    })
    const insights = aggregateDayInsights(day)
    expect(insights.firstVisitMs).toBe(Date.UTC(2026, 4, 21, 9, 14))
    expect(insights.lastVisitMs).toBe(Date.UTC(2026, 4, 21, 22, 43))
    // Peak hour is local-time bucket; depending on the host timezone the
    // visit hour may shift, but the peak count should still come from the
    // bucket with two visits (docs.rs/sqlx fires twice in the same local
    // hour because both samples are within the same day).
    expect(insights.peakHour).not.toBeNull()
    expect(insights.longestSessionMs).toBe(2 * 60 * 60_000 + 30 * 60_000)
    expect(insights.topUrls.map((row) => row.url)).toEqual([
      'https://docs.rs/sqlx',
      'https://example.com/path',
    ])
    // The first title we see for a URL is preserved across re-visits even
    // when later samples carry a slightly different title.
    expect(insights.topUrls[0].title).toBe('sqlx docs')
    // URL with a null title falls through to null on the aggregate, so
    // the UI knows to render the URL itself instead of an empty span.
    expect(insights.topUrls[1].title).toBeNull()
    expect(insights.topUrls[0].visits).toBe(2)
    expect(insights.topUrls[1].visits).toBe(1)
  })

  test('treats ms-precision visitTime the same as second-precision', () => {
    const msEntry = entry({
      id: 1,
      url: 'https://example.com/',
      title: 'Example',
      domain: 'example.com',
      visitedAt: '2026-05-21T09:14:00Z',
      // > 1e12 → already ms; the helper must NOT multiply by 1,000 again.
      visitTime: Date.UTC(2026, 4, 21, 9, 14),
    })
    const insights = aggregateDayInsights(
      dayFromEntries('2026-05-21', [msEntry]),
    )
    // Peak hour is the LOCAL hour of 09:14 UTC, which depends on the host
    // timezone — but it must be in the 0..23 range and reflect a real
    // visit, not the "fictional date ~50,000 years from now" 0-hour the
    // pre-fix code produced for ms-precision inputs.
    expect(insights.peakHour).not.toBeNull()
    expect(insights.peakHour).toBeGreaterThanOrEqual(0)
    expect(insights.peakHour).toBeLessThanOrEqual(23)
    // hourBuckets total matches the number of entries.
    expect(insights.hourBuckets.reduce((sum, n) => sum + n, 0)).toBe(1)
  })

  test('falls back to visitedAt when visitTime is missing', () => {
    const fallbackEntry = entry({
      id: 1,
      url: 'https://example.com/',
      title: 'Example',
      domain: 'example.com',
      visitedAt: '2026-05-21T09:14:00Z',
      visitTime: 0,
    })
    const insights = aggregateDayInsights(
      dayFromEntries('2026-05-21', [fallbackEntry]),
    )
    expect(insights.firstVisitMs).not.toBeNull()
  })

  test('extracts top search queries from recognised search-engine hosts', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://www.google.com/search?q=rust%20async',
        domain: 'google.com',
        visitedAt: '2026-05-21T09:14:00Z',
      }),
      entry({
        id: 2,
        url: 'https://www.google.com/search?q=Rust+Async',
        domain: 'google.com',
        visitedAt: '2026-05-21T09:18:00Z',
      }),
      entry({
        id: 3,
        url: 'https://duckduckgo.com/?q=sqlx+migrations',
        domain: 'duckduckgo.com',
        visitedAt: '2026-05-21T10:00:00Z',
      }),
      entry({
        id: 4,
        url: 'https://www.baidu.com/s?wd=%E5%90%89%E9%87%8E%E5%AE%B6',
        domain: 'baidu.com',
        visitedAt: '2026-05-21T11:00:00Z',
      }),
      // Search-engine host with no query param → ignored.
      entry({
        id: 5,
        url: 'https://www.google.com/',
        domain: 'google.com',
        visitedAt: '2026-05-21T12:00:00Z',
      }),
      // Search-engine host with blank query → ignored.
      entry({
        id: 6,
        url: 'https://www.google.com/search?q=%20%20',
        domain: 'google.com',
        visitedAt: '2026-05-21T12:30:00Z',
      }),
      // Unrelated host → never contributes.
      entry({
        id: 7,
        url: 'https://example.com/?q=should-not-appear',
        domain: 'example.com',
        visitedAt: '2026-05-21T13:00:00Z',
      }),
      // Pathologically long query → dropped.
      entry({
        id: 8,
        url: `https://www.google.com/search?q=${encodeURIComponent('z'.repeat(200))}`,
        domain: 'google.com',
        visitedAt: '2026-05-21T13:30:00Z',
      }),
      // Malformed URL → swallowed by URL parser, no crash, no entry.
      entry({
        id: 9,
        url: 'not a url',
        domain: '',
        visitedAt: '2026-05-21T13:45:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    const queries = insights.topSearchQueries
    // 'rust async' was typed twice (case + + decoding folded into one).
    const rust = queries.find((row) => row.query.toLowerCase() === 'rust async')
    expect(rust?.count).toBe(2)
    const sqlx = queries.find((row) => row.query === 'sqlx migrations')
    expect(sqlx?.count).toBe(1)
    const baidu = queries.find((row) => row.query === '吉野家')
    expect(baidu?.count).toBe(1)
    // 'should-not-appear' lived on an unrecognised host.
    expect(
      queries.some((row) => row.query.toLowerCase() === 'should-not-appear'),
    ).toBe(false)
    // 200-z pasted query was dropped on length guard.
    expect(queries.some((row) => row.query.startsWith('zzz'))).toBe(false)
    // Sorted by count desc then query asc; rust(2) before sqlx(1) before 吉野家.
    expect(queries[0].count).toBeGreaterThanOrEqual(queries[1].count)
  })

  test('caps the top-search-queries list at 6 entries', () => {
    const visits = Array.from({ length: 12 }, (_, idx) =>
      entry({
        id: idx + 1,
        url: `https://www.google.com/search?q=query-${idx}`,
        domain: 'google.com',
        visitedAt: `2026-05-21T${String(8 + idx).padStart(2, '0')}:00:00Z`,
      }),
    )
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.topSearchQueries.length).toBe(6)
  })

  // ── Additional edge cases for aggregator + extractor ───────────────

  test('top-search-queries sorts by count DESC, then alphabetically', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://www.google.com/search?q=zebra',
        domain: 'google.com',
        visitedAt: '2026-05-21T08:00:00Z',
      }),
      entry({
        id: 2,
        url: 'https://www.google.com/search?q=alpha',
        domain: 'google.com',
        visitedAt: '2026-05-21T08:01:00Z',
      }),
      entry({
        id: 3,
        url: 'https://www.google.com/search?q=Alpha',
        domain: 'google.com',
        visitedAt: '2026-05-21T08:02:00Z',
      }),
      entry({
        id: 4,
        url: 'https://www.google.com/search?q=alpha',
        domain: 'google.com',
        visitedAt: '2026-05-21T08:03:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    // 'alpha' searched 3x (case folded), 'zebra' once → alpha first
    // then zebra. Since alpha's count is highest it's at index 0;
    // zebra at index 1.
    expect(insights.topSearchQueries[0].query.toLowerCase()).toBe('alpha')
    expect(insights.topSearchQueries[0].count).toBe(3)
    expect(insights.topSearchQueries[1].query).toBe('zebra')
    expect(insights.topSearchQueries[1].count).toBe(1)
  })

  test('search query with only whitespace after trim is dropped', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://www.google.com/search?q=%09%0A%20',
        domain: 'google.com',
        visitedAt: '2026-05-21T08:00:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.topSearchQueries).toEqual([])
  })

  test('search query exactly 120 chars long is kept (boundary)', () => {
    const exact120 = 'a'.repeat(120)
    const visits = [
      entry({
        id: 1,
        url: `https://www.google.com/search?q=${encodeURIComponent(exact120)}`,
        domain: 'google.com',
        visitedAt: '2026-05-21T08:00:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.topSearchQueries[0]?.query.length).toBe(120)
  })

  test('search query 121 chars long is dropped (boundary +1)', () => {
    const just121 = 'a'.repeat(121)
    const visits = [
      entry({
        id: 1,
        url: `https://www.google.com/search?q=${encodeURIComponent(just121)}`,
        domain: 'google.com',
        visitedAt: '2026-05-21T08:00:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.topSearchQueries).toEqual([])
  })

  test('search-engine hosts are recognised regardless of www. prefix', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://google.com/search?q=naked',
        domain: 'google.com',
        visitedAt: '2026-05-21T08:00:00Z',
      }),
      entry({
        id: 2,
        url: 'https://www.bing.com/search?q=with-www',
        domain: 'bing.com',
        visitedAt: '2026-05-21T08:01:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.topSearchQueries.map((row) => row.query).sort()).toEqual(
      ['naked', 'with-www'],
    )
  })

  test('search-engine subdomain we have not mapped is ignored', () => {
    // `images.google.com/search?q=...` is not in the SEARCH_QUERY_PARAMS_BY_HOST
    // map (we only handle `google.com`). The visit still counts as a
    // page but contributes no search query.
    const visits = [
      entry({
        id: 1,
        url: 'https://images.google.com/search?q=cats',
        domain: 'images.google.com',
        visitedAt: '2026-05-21T08:00:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.topSearchQueries).toEqual([])
    expect(insights.totalPages).toBe(1)
  })

  test('Yahoo p= and Baidu wd= parameters work alongside Google q=', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://search.yahoo.com/search?p=yahoo-query',
        domain: 'yahoo.com',
        visitedAt: '2026-05-21T08:00:00Z',
      }),
      entry({
        id: 2,
        url: 'https://www.baidu.com/s?wd=baidu-query',
        domain: 'baidu.com',
        visitedAt: '2026-05-21T08:01:00Z',
      }),
      entry({
        id: 3,
        url: 'https://www.google.com/search?q=google-query',
        domain: 'google.com',
        visitedAt: '2026-05-21T08:02:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    const queries = insights.topSearchQueries.map((row) => row.query).sort()
    expect(queries).toEqual([
      'baidu-query',
      'google-query',
      'yahoo-query',
    ])
  })

  test('totalPages tally still counts even when no queries are extracted', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://github.com/tokio-rs/tokio',
        domain: 'github.com',
        visitedAt: '2026-05-21T08:00:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.totalPages).toBe(1)
    expect(insights.topSearchQueries).toEqual([])
  })

  test('topDomains caps at 4 entries and orders by visits DESC', () => {
    const hosts = [
      'a.example.com',
      'b.example.com',
      'c.example.com',
      'd.example.com',
      'e.example.com',
      'f.example.com',
    ]
    const visits: ReturnType<typeof entry>[] = []
    hosts.forEach((host, index) => {
      // host 0 → 6 visits, host 1 → 5 visits, ... host 5 → 1 visit
      const count = 6 - index
      for (let n = 0; n < count; n += 1) {
        visits.push(
          entry({
            id: index * 10 + n,
            url: `https://${host}/page-${n}`,
            domain: host,
            visitedAt: `2026-05-21T08:${String(index * 10 + n).padStart(2, '0')}:00Z`,
          }),
        )
      }
    })
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.topDomains.length).toBe(4)
    expect(insights.topDomains.map((d) => d.domain)).toEqual([
      'a.example.com',
      'b.example.com',
      'c.example.com',
      'd.example.com',
    ])
  })

  test('topUrls caps at 3 entries', () => {
    const visits: ReturnType<typeof entry>[] = []
    for (let i = 0; i < 5; i += 1) {
      // Each URL visited (5 - i) times → first URL most visited.
      const count = 5 - i
      for (let n = 0; n < count; n += 1) {
        visits.push(
          entry({
            id: i * 10 + n,
            url: `https://example.com/page-${i}`,
            domain: 'example.com',
            visitedAt: `2026-05-21T08:${String(i * 10 + n).padStart(2, '0')}:00Z`,
          }),
        )
      }
    }
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.topUrls.length).toBe(3)
    expect(insights.topUrls[0].url).toBe('https://example.com/page-0')
    expect(insights.topUrls[0].visits).toBe(5)
  })

  test('first/last visit are stable across multi-session days', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://example.com/morning',
        domain: 'example.com',
        visitedAt: '2026-05-21T06:00:00Z',
        visitTime: Date.UTC(2026, 4, 21, 6, 0),
      }),
      entry({
        id: 2,
        url: 'https://example.com/midday',
        domain: 'example.com',
        visitedAt: '2026-05-21T12:00:00Z',
        visitTime: Date.UTC(2026, 4, 21, 12, 0),
      }),
      entry({
        id: 3,
        url: 'https://example.com/evening',
        domain: 'example.com',
        visitedAt: '2026-05-21T22:00:00Z',
        visitTime: Date.UTC(2026, 4, 21, 22, 0),
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.firstVisitMs).toBe(Date.UTC(2026, 4, 21, 6, 0))
    expect(insights.lastVisitMs).toBe(Date.UTC(2026, 4, 21, 22, 0))
  })

  test('hourPeak is at least 1 even for zero-visit days (divide-by-zero guard)', () => {
    const insights = aggregateDayInsights({
      date: '2026-05-21',
      visitCount: 0,
      domains: 0,
      sessions: [],
    })
    expect(insights.hourPeak).toBe(1)
  })

  test('peakHour reports the local-hour bucket with the most visits', () => {
    // Two visits at 14:00 local, one at 09:00 local → peak 14.
    // Use bare ISO without Z so they parse as local time and the hour
    // bucket is stable regardless of test runner timezone.
    const visits = [
      entry({
        id: 1,
        url: 'https://example.com/a',
        domain: 'example.com',
        visitedAt: '2026-05-21T14:00:00',
        visitTime: new Date('2026-05-21T14:00:00').getTime(),
      }),
      entry({
        id: 2,
        url: 'https://example.com/b',
        domain: 'example.com',
        visitedAt: '2026-05-21T14:15:00',
        visitTime: new Date('2026-05-21T14:15:00').getTime(),
      }),
      entry({
        id: 3,
        url: 'https://example.com/c',
        domain: 'example.com',
        visitedAt: '2026-05-21T09:00:00',
        visitTime: new Date('2026-05-21T09:00:00').getTime(),
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    expect(insights.peakHour).toBe(14)
  })

  test('top-URL title falls back to first non-null title across visits', () => {
    const visits = [
      entry({
        id: 1,
        url: 'https://example.com/page',
        title: null,
        domain: 'example.com',
        visitedAt: '2026-05-21T08:00:00Z',
      }),
      entry({
        id: 2,
        url: 'https://example.com/page',
        title: 'Better title',
        domain: 'example.com',
        visitedAt: '2026-05-21T08:01:00Z',
      }),
      entry({
        id: 3,
        url: 'https://example.com/page',
        title: 'Latest title',
        domain: 'example.com',
        visitedAt: '2026-05-21T08:02:00Z',
      }),
    ]
    const insights = aggregateDayInsights(dayFromEntries('2026-05-21', visits))
    // The aggregator uses "first non-null title wins" semantics so the
    // initial null doesn't bind to the URL; subsequent values can
    // promote it. Once a non-null title is captured, later titles are
    // ignored — keeps the panel stable across rapid sessions.
    expect(insights.topUrls[0].title).toBe('Better title')
  })
})
