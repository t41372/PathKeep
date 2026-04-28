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

  test('loads cold primary data first, then starts the deferred secondary band', async () => {
    peekIntelligencePrimaryOverviewMock.mockReturnValue(null)
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockResolvedValue(
      primaryOverviewFixture(),
    )
    loadIntelligenceSecondaryOverviewMock.mockResolvedValue(
      secondaryOverviewFixture(),
    )

    const { result } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, null),
    )

    expect(result.current).toMatchObject({
      scopeKey: '2026-01-01:2026-03-31:archive-wide',
      primaryReady: false,
      secondaryReady: false,
      secondaryLoading: false,
    })

    await act(async () => {
      await Promise.resolve()
    })

    expect(result.current.primaryReady).toBe(true)
    expect(result.current.primaryError).toBeNull()
    expect(loadIntelligencePrimaryOverviewMock).toHaveBeenCalledWith(
      dateRange,
      null,
      { force: false },
    )

    await act(async () => {
      vi.runOnlyPendingTimers()
      vi.advanceTimersByTime(160)
      await Promise.resolve()
    })

    expect(result.current).toMatchObject({
      secondaryReady: true,
      secondaryLoading: false,
      secondaryError: null,
    })
    expect(loadIntelligenceSecondaryOverviewMock).toHaveBeenCalledWith(
      dateRange,
      null,
      { force: false },
    )
  })

  test('surfaces primary load failures as a completed degraded state', async () => {
    peekIntelligencePrimaryOverviewMock.mockReturnValue(null)
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockRejectedValue(
      new Error('primary unavailable'),
    )

    const { result } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    await act(async () => {
      await Promise.resolve()
    })

    expect(result.current).toMatchObject({
      primaryReady: true,
      primaryError: 'primary unavailable',
      secondaryReady: true,
      secondaryLoading: false,
      secondaryError: null,
    })
    expect(loadIntelligenceSecondaryOverviewMock).not.toHaveBeenCalled()
  })

  test('keeps cached secondary data visible when deferred refresh fails', async () => {
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(
      secondaryOverviewFixture(),
    )
    loadIntelligencePrimaryOverviewMock.mockResolvedValue(
      primaryOverviewFixture(),
    )
    loadIntelligenceSecondaryOverviewMock.mockRejectedValue(
      new Error('secondary refresh failed'),
    )

    const { result } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    await act(async () => {
      vi.runOnlyPendingTimers()
      vi.runOnlyPendingTimers()
      await Promise.resolve()
    })
    await act(async () => {
      vi.advanceTimersByTime(160)
      vi.runOnlyPendingTimers()
      await Promise.resolve()
    })

    expect(result.current).toMatchObject({
      primaryReady: true,
      secondaryReady: true,
      secondaryLoading: false,
      secondaryError: 'secondary refresh failed',
    })
  })

  test('keeps warm primary data visible when its background refresh fails', async () => {
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockRejectedValue(
      'primary refresh failed',
    )
    loadIntelligenceSecondaryOverviewMock.mockResolvedValue(
      secondaryOverviewFixture(),
    )

    const { result } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    await act(async () => {
      vi.runOnlyPendingTimers()
      await Promise.resolve()
    })

    expect(result.current).toMatchObject({
      primaryReady: true,
      primaryError: 'primary refresh failed',
    })
    expect(loadIntelligencePrimaryOverviewMock).toHaveBeenCalledWith(
      dateRange,
      'chrome:Default',
      { force: true },
    )
  })

  test('keeps warm primary data visible when its background refresh throws an Error', async () => {
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockRejectedValue(
      new Error('primary refresh errored'),
    )
    loadIntelligenceSecondaryOverviewMock.mockResolvedValue(
      secondaryOverviewFixture(),
    )

    const { result } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    await act(async () => {
      vi.runOnlyPendingTimers()
      await Promise.resolve()
    })

    expect(result.current).toMatchObject({
      primaryReady: true,
      primaryError: 'primary refresh errored',
    })
  })

  test('promotes a rerendered warm-cache scope from the pending reveal frame', async () => {
    const nextDateRange: DateRange = {
      start: '2026-04-01',
      end: '2026-04-30',
    }
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockReturnValue(
      deferred<CoreIntelligencePrimaryOverview>().promise,
    )
    loadIntelligenceSecondaryOverviewMock.mockResolvedValue(
      secondaryOverviewFixture(),
    )

    const { result, rerender } = renderHook(
      ({ range }) => useStagedIntelligenceOverview(range, 'chrome:Default'),
      {
        initialProps: { range: dateRange },
      },
    )

    rerender({ range: nextDateRange })

    await act(async () => {
      vi.runOnlyPendingTimers()
      await Promise.resolve()
    })

    expect(result.current).toMatchObject({
      scopeKey: '2026-04-01:2026-04-30:chrome:Default',
      primaryReady: true,
      primaryError: null,
    })
  })

  test('surfaces string primary and secondary cold-load failures', async () => {
    peekIntelligencePrimaryOverviewMock.mockReturnValue(null)
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockRejectedValue('primary unavailable')

    const primaryFailure = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    await act(async () => {
      await Promise.resolve()
    })

    expect(primaryFailure.result.current).toMatchObject({
      primaryReady: true,
      primaryError: 'primary unavailable',
      secondaryReady: true,
      secondaryLoading: false,
    })
    primaryFailure.unmount()

    loadIntelligencePrimaryOverviewMock.mockReset()
    loadIntelligenceSecondaryOverviewMock.mockReset()
    peekIntelligencePrimaryOverviewMock.mockReturnValue(null)
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockResolvedValue(
      primaryOverviewFixture(),
    )
    loadIntelligenceSecondaryOverviewMock.mockRejectedValue(
      'secondary unavailable',
    )

    const secondaryFailure = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    await act(async () => {
      await Promise.resolve()
      vi.runOnlyPendingTimers()
      vi.advanceTimersByTime(160)
      await Promise.resolve()
    })

    expect(secondaryFailure.result.current).toMatchObject({
      primaryReady: true,
      secondaryReady: false,
      secondaryLoading: false,
      secondaryError: 'secondary unavailable',
    })
  })

  test('promotes a rerendered scope before cached background updates settle', async () => {
    const nextDateRange: DateRange = {
      start: '2026-04-01',
      end: '2026-04-30',
    }
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockRejectedValue(
      new Error('refresh failed after scope switch'),
    )
    loadIntelligenceSecondaryOverviewMock.mockResolvedValue(
      secondaryOverviewFixture(),
    )

    const { result, rerender } = renderHook(
      ({ range }) => useStagedIntelligenceOverview(range, 'chrome:Default'),
      {
        initialProps: { range: dateRange },
      },
    )

    rerender({ range: nextDateRange })

    expect(result.current).toMatchObject({
      scopeKey: '2026-04-01:2026-04-30:chrome:Default',
      primaryReady: false,
    })

    await act(async () => {
      await Promise.resolve()
    })

    expect(result.current).toMatchObject({
      scopeKey: '2026-04-01:2026-04-30:chrome:Default',
      primaryReady: true,
      primaryError: 'refresh failed after scope switch',
    })

    await act(async () => {
      vi.runOnlyPendingTimers()
      await Promise.resolve()
    })

    expect(result.current).toMatchObject({
      scopeKey: '2026-04-01:2026-04-30:chrome:Default',
      primaryReady: true,
    })
  })

  test('uses requestIdleCallback for deferred secondary refresh and cancels it on cleanup', async () => {
    const cancelIdleCallback = vi.fn()
    const requestIdleCallback = vi.fn((callback: IdleRequestCallback) => {
      callback({
        didTimeout: false,
        timeRemaining: () => 16,
      })
      return 42
    })
    Object.defineProperty(window, 'requestIdleCallback', {
      configurable: true,
      value: requestIdleCallback,
    })
    Object.defineProperty(window, 'cancelIdleCallback', {
      configurable: true,
      value: cancelIdleCallback,
    })
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

    const { result, unmount } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    await act(async () => {
      await vi.runAllTimersAsync()
    })

    expect(requestIdleCallback).toHaveBeenCalledWith(expect.any(Function), {
      timeout: 1200,
    })
    expect(result.current.secondaryReady).toBe(true)

    unmount()
    expect(cancelIdleCallback).toHaveBeenCalledWith(42)
  })

  test('cancels a pending animation-frame reveal on cleanup', () => {
    const cancelAnimationFrame = vi.fn()
    const requestAnimationFrame = vi.fn(() => 123)
    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      value: requestAnimationFrame,
    })
    Object.defineProperty(window, 'cancelAnimationFrame', {
      configurable: true,
      value: cancelAnimationFrame,
    })
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(
      secondaryOverviewFixture(),
    )
    loadIntelligencePrimaryOverviewMock.mockResolvedValue(
      primaryOverviewFixture(),
    )

    const { unmount } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    unmount()

    expect(requestAnimationFrame).toHaveBeenCalled()
    expect(cancelAnimationFrame).toHaveBeenCalledWith(123)
  })

  test('cancels a cold-load secondary animation frame on cleanup', async () => {
    const cancelAnimationFrame = vi.fn()
    const requestAnimationFrame = vi.fn(() => 456)
    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      value: requestAnimationFrame,
    })
    Object.defineProperty(window, 'cancelAnimationFrame', {
      configurable: true,
      value: cancelAnimationFrame,
    })
    peekIntelligencePrimaryOverviewMock.mockReturnValue(null)
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    const primaryLoad = deferred<CoreIntelligencePrimaryOverview>()
    loadIntelligencePrimaryOverviewMock.mockReturnValue(primaryLoad.promise)

    const { unmount } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    await act(async () => {
      primaryLoad.resolve(primaryOverviewFixture())
      await primaryLoad.promise
      await Promise.resolve()
    })
    unmount()

    expect(requestAnimationFrame).toHaveBeenCalled()
    expect(cancelAnimationFrame).toHaveBeenCalledWith(456)
  })

  test('ignores a cached-primary reveal frame when it fires after cleanup', () => {
    let reveal: FrameRequestCallback | null = null
    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      value: (callback: FrameRequestCallback) => {
        reveal = callback
        return 789
      },
    })
    Object.defineProperty(window, 'cancelAnimationFrame', {
      configurable: true,
      value: vi.fn(),
    })
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockResolvedValue(
      primaryOverviewFixture(),
    )

    const { result, unmount } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    unmount()
    act(() => {
      reveal?.(0)
    })

    expect(result.current.primaryReady).toBe(false)
  })

  test('does not start deferred secondary loading when the idle callback fires after cleanup', () => {
    const idleCallbacks: IdleRequestCallback[] = []
    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      value: (callback: FrameRequestCallback) => {
        callback(0)
        return 1
      },
    })
    Object.defineProperty(window, 'requestIdleCallback', {
      configurable: true,
      value: (callback: IdleRequestCallback) => {
        idleCallbacks.push(callback)
        return 2
      },
    })
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockResolvedValue(
      primaryOverviewFixture(),
    )

    const { unmount } = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )

    unmount()
    act(() => {
      idleCallbacks.forEach((callback) =>
        callback({ didTimeout: false, timeRemaining: () => 16 }),
      )
    })

    expect(loadIntelligenceSecondaryOverviewMock).not.toHaveBeenCalled()
  })

  test('ignores secondary refresh resolution and rejection after cleanup', async () => {
    Object.defineProperty(window, 'requestAnimationFrame', {
      configurable: true,
      value: (callback: FrameRequestCallback) => {
        callback(0)
        return 1
      },
    })
    Object.defineProperty(window, 'requestIdleCallback', {
      configurable: true,
      value: (callback: IdleRequestCallback) => {
        callback({ didTimeout: false, timeRemaining: () => 16 })
        return 2
      },
    })
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    loadIntelligencePrimaryOverviewMock.mockResolvedValue(
      primaryOverviewFixture(),
    )
    const secondaryLoad = deferred<CoreIntelligenceSecondaryOverview>()
    loadIntelligenceSecondaryOverviewMock.mockReturnValueOnce(
      secondaryLoad.promise,
    )

    const first = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )
    first.unmount()
    await act(async () => {
      secondaryLoad.resolve(secondaryOverviewFixture())
      await secondaryLoad.promise
    })

    const secondaryFailure = deferred<CoreIntelligenceSecondaryOverview>()
    loadIntelligenceSecondaryOverviewMock.mockReturnValueOnce(
      secondaryFailure.promise,
    )
    const second = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )
    second.unmount()
    await act(async () => {
      secondaryFailure.reject('secondary failed after cleanup')
      await secondaryFailure.promise.catch(() => undefined)
    })
  })

  test('ignores primary refresh and cold-load completions after cleanup', async () => {
    peekIntelligencePrimaryOverviewMock.mockReturnValue(
      primaryOverviewFixture(),
    )
    peekIntelligenceSecondaryOverviewMock.mockReturnValue(null)
    const warmRefresh = deferred<CoreIntelligencePrimaryOverview>()
    loadIntelligencePrimaryOverviewMock.mockReturnValueOnce(warmRefresh.promise)

    const warm = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )
    warm.unmount()
    await act(async () => {
      warmRefresh.reject('primary refresh failed after cleanup')
      await warmRefresh.promise.catch(() => undefined)
    })

    peekIntelligencePrimaryOverviewMock.mockReturnValue(null)
    const coldLoad = deferred<CoreIntelligencePrimaryOverview>()
    loadIntelligencePrimaryOverviewMock.mockReturnValueOnce(coldLoad.promise)

    const cold = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )
    cold.unmount()
    await act(async () => {
      coldLoad.resolve(primaryOverviewFixture())
      await coldLoad.promise
    })

    const coldFailure = deferred<CoreIntelligencePrimaryOverview>()
    loadIntelligencePrimaryOverviewMock.mockReturnValueOnce(coldFailure.promise)
    const failed = renderHook(() =>
      useStagedIntelligenceOverview(dateRange, 'chrome:Default'),
    )
    failed.unmount()
    await act(async () => {
      coldFailure.reject('primary failed after cleanup')
      await coldFailure.promise.catch(() => undefined)
    })
  })
})

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  let reject: (error: unknown) => void = () => {}
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })

  return { promise, reject, resolve }
}
