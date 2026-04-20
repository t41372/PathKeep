import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  CoreIntelligencePrimaryOverview,
  CoreIntelligenceSecondaryOverview,
  DateRange,
} from '../../lib/core-intelligence'
import { useStagedIntelligenceOverview } from './use-staged-intelligence-overview'

const {
  loadIntelligencePrimaryOverviewMock,
  loadIntelligenceSecondaryOverviewMock,
  peekIntelligencePrimaryOverviewMock,
  peekIntelligenceSecondaryOverviewMock,
} = vi.hoisted(() => ({
  loadIntelligencePrimaryOverviewMock: vi.fn(),
  loadIntelligenceSecondaryOverviewMock: vi.fn(),
  peekIntelligencePrimaryOverviewMock: vi.fn(),
  peekIntelligenceSecondaryOverviewMock: vi.fn(),
}))

vi.mock('../../lib/core-intelligence', () => ({
  loadIntelligencePrimaryOverview: loadIntelligencePrimaryOverviewMock,
  loadIntelligenceSecondaryOverview: loadIntelligenceSecondaryOverviewMock,
  peekIntelligencePrimaryOverview: peekIntelligencePrimaryOverviewMock,
  peekIntelligenceSecondaryOverview: peekIntelligenceSecondaryOverviewMock,
}))

const dateRange: DateRange = {
  start: '2026-01-01',
  end: '2026-03-31',
}

function primaryOverviewFixture(): CoreIntelligencePrimaryOverview {
  return {
    digestSummary: {
      data: {
        dateRange,
        totalVisits: { value: 1, trend: 'flat' },
        totalSearches: { value: 1, trend: 'flat' },
        newDomains: { value: 1, trend: 'flat' },
        deepReadPages: { value: 1, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      meta: {
        sectionId: 'digest-summary',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
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
        window: {
          kind: 'calendar-day-history',
          referenceDate: '2026-04-20',
        },
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
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
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
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
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
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
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
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    queryFamilies: {
      data: { families: [], total: 0, page: 0, pageSize: 10 },
      meta: {
        sectionId: 'search-activity',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
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
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
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
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
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
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
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
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    timings: [],
    totalDurationMs: 1,
  }
}

function secondaryOverviewFixture(): CoreIntelligenceSecondaryOverview {
  return {
    stableSources: {
      data: [],
      meta: {
        sectionId: 'stable-sources',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    searchEffectiveness: {
      data: {
        engineStats: [],
        topResolvingSources: [],
        hardestTopics: [],
      },
      meta: {
        sectionId: 'search-effectiveness',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    frictionSignals: {
      data: [],
      meta: {
        sectionId: 'friction-signals',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    reopenedInvestigations: {
      data: [],
      meta: {
        sectionId: 'reopened-investigations',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    discoveryTrendWeek: {
      data: { points: [], availableYears: [2026] },
      meta: {
        sectionId: 'discovery-trend',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    breadthIndex: {
      data: { hhi: 0.5, breadthScore: 50, concentrationDomainCount: 1 },
      meta: {
        sectionId: 'breadth-index',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    pathFlows: {
      data: [],
      meta: {
        sectionId: 'path-flows',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    compareSets: {
      data: [],
      meta: {
        sectionId: 'compare-sets',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    multiBrowserDiff: {
      data: {
        profiles: [],
        exclusiveDomains: [],
        sharedDomains: [],
        categoryDistributions: [],
      },
      meta: {
        sectionId: 'multi-browser-diff',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    observedInteractions: {
      data: [],
      meta: {
        sectionId: 'observed-interactions',
        generatedAt: null,
        window: { kind: 'date-range', dateRange },
        moduleIds: [],
        sourceTables: [],
        includesEnrichment: false,
        state: 'ready',
        stateReason: null,
        notes: [],
      },
    },
    timings: [],
    totalDurationMs: 1,
  }
}

describe('useStagedIntelligenceOverview', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    loadIntelligencePrimaryOverviewMock.mockReset()
    loadIntelligenceSecondaryOverviewMock.mockReset()
    peekIntelligencePrimaryOverviewMock.mockReset()
    peekIntelligenceSecondaryOverviewMock.mockReset()
    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      value: (callback: FrameRequestCallback) =>
        window.setTimeout(() => callback(0), 0),
    })
    Object.defineProperty(window, 'cancelAnimationFrame', {
      configurable: true,
      value: (handle: number) => window.clearTimeout(handle),
    })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  test('keeps the skeleton visible for one paint before revealing warm cached data', async () => {
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(
      secondaryOverviewFixture(),
    )
    loadIntelligencePrimaryOverviewMock.mockResolvedValue(
      primaryOverviewFixture(),
    )
    loadIntelligenceSecondaryOverviewMock.mockResolvedValue(
      secondaryOverviewFixture(),
    )

    const { result } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    expect(result.current.primaryReady).toBe(false)
    expect(result.current.secondaryReady).toBe(false)

    await act(async () => {
      vi.runOnlyPendingTimers()
      await Promise.resolve()
    })

    expect(result.current.primaryReady).toBe(true)
    expect(loadIntelligencePrimaryOverviewMock).toHaveBeenCalledWith(
      dateRange,
      'chrome:Default',
      { force: true },
    )

    expect(result.current.secondaryReady).toBe(false)
    await act(async () => {
      vi.advanceTimersByTime(160)
      vi.runOnlyPendingTimers()
      await Promise.resolve()
    })
    expect(result.current.secondaryReady).toBe(true)
    expect(loadIntelligenceSecondaryOverviewMock).toHaveBeenCalledWith(
      dateRange,
      'chrome:Default',
      { force: true },
    )
  })
})
