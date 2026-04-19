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
})
