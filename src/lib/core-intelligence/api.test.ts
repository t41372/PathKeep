/**
 * Verifies the typed Core Intelligence IPC wrappers.
 *
 * Why this file exists:
 * - The backend already ships payload-provider commands, so the front-end draft contract should
 *   prove it sends the exact command names and request envelopes we intend to support.
 * - These wrappers are tiny, but getting the invoke shape wrong would silently break future host
 *   consumers and be hard to spot from route tests alone.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'
import type { CoreIntelligenceSectionResult } from './types'

const { callMock } = vi.hoisted(() => ({
  callMock: vi.fn(),
}))

vi.mock('../backend-client/shared', () => ({
  call: callMock,
}))

describe('core intelligence api', () => {
  beforeEach(() => {
    callMock.mockReset()
    callMock.mockResolvedValue({})
    return import('./api').then(({ clearIntelligenceOverviewCache }) => {
      clearIntelligenceOverviewCache()
    })
  })

  test('requests embed cards through the backend payload-provider command', async () => {
    const { getIntelligenceEmbedCards } = await import('./api')

    await getIntelligenceEmbedCards(
      { start: '2024-04-01', end: '2024-04-30' },
      'chrome:Default',
      6,
    )

    expect(callMock).toHaveBeenCalledWith('get_intelligence_embed_cards', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
        limit: 6,
      },
    })
  })

  test('requests widget snapshots through the backend payload-provider command', async () => {
    const { getIntelligenceWidgetSnapshot } = await import('./api')

    await getIntelligenceWidgetSnapshot(
      { start: '2024-04-01', end: '2024-04-30' },
      'chrome:Default',
      4,
    )

    expect(callMock).toHaveBeenCalledWith('get_intelligence_widget_snapshot', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
        limit: 4,
      },
    })
  })

  test('requests public snapshots through the backend payload-provider command', async () => {
    const { getIntelligencePublicSnapshot } = await import('./api')

    await getIntelligencePublicSnapshot(
      { start: '2024-04-01', end: '2024-04-30' },
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith('get_intelligence_public_snapshot', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
      },
    })
  })

  test('requests local host previews through the backend host-preview command', async () => {
    const { previewIntelligenceLocalHost } = await import('./api')

    await previewIntelligenceLocalHost(
      { start: '2024-04-01', end: '2024-04-30' },
      'zh-CN',
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith('preview_intelligence_local_host', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
        locale: 'zh-CN',
      },
    })
  })

  test('requests local host builds through the backend host-build command', async () => {
    const { buildIntelligenceLocalHost } = await import('./api')

    await buildIntelligenceLocalHost(
      { start: '2024-04-01', end: '2024-04-30' },
      'en',
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith('build_intelligence_local_host', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
        locale: 'en',
      },
    })
  })

  test('normalizes legacy snake_case date-range section window metadata', async () => {
    const { getDigestSummary } = await import('./api')

    callMock.mockResolvedValueOnce({
      data: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        totalVisits: { value: 42, trend: 'flat' },
        totalSearches: { value: 7, trend: 'flat' },
        newDomains: { value: 3, trend: 'flat' },
        deepReadPages: { value: 2, trend: 'flat' },
        refindPages: { value: 1, trend: 'flat' },
      },
      meta: {
        sectionId: 'digest-summary',
        generatedAt: '2026-04-18T12:00:00Z',
        window: {
          kind: 'date-range',
          date_range: {
            start: '2024-04-01',
            end: '2024-04-30',
          },
        },
        moduleIds: ['daily-rollups'],
        sourceTables: ['daily_summary_rollups'],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    })

    const result = await getDigestSummary({
      start: '2024-04-01',
      end: '2024-04-30',
    })

    expect(result.meta.window).toEqual({
      kind: 'date-range',
      dateRange: {
        start: '2024-04-01',
        end: '2024-04-30',
      },
    })
  })

  test('prefers snake_case date-range metadata when camelCase metadata is malformed', async () => {
    const { getDigestSummary } = await import('./api')

    callMock.mockResolvedValueOnce({
      data: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        totalVisits: { value: 42, trend: 'flat' },
        totalSearches: { value: 7, trend: 'flat' },
        newDomains: { value: 3, trend: 'flat' },
        deepReadPages: { value: 2, trend: 'flat' },
        refindPages: { value: 1, trend: 'flat' },
      },
      meta: {
        sectionId: 'digest-summary',
        generatedAt: '2026-04-18T12:00:00Z',
        window: {
          kind: 'date-range',
          dateRange: 'bad-date-range',
          date_range: {
            start: '2024-04-02',
            end: '2024-04-29',
          },
        },
        moduleIds: ['daily-rollups'],
        sourceTables: ['daily_summary_rollups'],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    })

    const result = await getDigestSummary({
      start: '2024-04-01',
      end: '2024-04-30',
    })

    expect(result.meta.window).toEqual({
      kind: 'date-range',
      dateRange: {
        start: '2024-04-02',
        end: '2024-04-29',
      },
    })
  })

  test('normalizes legacy snake_case calendar-day-history metadata', async () => {
    const { getOnThisDay } = await import('./api')

    callMock.mockResolvedValueOnce({
      data: [],
      meta: {
        sectionId: 'on-this-day',
        generatedAt: '2026-04-18T12:00:00Z',
        window: {
          kind: 'calendar-day-history',
          reference_date: '2026-04-18',
        },
        moduleIds: ['daily-rollups'],
        sourceTables: ['daily_summary_rollups'],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    })

    const result = await getOnThisDay('chrome:Default')

    expect(result.meta.window).toEqual({
      kind: 'calendar-day-history',
      referenceDate: '2026-04-18',
    })
  })

  test('falls back to the requested calendar-day-history window when metadata omits a reference date', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-04-19T12:00:00Z'))
    try {
      const { getOnThisDay } = await import('./api')

      callMock.mockResolvedValueOnce({
        data: [],
        meta: {
          sectionId: 'on-this-day',
          generatedAt: '2026-04-18T12:00:00Z',
          window: {
            kind: 'calendar-day-history',
            referenceDate: 123,
            reference_date: null,
          },
          moduleIds: ['daily-rollups'],
          sourceTables: ['daily_summary_rollups'],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      })

      const result = await getOnThisDay('chrome:Default')

      expect(result.meta.window).toEqual({
        kind: 'calendar-day-history',
        referenceDate: '2026-04-19',
      })
    } finally {
      vi.useRealTimers()
    }
  })

  test('requests discovery trend with the explicit granularity and preserves available years', async () => {
    const { getDiscoveryTrend } = await import('./api')

    callMock.mockResolvedValueOnce({
      data: {
        points: [],
        availableYears: [2026, 2025, 2024],
      },
      meta: {
        sectionId: 'discovery-trend',
        generatedAt: '2026-04-19T12:00:00Z',
        window: {
          kind: 'date-range',
          dateRange: {
            start: '2026-01-01',
            end: '2026-12-31',
          },
        },
        moduleIds: ['daily-rollups'],
        sourceTables: ['daily_summary_rollups'],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    })

    const result = await getDiscoveryTrend(
      {
        start: '2026-01-01',
        end: '2026-12-31',
      },
      'chrome:Default',
      'day',
    )

    expect(callMock).toHaveBeenCalledWith('get_discovery_trend', {
      request: {
        dateRange: { start: '2026-01-01', end: '2026-12-31' },
        profileId: 'chrome:Default',
        granularity: 'day',
      },
    })
    expect(result.data.availableYears).toEqual([2026, 2025, 2024])
  })

  test('requests activity mix trend with explicit granularity', async () => {
    const { getActivityMixTrend } = await import('./api')

    await getActivityMixTrend(
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
      'week',
    )

    expect(callMock).toHaveBeenCalledWith('get_activity_mix_trend', {
      request: {
        dateRange: { start: '2026-04-01', end: '2026-04-30' },
        profileId: 'chrome:Default',
        granularity: 'week',
      },
    })
  })

  test('requests day insights through the dedicated day-entity command', async () => {
    const { getDayInsights } = await import('./api')

    await getDayInsights('2026-04-18', 'chrome:Default')

    expect(callMock).toHaveBeenCalledWith('get_day_insights', {
      request: {
        date: '2026-04-18',
        profileId: 'chrome:Default',
      },
    })
  })

  test('dedupes inflight primary-overview reads and seeds first-band section cache', async () => {
    const { getDigestSummary, loadIntelligencePrimaryOverview } =
      await import('./api')

    callMock.mockResolvedValueOnce({
      digestSummary: {
        data: {
          dateRange: { start: '2026-04-01', end: '2026-04-30' },
          totalVisits: { value: 42, trend: 'flat' },
          totalSearches: { value: 7, trend: 'flat' },
          newDomains: { value: 3, trend: 'flat' },
          deepReadPages: { value: 2, trend: 'flat' },
          refindPages: { value: 1, trend: 'flat' },
        },
        meta: {
          sectionId: 'digest-summary',
          generatedAt: '2026-04-18T12:00:00Z',
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['daily-rollups'],
          sourceTables: ['daily_summary_rollups'],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      onThisDay: {
        data: [],
        meta: {
          sectionId: 'on-this-day',
          generatedAt: null,
          window: { kind: 'calendar-day-history', referenceDate: '2026-04-18' },
          moduleIds: [],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      topSites: {
        data: [],
        meta: {
          sectionId: 'top-sites',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['daily-rollups'],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      refindPages: {
        data: [],
        meta: {
          sectionId: 'refind-pages',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['refind-pages'],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      searchEngineRanking: {
        data: [],
        meta: {
          sectionId: 'search-activity',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['daily-rollups'],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      topSearchConcepts: {
        data: [],
        meta: {
          sectionId: 'search-activity',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['search-trails'],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      queryFamilies: {
        data: { page: 0, pageSize: 10, total: 0, families: [] },
        meta: {
          sectionId: 'search-activity',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['search-trails'],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      activityMix: {
        data: { categories: [], changeVsPrevious: [] },
        meta: {
          sectionId: 'activity-mix',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['activity-mix'],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      discoveryTrendDay: {
        data: { points: [], availableYears: [2026] },
        meta: {
          sectionId: 'browsing-rhythm',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['daily-rollups'],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      habitPatterns: {
        data: [],
        meta: {
          sectionId: 'habits',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['domain-deep-dive'],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      interruptedHabits: {
        data: [],
        meta: {
          sectionId: 'habits',
          generatedAt: null,
          window: {
            kind: 'date-range',
            dateRange: { start: '2026-04-01', end: '2026-04-30' },
          },
          moduleIds: ['domain-deep-dive'],
          sourceTables: [],
          includesEnrichment: false,
          state: 'ready',
          stateReason: null,
          notes: [],
        },
      },
      timings: [{ sectionId: 'digest-summary', durationMs: 4 }],
      totalDurationMs: 12,
    })

    const request = { start: '2026-04-01', end: '2026-04-30' } as const
    const [first, second] = await Promise.all([
      loadIntelligencePrimaryOverview(request, 'chrome:Default'),
      loadIntelligencePrimaryOverview(request, 'chrome:Default'),
    ])

    expect(callMock).toHaveBeenCalledTimes(1)
    expect(first.digestSummary.data.totalVisits.value).toBe(42)
    expect(second.digestSummary.data.totalVisits.value).toBe(42)

    const digest = await getDigestSummary(request, 'chrome:Default')
    expect(callMock).toHaveBeenCalledTimes(1)
    expect(digest.data.totalVisits.value).toBe(42)
  })

  test('requests search-query history through the shared section command', async () => {
    const { getSearchQueries } = await import('./api')

    await getSearchQueries(
      { start: '2026-04-01', end: '2026-04-30' },
      {
        profileId: 'chrome:Default',
        browserKind: 'chrome',
        engine: 'google',
        domain: 'google.com',
        query: 'sqlite',
        sort: 'family-frequency',
        pagination: { page: 1, pageSize: 40 },
      },
    )

    expect(callMock).toHaveBeenCalledWith('get_search_queries', {
      request: {
        dateRange: { start: '2026-04-01', end: '2026-04-30' },
        profileId: 'chrome:Default',
        browserKind: 'chrome',
        engine: 'google',
        domain: 'google.com',
        query: 'sqlite',
        sort: 'family-frequency',
        page: 1,
        pageSize: 40,
      },
    })
  })

  test('requests query-family detail through the dedicated entity command', async () => {
    const { getQueryFamilyDetail } = await import('./api')

    await getQueryFamilyDetail(
      'family-123',
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith('get_query_family_detail', {
      request: {
        familyId: 'family-123',
        dateRange: { start: '2026-04-01', end: '2026-04-30' },
        profileId: 'chrome:Default',
      },
    })
  })

  test('requests refind-page detail through the dedicated entity command', async () => {
    const { getRefindPageDetail } = await import('./api')

    await getRefindPageDetail(
      'https://example.com/docs',
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith('get_refind_page_detail', {
      request: {
        canonicalUrl: 'https://example.com/docs',
        dateRange: { start: '2026-04-01', end: '2026-04-30' },
        profileId: 'chrome:Default',
      },
    })
  })

  test('requests compare-set detail through the dedicated entity command', async () => {
    const { getCompareSetDetail } = await import('./api')

    await getCompareSetDetail(
      'compare:trail-1:docs_page',
      { start: '2026-04-01', end: '2026-04-30' },
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith('get_compare_set_detail', {
      request: {
        compareSetId: 'compare:trail-1:docs_page',
        dateRange: { start: '2026-04-01', end: '2026-04-30' },
        profileId: 'chrome:Default',
      },
    })
  })

  test('normalizes section metadata fallbacks and filters malformed envelope fields', async () => {
    const { normalizeSectionResult } = await import('./api/shared')
    const fallbackWindow = {
      kind: 'date-range' as const,
      dateRange: { start: '2026-04-01', end: '2026-04-30' },
    }

    expect(
      normalizeSectionResult('plain-section', fallbackWindow, {
        value: 1,
      }).meta,
    ).toEqual({
      sectionId: 'plain-section',
      generatedAt: null,
      window: fallbackWindow,
      moduleIds: [],
      sourceTables: [],
      includesEnrichment: false,
      state: 'degraded',
      stateReason: null,
      notes: [],
    })

    const normalized = normalizeSectionResult('safe-section', fallbackWindow, {
      data: ['row'],
      meta: {
        sectionId: 42,
        generatedAt: 7,
        window: {
          kind: 'date-range',
          date_range: {
            start: 1,
          },
        },
        moduleIds: ['daily-rollups', 5],
        sourceTables: ['daily_summary_rollups', null],
        includesEnrichment: 1,
        state: 'unknown',
        stateReason: 123,
        notes: ['kept', false],
      },
    } as unknown as CoreIntelligenceSectionResult<string[]>)

    expect(normalized).toMatchObject({
      data: ['row'],
      meta: {
        sectionId: 'safe-section',
        generatedAt: null,
        window: fallbackWindow,
        moduleIds: ['daily-rollups'],
        sourceTables: ['daily_summary_rollups'],
        includesEnrichment: true,
        state: 'degraded',
        stateReason: null,
        notes: ['kept'],
      },
    })

    expect(
      normalizeSectionResult('unknown-window', fallbackWindow, {
        data: null,
        meta: {
          window: {
            kind: 'not-supported',
          },
        },
      } as unknown as CoreIntelligenceSectionResult<null>).meta.window,
    ).toEqual(fallbackWindow)

    expect(
      normalizeSectionResult('missing-meta', fallbackWindow, {
        data: ['row'],
        meta: null,
      } as unknown as CoreIntelligenceSectionResult<string[]>),
    ).toEqual({
      data: ['row'],
      meta: {
        sectionId: 'missing-meta',
        generatedAt: null,
        window: fallbackWindow,
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'degraded',
        stateReason: null,
        notes: [],
      },
    })

    expect(
      normalizeSectionResult('non-object-window', fallbackWindow, {
        data: [],
        meta: {
          window: 'last-thirty-days',
        },
      } as unknown as CoreIntelligenceSectionResult<unknown[]>).meta.window,
    ).toBe(fallbackWindow)

    expect(
      normalizeSectionResult(
        'date-range-without-fallback',
        { kind: 'calendar-day-history', referenceDate: '2026-04-25' },
        {
          data: [],
          meta: {
            window: {
              kind: 'date-range',
              dateRange: {
                start: 1,
                end: 2,
              },
            },
          },
        } as unknown as CoreIntelligenceSectionResult<unknown[]>,
      ).meta.window,
    ).toEqual({
      kind: 'date-range',
      dateRange: { start: '', end: '' },
    })

    expect(
      normalizeSectionResult('date-range-snake-case', fallbackWindow, {
        data: [],
        meta: {
          window: {
            kind: 'date-range',
            date_range: {
              start: '2026-05-01',
              end: '2026-05-31',
            },
          },
        },
      } as unknown as CoreIntelligenceSectionResult<unknown[]>).meta.window,
    ).toEqual({
      kind: 'date-range',
      dateRange: { start: '2026-05-01', end: '2026-05-31' },
    })

    expect(
      normalizeSectionResult(
        'date-range-malformed-snake-case',
        fallbackWindow,
        {
          data: [],
          meta: {
            window: {
              kind: 'date-range',
              date_range: '2026-05',
            },
          },
        } as unknown as CoreIntelligenceSectionResult<unknown[]>,
      ).meta.window,
    ).toEqual(fallbackWindow)

    expect(
      normalizeSectionResult('calendar-reference-date', fallbackWindow, {
        data: [],
        meta: {
          window: {
            kind: 'calendar-day-history',
            referenceDate: '2026-04-25',
          },
        },
      } as unknown as CoreIntelligenceSectionResult<unknown[]>).meta.window,
    ).toEqual({
      kind: 'calendar-day-history',
      referenceDate: '2026-04-25',
    })

    expect(
      normalizeSectionResult(
        'calendar-reference-date-snake-case',
        fallbackWindow,
        {
          data: [],
          meta: {
            window: {
              kind: 'calendar-day-history',
              reference_date: '2026-04-26',
            },
          },
        } as unknown as CoreIntelligenceSectionResult<unknown[]>,
      ).meta.window,
    ).toEqual({
      kind: 'calendar-day-history',
      referenceDate: '2026-04-26',
    })

    expect(
      normalizeSectionResult('calendar-without-fallback', fallbackWindow, {
        data: [],
        meta: {
          window: {
            kind: 'calendar-day-history',
          },
        },
      } as unknown as CoreIntelligenceSectionResult<unknown[]>).meta.window,
    ).toMatchObject({
      kind: 'calendar-day-history',
    })
  })

  test('reads cached profile/date-range sections after skipping non-matching overview entries', async () => {
    const {
      cachedPrimarySectionForProfile,
      cachedSecondarySectionForDateRange,
      writeOverviewCache,
    } = await import('./api/shared')
    const april = { start: '2026-04-01', end: '2026-04-30' }
    const may = { start: '2026-05-01', end: '2026-05-31' }
    const targetPrimary = { data: ['target-primary'], meta: null }
    const targetSecondary = { data: ['target-secondary'], meta: null }

    writeOverviewCache(april, 'safari:Default', () => ({
      primary: {
        interruptedHabits: { data: ['wrong-profile'], meta: null },
      } as unknown as Parameters<
        Parameters<typeof writeOverviewCache>[2]
      >[0]['primary'],
    }))
    writeOverviewCache(may, 'chrome:Default', () => ({
      secondary: {
        multiBrowserDiff: { data: ['wrong-range'], meta: null },
      } as unknown as Parameters<
        Parameters<typeof writeOverviewCache>[2]
      >[0]['secondary'],
    }))
    writeOverviewCache(april, 'chrome:Default', () => ({
      primary: {
        interruptedHabits: targetPrimary,
      } as unknown as Parameters<
        Parameters<typeof writeOverviewCache>[2]
      >[0]['primary'],
      secondary: {
        multiBrowserDiff: targetSecondary,
      } as unknown as Parameters<
        Parameters<typeof writeOverviewCache>[2]
      >[0]['secondary'],
    }))

    expect(
      cachedPrimarySectionForProfile('chrome:Default', (overview) =>
        (overview.interruptedHabits?.data as unknown as string[])[0] ===
        'target-primary'
          ? overview.interruptedHabits
          : undefined,
      ),
    ).toBe(targetPrimary)
    expect(
      cachedSecondarySectionForDateRange(april, (overview) =>
        (overview.multiBrowserDiff?.data as unknown as string[])[0] ===
        'target-secondary'
          ? overview.multiBrowserDiff
          : undefined,
      ),
    ).toBe(targetSecondary)
  })

  test('normalizes secondary overview envelopes and drops malformed timing rows', async () => {
    const { normalizeSecondaryOverview } = await import('./api/shared')
    const dateRange = { start: '2026-04-01', end: '2026-04-30' }
    const normalized = normalizeSecondaryOverview(dateRange, {
      stableSources: ['stable'],
      searchEffectiveness: [],
      frictionSignals: [],
      reopenedInvestigations: [],
      discoveryTrendWeek: [],
      breadthIndex: [],
      pathFlows: [],
      compareSets: [],
      multiBrowserDiff: [],
      observedInteractions: [],
      timings: [
        null,
        { sectionId: 'missing-duration' },
        { sectionId: 'bad-duration', durationMs: '4' },
        { sectionId: 'stable-sources', durationMs: 4 },
      ],
      totalDurationMs: 'slow',
    } as unknown as Parameters<typeof normalizeSecondaryOverview>[1])

    expect(normalized.stableSources).toMatchObject({
      data: ['stable'],
      meta: {
        sectionId: 'stable-sources',
        state: 'degraded',
        window: { kind: 'date-range', dateRange },
      },
    })
    expect(normalized.timings).toEqual([
      { sectionId: 'stable-sources', durationMs: 4 },
    ])
    expect(normalized.totalDurationMs).toBe(0)
  })

  test('normalizes primary overview timing and duration fallbacks', async () => {
    const { normalizePrimaryOverview } = await import('./api/shared')
    const dateRange = { start: '2026-04-01', end: '2026-04-30' }
    const normalized = normalizePrimaryOverview(dateRange, {
      digestSummary: {
        dateRange,
        totalVisits: { value: 1, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 1, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      onThisDay: [],
      topSites: [],
      refindPages: [],
      searchEngineRanking: [],
      topSearchConcepts: [],
      queryFamilies: [],
      activityMix: [],
      discoveryTrendDay: [],
      habitPatterns: [],
      interruptedHabits: [],
      timings: { sectionId: 'not-an-array', durationMs: 1 },
      totalDurationMs: 'slow',
    } as unknown as Parameters<typeof normalizePrimaryOverview>[1])

    expect(normalized.digestSummary).toMatchObject({
      data: {
        totalVisits: { value: 1, trend: 'flat' },
      },
      meta: {
        sectionId: 'digest-summary',
        state: 'degraded',
        window: { kind: 'date-range', dateRange },
      },
    })
    expect(normalized.onThisDay.meta.window).toMatchObject({
      kind: 'calendar-day-history',
    })
    expect(normalized.timings).toEqual([])
    expect(normalized.totalDurationMs).toBe(0)
  })

  test('requests entity drilldowns through stable command envelopes', async () => {
    const {
      explainEntity,
      explainRefind,
      getDomainDeepDive,
      getDomainTrend,
      getHubPages,
      getNavigationPath,
      getSearchTrails,
      getSessionDetail,
      getSessions,
      getTrailDetail,
    } = await import('./api/entities')
    const dateRange = { start: '2026-04-01', end: '2026-04-30' }

    await getDomainTrend('example.com', dateRange)
    await explainRefind('https://example.com/docs')
    await getSessions(dateRange, 'chrome:Default', { page: 2, pageSize: 30 })
    await getSessionDetail('session-1')
    await getSearchTrails(dateRange, 'chrome:Default', 'google', {
      page: 1,
      pageSize: 15,
    })
    await getTrailDetail('trail-1')
    await getNavigationPath(42)
    await getHubPages(dateRange, 'chrome:Default', 8)
    await getDomainDeepDive('example.com', dateRange, 'chrome:Default')
    await explainEntity('domain', 'example.com')

    expect(callMock).toHaveBeenNthCalledWith(1, 'get_domain_trend', {
      request: {
        registrableDomain: 'example.com',
        dateRange,
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(2, 'explain_refind', {
      request: {
        canonicalUrl: 'https://example.com/docs',
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(3, 'get_sessions', {
      request: {
        dateRange,
        profileId: 'chrome:Default',
        page: 2,
        pageSize: 30,
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(4, 'get_session_detail', {
      sessionId: 'session-1',
    })
    expect(callMock).toHaveBeenNthCalledWith(5, 'get_search_trails', {
      request: {
        dateRange,
        profileId: 'chrome:Default',
        engine: 'google',
        page: 1,
        pageSize: 15,
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(6, 'get_trail_detail', {
      trailId: 'trail-1',
    })
    expect(callMock).toHaveBeenNthCalledWith(7, 'get_navigation_path', {
      visitId: 42,
    })
    expect(callMock).toHaveBeenNthCalledWith(8, 'get_hub_pages', {
      request: {
        dateRange,
        profileId: 'chrome:Default',
        limit: 8,
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(9, 'get_domain_deep_dive', {
      request: {
        registrableDomain: 'example.com',
        dateRange,
        profileId: 'chrome:Default',
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(10, 'explain_entity', {
      request: {
        entityType: 'domain',
        entityId: 'example.com',
      },
    })
  })

  test('uses default entity-list pagination when callers omit it', async () => {
    const { getSearchTrails, getSessions } = await import('./api/entities')
    const dateRange = { start: '2026-04-01', end: '2026-04-30' }

    await getSessions(dateRange)
    await getSearchTrails(dateRange)

    expect(callMock).toHaveBeenNthCalledWith(1, 'get_sessions', {
      request: {
        dateRange,
        profileId: undefined,
        page: 0,
        pageSize: 20,
      },
    })
    expect(callMock).toHaveBeenNthCalledWith(2, 'get_search_trails', {
      request: {
        dateRange,
        profileId: undefined,
        engine: undefined,
        page: 0,
        pageSize: 20,
      },
    })
  })
})
