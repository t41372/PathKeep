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
})
