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

function dayFromEntries(date: string, entries: HistoryEntry[]): PaperDay {
  return {
    date,
    visitCount: entries.length,
    domains: new Set(entries.map((row) => row.domain)).size,
    sessions: [
      {
        id: 'session-1',
        startMs: entries[0]?.visitTime ? entries[0].visitTime * 1000 : 0,
        endMs: entries[entries.length - 1]?.visitTime
          ? (entries.at(-1)?.visitTime ?? 0) * 1000
          : 0,
        visitCount: entries.length,
        blocks: [
          {
            type: 'stack',
            domain: entries[0]?.domain ?? '',
            entries,
          },
        ],
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
              type: 'stack',
              domain: 'github.com',
              entries: [
                entry({
                  id: 11,
                  url: 'https://github.com/a',
                  domain: 'github.com',
                  visitedAt: '2026-05-21T08:30:00Z',
                }),
                entry({
                  id: 12,
                  url: 'https://github.com/b',
                  domain: 'github.com',
                  visitedAt: '2026-05-21T08:31:00Z',
                }),
              ],
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
})
