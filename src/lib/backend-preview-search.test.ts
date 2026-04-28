/**
 * @file backend-preview-search.test.ts
 * @description Unit coverage for the browser-preview search/history helper surface.
 * @module lib/backend-preview-search
 *
 * ## Responsibilities
 * - Verify deterministic preview search filters, sorting, pagination, cursors, and favicon hydration.
 * - Protect browser-preview semantics without going through the full backend facade.
 *
 * ## Not responsible for
 * - Re-testing Tauri IPC passthrough.
 * - Re-testing route-level Explorer rendering.
 *
 * ## Dependencies
 * - Uses the canonical preview state factory so helper tests stay aligned with fixture shape.
 *
 * ## Performance notes
 * - All tests are synchronous and operate on bounded in-memory fixtures.
 */

import { describe, expect, test, vi } from 'vitest'
import { createMockState } from './backend-preview-state'
import {
  browserKindFromProfileId,
  buildMockSearchQueries,
  compareMockSearchQueryRows,
  coreIntelligenceProfileId,
  coreIsoTimestamp,
  encodeMockHistoryCursor,
  filterMockHistory,
  loadMockHistoryFavicons,
  paginateMockAiSearch,
  parseMockHistoryCursor,
  uniqueUrlCount,
} from './backend-preview-search'

describe('backend preview search helpers', () => {
  test('normalizes profile ids, counts unique URLs, and anchors preview timestamps', () => {
    vi.setSystemTime(new Date('2026-04-25T12:00:00Z'))
    const state = createMockState()

    expect(browserKindFromProfileId('chrome:Default')).toBe('chrome')
    expect(browserKindFromProfileId('safari')).toBe('safari')
    expect(coreIntelligenceProfileId('  ')).toBe('chrome:Default')
    expect(coreIntelligenceProfileId('safari:Personal')).toBe('safari:Personal')
    expect(coreIsoTimestamp(1000)).toBe('2026-04-25T12:00:01.000Z')
    expect(uniqueUrlCount(state.history.items)).toBeGreaterThan(1)

    vi.useRealTimers()
  })

  test('filters and sorts synthetic search query rows', () => {
    vi.setSystemTime(new Date('2026-04-25T12:00:00Z'))

    expect(
      buildMockSearchQueries({
        browserKind: 'firefox',
      }),
    ).toMatchObject({ rows: [], total: 0 })
    expect(
      buildMockSearchQueries({
        engine: 'bilibili',
      }).rows.map((row) => row.searchEngine),
    ).toEqual(['bilibili'])
    expect(
      buildMockSearchQueries({
        query: 'TAURI',
      }).rows.map((row) => row.normalizedQuery),
    ).toEqual(['tauri sqlite local-first'])
    expect(
      buildMockSearchQueries({
        query: 'missing',
      }),
    ).toMatchObject({ rows: [], total: 0 })
    expect(
      buildMockSearchQueries({
        sort: 'alphabetical',
      }).rows.map((row) => row.normalizedQuery),
    ).toEqual([
      'chrome history sqlite',
      'sqlite wal 教学',
      'tauri sqlite local-first',
    ])
    expect(
      buildMockSearchQueries({
        sort: 'exact-frequency',
      }).rows.map((row) => row.normalizedQuery),
    ).toEqual([
      'chrome history sqlite',
      'tauri sqlite local-first',
      'sqlite wal 教学',
    ])
    expect(
      buildMockSearchQueries({
        page: 1,
        pageSize: 1,
        sort: 'family-frequency',
      }),
    ).toMatchObject({
      page: 1,
      pageSize: 1,
      total: 3,
      rows: [{ normalizedQuery: 'tauri sqlite local-first' }],
    })

    vi.useRealTimers()
  })

  test('orders preview search-query rows through deterministic tie breakers', () => {
    const row = (
      overrides: Partial<
        ReturnType<typeof buildMockSearchQueries>['rows'][number]
      >,
    ): ReturnType<typeof buildMockSearchQueries>['rows'][number] => ({
      browserKind: 'chrome',
      displayName: 'Google',
      exactRepeatCount: 1,
      familyCount: 1,
      familyId: 'family',
      normalizedQuery: 'alpha',
      profileId: 'chrome:Default',
      rawQuery: 'alpha',
      searchEngine: 'google',
      searchedAt: '2026-04-25T00:00:00Z',
      searchedAtMs: 100,
      trailId: null,
      trailInitialQuery: null,
      trailReformulationCount: null,
      visitId: 1,
      ...overrides,
    })

    expect(
      compareMockSearchQueryRows(
        'family-frequency',
        row({ familyCount: 1 }),
        row({ familyCount: 2 }),
      ),
    ).toBeGreaterThan(0)
    expect(
      compareMockSearchQueryRows(
        'family-frequency',
        row({ exactRepeatCount: 1, familyCount: 2 }),
        row({ exactRepeatCount: 3, familyCount: 2 }),
      ),
    ).toBeGreaterThan(0)
    expect(
      compareMockSearchQueryRows(
        'family-frequency',
        row({ exactRepeatCount: 2, familyCount: 2, searchedAtMs: 100 }),
        row({ exactRepeatCount: 2, familyCount: 2, searchedAtMs: 200 }),
      ),
    ).toBeGreaterThan(0)
    expect(
      compareMockSearchQueryRows(
        'exact-frequency',
        row({ exactRepeatCount: 1, searchedAtMs: 100 }),
        row({ exactRepeatCount: 1, searchedAtMs: 200 }),
      ),
    ).toBeGreaterThan(0)
  })

  test('filters history with regex, cursor, page, domain, time, and sort boundaries', () => {
    const state = createMockState()
    state.history.items = [
      historyItem(1, 'chrome:Default', 1000, 'https://a.example/docs', 'Alpha'),
      historyItem(2, 'chrome:Default', 2000, 'https://b.example/docs', 'Beta'),
      historyItem(
        3,
        'safari:Personal',
        3000,
        'https://a.example/news',
        'Gamma',
      ),
    ]

    expect(parseMockHistoryCursor()).toBeNull()
    expect(parseMockHistoryCursor('bad|cursor')).toBeNull()
    expect(parseMockHistoryCursor('2000|2')).toEqual({ id: 2, visitTime: 2000 })
    expect(encodeMockHistoryCursor(state.history.items[1])).toBe('2000|2')

    expect(
      filterMockHistory(state, {
        browserKind: 'chrome',
        domain: 'b.example',
        limit: 10,
        q: 'Beta',
        regexMode: true,
      }).items.map((item) => item.id),
    ).toEqual([2])
    expect(
      filterMockHistory(state, {
        cursor: '3000|3',
        limit: 1,
        sort: 'newest',
      }),
    ).toMatchObject({
      hasNext: true,
      hasPrevious: true,
      items: [{ id: 2, favicon: null }],
      nextCursor: '2000|2',
      page: 2,
      pageCount: 3,
    })
    expect(
      filterMockHistory(state, {
        cursor: '1000|1',
        limit: 1,
        sort: 'oldest',
      }).items.map((item) => item.id),
    ).toEqual([2])
    expect(
      filterMockHistory(state, {
        endTimeMs: 2500,
        limit: 1,
        page: 99,
        profileId: 'chrome:Default',
        q: 'docs',
        sort: 'oldest',
        startTimeMs: 500,
      }),
    ).toMatchObject({
      hasNext: false,
      hasPrevious: true,
      items: [{ id: 2, favicon: null }],
      page: 2,
      pageCount: 2,
    })
  })

  test('deduplicates favicon lookups and paginates lexical AI search previews', () => {
    const state = createMockState()
    state.history.items = [
      historyItem(
        1,
        'chrome:Default',
        1000,
        'https://a.example/docs',
        'Alpha',
        {
          favicon: { dataUrl: 'data:image/png;base64,AAA=' },
        },
      ),
      historyItem(2, 'chrome:Default', 2000, 'https://b.example/docs', 'Beta'),
      historyItem(3, 'chrome:Default', 3000, 'https://c.example/docs', 'Gamma'),
    ]

    expect(
      loadMockHistoryFavicons(state, [
        {
          profileId: 'chrome:Default',
          url: 'https://a.example/docs',
          visitTime: 1000,
        },
        {
          profileId: 'chrome:Default',
          url: 'https://a.example/docs',
          visitTime: 1000,
        },
        {
          profileId: 'chrome:Default',
          url: 'https://missing.example',
          visitTime: 4000,
        },
      ]),
    ).toEqual([
      {
        favicon: { dataUrl: 'data:image/png;base64,AAA=' },
        profileId: 'chrome:Default',
        url: 'https://a.example/docs',
        visitTime: 1000,
      },
      {
        favicon: null,
        profileId: 'chrome:Default',
        url: 'https://missing.example',
        visitTime: 4000,
      },
    ])

    expect(
      paginateMockAiSearch(state, {
        cursor: 'not-a-number',
        limit: 2,
        query: 'ignored',
      }),
    ).toMatchObject({
      items: [{ historyId: 1 }, { historyId: 2 }],
      nextCursor: '2',
      providerId: 'lexical-fallback',
      total: 3,
    })
    expect(
      paginateMockAiSearch(state, {
        cursor: '2',
        limit: 100,
        query: 'ignored',
      }),
    ).toMatchObject({
      items: [{ historyId: 3 }],
      nextCursor: null,
      total: 3,
    })
  })
})

function historyItem(
  id: number,
  profileId: string,
  visitTime: number,
  url: string,
  title: string,
  overrides: Partial<
    ReturnType<typeof createMockState>['history']['items'][number]
  > = {},
): ReturnType<typeof createMockState>['history']['items'][number] {
  return {
    appId: null,
    domain: new URL(url).hostname,
    durationMs: null,
    favicon: null,
    id,
    profileId,
    sourceVisitId: id,
    title,
    transition: null,
    url,
    visitedAt: new Date(visitTime).toISOString(),
    visitTime,
    ...overrides,
  }
}
