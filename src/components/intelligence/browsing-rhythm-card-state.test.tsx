import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  CoreIntelligenceSectionMeta,
  CoreIntelligenceSectionResult,
  DateRange,
  DayInsights,
  DiscoveryTrend,
  DiscoveryTrendPoint,
} from '../../lib/core-intelligence'
import { useBrowsingRhythmCardState } from './browsing-rhythm-card-state'
import type { BrowsingRhythmTranslator } from './browsing-rhythm-card-helpers'

const { getDayInsightsMock, getDiscoveryTrendMock, peekDiscoveryTrendMock } =
  vi.hoisted(() => ({
    getDayInsightsMock: vi.fn(),
    getDiscoveryTrendMock: vi.fn(),
    peekDiscoveryTrendMock: vi.fn(),
  }))

vi.mock('../../lib/core-intelligence/api', () => ({
  getDayInsights: getDayInsightsMock,
  getDiscoveryTrend: getDiscoveryTrendMock,
  peekDiscoveryTrend: peekDiscoveryTrendMock,
}))

const t: BrowsingRhythmTranslator = (key, vars) => {
  if (!vars) {
    return key
  }

  return `${key}:${Object.entries(vars)
    .map(([name, value]) => `${name}=${value}`)
    .join('|')}`
}

function calendarYearRange(year: number): DateRange {
  return {
    start: `${year}-01-01`,
    end: `${year}-12-31`,
  }
}

function meta(
  dateRange: DateRange,
  sectionId = 'discovery-trend',
): CoreIntelligenceSectionMeta {
  return {
    sectionId,
    generatedAt: '2026-04-25T12:00:00Z',
    window: {
      kind: 'date-range',
      dateRange,
    },
    moduleIds: ['daily-rollups'],
    sourceTables: ['daily_summary_rollups'],
    includesEnrichment: false,
    state: 'ready',
    stateReason: null,
    notes: [],
  }
}

function section<T>(
  data: T,
  dateRange: DateRange,
  sectionId?: string,
): CoreIntelligenceSectionResult<T> {
  return {
    data,
    meta: meta(dateRange, sectionId),
  }
}

function point(dateKey: string, totalVisits: number): DiscoveryTrendPoint {
  return {
    dateKey,
    discoveryRate: 0.25,
    newDomainCount: Math.max(1, Math.floor(totalVisits / 2)),
    totalVisits,
  }
}

function trendFor(
  dateRange: DateRange,
  points: DiscoveryTrendPoint[],
  availableYears: number[],
) {
  return section<DiscoveryTrend>(
    {
      points,
      availableYears,
    },
    dateRange,
  )
}

function dayInsightsFixture(date: string): DayInsights {
  return {
    date,
    digestSummary: {
      dateRange: {
        start: date,
        end: date,
      },
      totalVisits: {
        value: 12,
        trend: 'flat',
      },
      totalSearches: {
        value: 3,
        trend: 'flat',
      },
      newDomains: {
        value: 2,
        trend: 'flat',
      },
      deepReadPages: {
        value: 1,
        trend: 'flat',
      },
      refindPages: {
        value: 0,
        trend: 'flat',
      },
    },
    topSites: [],
    activityMix: {
      categories: [],
      changeVsPrevious: [],
    },
    refindPages: [],
    queryFamilies: {
      families: [],
      total: 0,
      page: 0,
      pageSize: 10,
    },
    hourlyActivity: [],
    drilldown: {
      explorerDateRange: {
        start: date,
        end: date,
      },
    },
  }
}

describe('useBrowsingRhythmCardState', () => {
  beforeEach(() => {
    getDayInsightsMock.mockReset()
    getDiscoveryTrendMock.mockReset()
    peekDiscoveryTrendMock.mockReset()
    peekDiscoveryTrendMock.mockReturnValue(null)
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  test('loads yearly trend data, navigates years, and lazily reads selected-day detail', async () => {
    const currentYear = new Date().getFullYear()
    const previousYear = currentYear - 1
    const currentRange = calendarYearRange(currentYear)
    const previousRange = calendarYearRange(previousYear)
    const selectedDate = `${previousYear}-12-31`
    const onTrendMetaChange = vi.fn()

    getDiscoveryTrendMock.mockImplementation((dateRange: DateRange) => {
      if (dateRange.start === previousRange.start) {
        return Promise.resolve(
          trendFor(
            previousRange,
            [point(`${previousYear}-01-05`, 4), point(selectedDate, 18)],
            [currentYear, previousYear],
          ),
        )
      }

      return Promise.resolve(
        trendFor(
          currentRange,
          [point(`${currentYear}-01-03`, 8), point(`${currentYear}-04-10`, 14)],
          [currentYear, previousYear],
        ),
      )
    })
    getDayInsightsMock.mockResolvedValue(
      section(
        dayInsightsFixture(selectedDate),
        {
          start: selectedDate,
          end: selectedDate,
        },
        'day-insights',
      ),
    )

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'en',
        mode: 'year',
        onTrendMetaChange,
        profileId: null,
        showCurrentYearShortcut: true,
        summaryPreset: 'calendar-year',
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))

    expect(getDiscoveryTrendMock).toHaveBeenCalledWith(
      currentRange,
      null,
      'day',
      undefined,
    )
    expect(onTrendMetaChange).toHaveBeenLastCalledWith(
      expect.objectContaining({
        sectionId: 'discovery-trend',
      }),
    )
    expect(result.current.hasCalendarVisits).toBe(true)
    expect(result.current.maxVisits).toBe(14)
    expect(result.current.monthLabels.length).toBe(
      result.current.calendarWeeks.length,
    )
    expect(result.current.visibleRangeHint).toContain('rhythmVisibleRange')
    expect(result.current.visitSummary).toContain('rhythmVisitSummaryYear')
    expect(result.current.weekdayLabels).toEqual([
      'dow_sun',
      'dow_mon',
      'dow_tue',
      'dow_wed',
      'dow_thu',
      'dow_fri',
      'dow_sat',
    ])
    expect(result.current.olderYear).toBe(previousYear)

    act(() => {
      result.current.selectYear(previousYear)
    })

    await waitFor(() =>
      expect(getDiscoveryTrendMock).toHaveBeenLastCalledWith(
        previousRange,
        null,
        'day',
        undefined,
      ),
    )
    await waitFor(() => expect(result.current.trendLoading).toBe(false))

    expect(result.current.selectedYear).toBe(previousYear)
    expect(result.current.canResetToCurrentYear).toBe(true)
    expect(result.current.newerYear).toBe(currentYear)

    act(() => {
      result.current.selectDay(selectedDate)
    })

    await waitFor(() =>
      expect(getDayInsightsMock).toHaveBeenCalledWith(selectedDate, null),
    )
    await waitFor(() =>
      expect(result.current.selectedDayDetail?.date).toBe(selectedDate),
    )

    expect(result.current.selectedDay?.dateKey).toBe(selectedDate)
    expect(result.current.selectedDayLoading).toBe(false)
    expect(result.current.selectedDayError).toBeNull()

    act(() => {
      result.current.resetToCurrentYear()
    })

    await waitFor(() => expect(result.current.selectedYear).toBe(currentYear))
    expect(result.current.selectedDay).toBeNull()
  })

  test('forces a trend refresh when the parent refresh token changes', async () => {
    const currentYear = new Date().getFullYear()
    const dateRange: DateRange = {
      start: `${currentYear}-02-01`,
      end: `${currentYear}-02-07`,
    }
    const trend = trendFor(dateRange, [], [currentYear])

    getDiscoveryTrendMock.mockResolvedValue(trend)

    const { rerender, result } = renderHook(
      ({ refreshToken }: { refreshToken: number | null }) =>
        useBrowsingRhythmCardState({
          dateRange,
          language: 'en',
          mode: 'range',
          profileId: 'chrome:Default',
          refreshToken,
          showCurrentYearShortcut: false,
          summaryPreset: 'week',
          t,
        }),
      {
        initialProps: {
          refreshToken: null as number | null,
        },
      },
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    expect(getDiscoveryTrendMock).toHaveBeenLastCalledWith(
      dateRange,
      'chrome:Default',
      'day',
      undefined,
    )
    expect(result.current.visibleRangeHint).toBeNull()
    expect(result.current.hasCalendarVisits).toBe(false)
    expect(result.current.maxVisits).toBe(1)

    rerender({
      refreshToken: 1,
    })

    await waitFor(() =>
      expect(getDiscoveryTrendMock).toHaveBeenLastCalledWith(
        dateRange,
        'chrome:Default',
        'day',
        {
          force: true,
        },
      ),
    )
  })

  test('ignores unchanged or nullish refresh tokens instead of forcing the cached trend scope', async () => {
    const currentYear = new Date().getFullYear()
    const dateRange: DateRange = {
      start: `${currentYear}-02-01`,
      end: `${currentYear}-02-07`,
    }
    const trend = trendFor(
      dateRange,
      [point(`${currentYear}-02-03`, 3)],
      [currentYear],
    )

    getDiscoveryTrendMock.mockResolvedValue(trend)

    const { rerender, result } = renderHook(
      ({ refreshToken }: { refreshToken?: number | null }) =>
        useBrowsingRhythmCardState({
          dateRange,
          language: 'en',
          mode: 'range',
          profileId: null,
          refreshToken,
          showCurrentYearShortcut: false,
          summaryPreset: 'week',
          t,
        }),
      {
        initialProps: {
          refreshToken: null as number | null | undefined,
        },
      },
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    const initialCallCount = getDiscoveryTrendMock.mock.calls.length

    rerender({
      refreshToken: null,
    })
    await flushMicrotasks()
    expect(getDiscoveryTrendMock.mock.calls).toHaveLength(initialCallCount)

    rerender({
      refreshToken: undefined,
    })
    await flushMicrotasks()

    expect(getDiscoveryTrendMock.mock.calls).toHaveLength(initialCallCount)
    expect(
      getDiscoveryTrendMock.mock.calls.some((call) => call[3]?.force === true),
    ).toBe(false)

    rerender({
      refreshToken: 1,
    })
    await waitFor(() =>
      expect(getDiscoveryTrendMock).toHaveBeenLastCalledWith(
        dateRange,
        null,
        'day',
        { force: true },
      ),
    )
    getDiscoveryTrendMock.mockClear()

    rerender({
      refreshToken: null,
    })
    await flushMicrotasks()

    expect(getDiscoveryTrendMock).not.toHaveBeenCalled()
  })

  test('cancels queued force refreshes when the hook unmounts before the microtask runs', async () => {
    const queuedMicrotasks: VoidFunction[] = []
    vi.stubGlobal('queueMicrotask', (callback: VoidFunction) => {
      queuedMicrotasks.push(callback)
    })
    const dateRange: DateRange = {
      start: '2026-02-01',
      end: '2026-02-07',
    }
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(dateRange, [point('2026-02-03', 3)], [2026]),
    )

    const { rerender, result, unmount } = renderHook(
      ({ refreshToken }: { refreshToken: number | null }) =>
        useBrowsingRhythmCardState({
          dateRange,
          language: 'en',
          mode: 'range',
          profileId: null,
          refreshToken,
          showCurrentYearShortcut: false,
          t,
        }),
      {
        initialProps: {
          refreshToken: null as number | null,
        },
      },
    )

    await drainQueuedMicrotasks(queuedMicrotasks)
    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    getDiscoveryTrendMock.mockClear()

    rerender({
      refreshToken: 1,
    })
    unmount()
    await drainQueuedMicrotasks(queuedMicrotasks)
    await drainQueuedMicrotasks(queuedMicrotasks)

    expect(getDiscoveryTrendMock).not.toHaveBeenCalled()
  })

  test('drops queued force refreshes when range and profile scope change before the microtask runs', async () => {
    const queuedMicrotasks: VoidFunction[] = []
    vi.stubGlobal('queueMicrotask', (callback: VoidFunction) => {
      queuedMicrotasks.push(callback)
    })
    const firstRange: DateRange = {
      start: '2026-04-01',
      end: '2026-04-07',
    }
    const secondRange: DateRange = {
      start: '2026-05-01',
      end: '2026-05-07',
    }

    getDiscoveryTrendMock.mockImplementation((dateRange: DateRange) =>
      Promise.resolve(
        trendFor(
          dateRange,
          [point(`${dateRange.start.slice(0, 8)}02`, 2)],
          [Number(dateRange.start.slice(0, 4))],
        ),
      ),
    )

    const { rerender, result } = renderHook(
      ({
        dateRange,
        profileId,
        refreshToken,
      }: {
        dateRange: DateRange
        profileId: string | null
        refreshToken: number | null
      }) =>
        useBrowsingRhythmCardState({
          dateRange,
          language: 'en',
          mode: 'range',
          profileId,
          refreshToken,
          showCurrentYearShortcut: false,
          t,
        }),
      {
        initialProps: {
          dateRange: firstRange,
          profileId: null as string | null,
          refreshToken: null as number | null,
        },
      },
    )

    await drainQueuedMicrotasks(queuedMicrotasks)
    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    getDiscoveryTrendMock.mockClear()

    rerender({
      dateRange: firstRange,
      profileId: null,
      refreshToken: 1,
    })
    rerender({
      dateRange: secondRange,
      profileId: 'profile-b',
      refreshToken: 1,
    })
    await drainQueuedMicrotasks(queuedMicrotasks)
    await waitFor(() =>
      expect(getDiscoveryTrendMock).toHaveBeenCalledWith(
        secondRange,
        'profile-b',
        'day',
        undefined,
      ),
    )
    await drainQueuedMicrotasks(queuedMicrotasks)

    expect(
      getDiscoveryTrendMock.mock.calls.filter(
        ([dateRange, profileId, , options]) =>
          dateRange.start === secondRange.start &&
          profileId === 'profile-b' &&
          options?.force === true,
      ),
    ).toHaveLength(0)
  })

  test('keeps queued archive-wide force refreshes valid when profile scope changes from null to omitted', async () => {
    const queuedMicrotasks: VoidFunction[] = []
    vi.stubGlobal('queueMicrotask', (callback: VoidFunction) => {
      queuedMicrotasks.push(callback)
    })
    const dateRange: DateRange = {
      start: '2026-04-01',
      end: '2026-04-07',
    }

    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(dateRange, [point('2026-04-02', 2)], [2026]),
    )

    const { rerender, result } = renderHook(
      ({
        profileId,
        refreshToken,
      }: {
        profileId?: string | null
        refreshToken: number | null
      }) =>
        useBrowsingRhythmCardState({
          dateRange,
          language: 'en',
          mode: 'range',
          profileId,
          refreshToken,
          showCurrentYearShortcut: false,
          t,
        }),
      {
        initialProps: {
          profileId: null as string | null | undefined,
          refreshToken: null as number | null,
        },
      },
    )

    await drainQueuedMicrotasks(queuedMicrotasks)
    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    getDiscoveryTrendMock.mockClear()

    rerender({
      profileId: null,
      refreshToken: 1,
    })
    rerender({
      profileId: undefined,
      refreshToken: 1,
    })
    await drainQueuedMicrotasks(queuedMicrotasks)
    await drainQueuedMicrotasks(queuedMicrotasks)
    await waitFor(() =>
      expect(getDiscoveryTrendMock).toHaveBeenCalledWith(
        dateRange,
        undefined,
        'day',
        { force: true },
      ),
    )
  })

  test('falls back to the current calendar year for range mode without an explicit date range', async () => {
    const currentYear = new Date().getFullYear()
    const currentRange = calendarYearRange(currentYear)
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(currentRange, [], [currentYear]),
    )

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'en',
        mode: 'range',
        profileId: null,
        showCurrentYearShortcut: false,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    expect(getDiscoveryTrendMock).toHaveBeenCalledWith(
      currentRange,
      null,
      'day',
      undefined,
    )
    expect(result.current.visitSummary).toContain('rhythmVisitSummaryYear')

    act(() => {
      result.current.selectDay('1900-01-01')
    })

    expect(result.current.selectedDay).toBeNull()
    expect(getDayInsightsMock).not.toHaveBeenCalled()
  })

  test('year mode ignores explicit ranges and keeps the calendar scoped to the selected year', async () => {
    const currentYear = new Date().getFullYear()
    const currentRange = calendarYearRange(currentYear)
    const staleRange: DateRange = {
      start: '1999-01-01',
      end: '1999-12-31',
    }

    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(currentRange, [point(`${currentYear}-02-10`, 5)], [currentYear]),
    )

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        dateRange: staleRange,
        language: 'en',
        mode: 'year',
        profileId: 'profile-a',
        showCurrentYearShortcut: true,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))

    expect(getDiscoveryTrendMock).toHaveBeenCalledWith(
      currentRange,
      'profile-a',
      'day',
      undefined,
    )
    expect(getDiscoveryTrendMock).not.toHaveBeenCalledWith(
      staleRange,
      'profile-a',
      'day',
      undefined,
    )
    expect(result.current.calendarDays[0]?.dateKey).toBe(`${currentYear}-01-01`)
    expect(result.current.calendarDays.at(-1)?.dateKey).toBe(
      `${currentYear}-12-31`,
    )
    expect(result.current.waitingForYearRealignment).toBe(false)
  })

  test('range mode filters calendar padding days and keeps visible-range hints year-only', async () => {
    const dateRange: DateRange = {
      start: '2026-02-03',
      end: '2026-02-05',
    }
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(
        dateRange,
        [point('2026-02-03', 2), point('2026-02-05', 4)],
        [2026],
      ),
    )

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        dateRange,
        language: 'en',
        mode: 'range',
        profileId: null,
        showCurrentYearShortcut: false,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))

    expect(result.current.calendarDays.map((cell) => cell.dateKey)).toEqual([
      '2026-02-03',
      '2026-02-04',
      '2026-02-05',
    ])
    expect(result.current.maxVisits).toBe(4)
    expect(result.current.hasCalendarVisits).toBe(true)
    expect(result.current.visibleRangeHint).toBeNull()
    expect(result.current.visitSummary).toContain('rhythmVisitSummaryRange')
    expect(result.current.visitSummary).toContain('count=6')
    expect(result.current.waitingForYearRealignment).toBe(false)
  })

  test('updates calendar labels and weekday copy when language and translator props change', async () => {
    const dateRange: DateRange = {
      start: '2026-01-01',
      end: '2026-01-07',
    }
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(dateRange, [point('2026-01-02', 1)], [2026]),
    )
    const translated =
      (prefix: string): BrowsingRhythmTranslator =>
      (key, vars) =>
        vars
          ? `${prefix}:${key}:${Object.keys(vars).join(',')}`
          : `${prefix}:${key}`

    const { rerender, result } = renderHook(
      ({
        language,
        translator,
      }: {
        language: string
        translator: BrowsingRhythmTranslator
      }) =>
        useBrowsingRhythmCardState({
          dateRange,
          language,
          mode: 'range',
          profileId: null,
          showCurrentYearShortcut: false,
          t: translator,
        }),
      {
        initialProps: {
          language: 'en',
          translator: translated('en'),
        },
      },
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    const englishMonthLabels = result.current.monthLabels
    expect(result.current.weekdayLabels[0]).toBe('en:dow_sun')

    rerender({
      language: 'zh-CN',
      translator: translated('zh'),
    })

    await waitFor(() =>
      expect(result.current.weekdayLabels[0]).toBe('zh:dow_sun'),
    )
    expect(result.current.monthLabels).not.toEqual(englishMonthLabels)
  })

  test('does not reuse selected-day detail across profile or range scope changes', async () => {
    const firstRange: DateRange = {
      start: '2026-03-01',
      end: '2026-03-03',
    }
    const secondRange: DateRange = {
      start: '2026-03-01',
      end: '2026-03-03',
    }
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(firstRange, [point('2026-03-02', 7)], [2026]),
    )
    getDayInsightsMock.mockResolvedValue(
      section(
        dayInsightsFixture('2026-03-02'),
        {
          start: '2026-03-02',
          end: '2026-03-02',
        },
        'day-insights',
      ),
    )

    const { rerender, result } = renderHook(
      ({
        dateRange,
        profileId,
      }: {
        dateRange: DateRange
        profileId: string | null
      }) =>
        useBrowsingRhythmCardState({
          dateRange,
          language: 'en',
          mode: 'range',
          profileId,
          showCurrentYearShortcut: false,
          t,
        }),
      {
        initialProps: {
          dateRange: firstRange,
          profileId: null as string | null,
        },
      },
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))

    act(() => {
      result.current.selectDay('2026-03-02')
    })

    await waitFor(() =>
      expect(getDayInsightsMock).toHaveBeenCalledWith('2026-03-02', null),
    )
    await waitFor(() =>
      expect(result.current.selectedDayDetail?.date).toBe('2026-03-02'),
    )

    rerender({
      dateRange: secondRange,
      profileId: 'profile-b',
    })

    await waitFor(() => expect(result.current.selectedDay).toBeNull())
    await waitFor(() => expect(result.current.selectedDayDetail).toBeNull())
    expect(getDayInsightsMock).not.toHaveBeenCalledWith(
      '2026-03-02',
      'profile-b',
    )
  })

  test('keeps archive-wide selected days stable when profile scope is null or omitted', async () => {
    const dateRange: DateRange = {
      start: '2026-03-01',
      end: '2026-03-03',
    }
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(dateRange, [point('2026-03-02', 7)], [2026]),
    )
    getDayInsightsMock.mockResolvedValue(
      section(
        dayInsightsFixture('2026-03-02'),
        {
          start: '2026-03-02',
          end: '2026-03-02',
        },
        'day-insights',
      ),
    )

    const { rerender, result } = renderHook(
      ({ profileId }: { profileId?: string | null }) =>
        useBrowsingRhythmCardState({
          dateRange,
          language: 'en',
          mode: 'range',
          profileId,
          showCurrentYearShortcut: false,
          t,
        }),
      {
        initialProps: {
          profileId: null as string | null | undefined,
        },
      },
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    act(() => {
      result.current.selectDay('2026-03-02')
    })
    await waitFor(() =>
      expect(result.current.selectedDay?.dateKey).toBe('2026-03-02'),
    )

    rerender({
      profileId: undefined,
    })

    expect(result.current.selectedDay?.dateKey).toBe('2026-03-02')
  })

  test('formats partial-year visible range hints with both occupied boundaries', async () => {
    const currentYear = new Date().getFullYear()
    const currentRange = calendarYearRange(currentYear)
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(
        currentRange,
        [point(`${currentYear}-01-01`, 3), point(`${currentYear}-12-30`, 5)],
        [currentYear],
      ),
    )

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'en',
        mode: 'year',
        profileId: null,
        showCurrentYearShortcut: true,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))

    expect(result.current.visibleRangeHint).toContain('rhythmVisibleRange')
    expect(result.current.visibleRangeHint).toContain('start=')
    expect(result.current.visibleRangeHint).toContain('end=')
  })

  test('formats partial-year visible range hints when only the end boundary reaches year end', async () => {
    const currentYear = new Date().getFullYear()
    const currentRange = calendarYearRange(currentYear)
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(
        currentRange,
        [point(`${currentYear}-01-02`, 3), point(`${currentYear}-12-31`, 5)],
        [currentYear],
      ),
    )

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'en',
        mode: 'year',
        profileId: null,
        showCurrentYearShortcut: true,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))

    expect(result.current.visibleRangeHint).toContain('rhythmVisibleRange')
    expect(result.current.visibleRangeHint).toContain('start=')
    expect(result.current.visibleRangeHint).toContain('end=')
  })

  test('exposes year navigation boundaries without leaking reset controls into disabled modes', async () => {
    const currentYear = new Date().getFullYear()
    const previousYear = currentYear - 1
    const oldestYear = currentYear - 2
    const currentRange = calendarYearRange(currentYear)
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(
        currentRange,
        [point(`${currentYear}-01-02`, 1), point(`${oldestYear}-05-01`, 1)],
        [currentYear, previousYear, oldestYear],
      ),
    )

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'en',
        mode: 'year',
        profileId: null,
        showCurrentYearShortcut: false,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    expect(result.current.yearOptions.slice(0, 3)).toEqual([
      currentYear,
      previousYear,
      oldestYear,
    ])
    expect(result.current.newerYear).toBeNull()
    expect(result.current.olderYear).toBe(previousYear)
    expect(result.current.canResetToCurrentYear).toBe(false)

    act(() => {
      result.current.selectYear(previousYear)
    })

    await waitFor(() => expect(result.current.selectedYear).toBe(previousYear))
    expect(result.current.newerYear).toBe(currentYear)
    expect(result.current.olderYear).toBe(oldestYear)
    expect(result.current.canResetToCurrentYear).toBe(false)

    act(() => {
      result.current.selectYear(oldestYear)
    })

    await waitFor(() => expect(result.current.selectedYear).toBe(oldestYear))
    expect(result.current.newerYear).toBe(previousYear)
    expect(result.current.olderYear).toBeNull()
    expect(result.current.canResetToCurrentYear).toBe(false)
  })

  test('keeps reset controls hidden in range mode even after manual year selection', async () => {
    const currentYear = new Date().getFullYear()
    const previousYear = currentYear - 1
    const dateRange: DateRange = {
      start: `${currentYear}-06-01`,
      end: `${currentYear}-06-07`,
    }
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(dateRange, [point(`${currentYear}-06-03`, 1)], [currentYear]),
    )

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        dateRange,
        language: 'en',
        mode: 'range',
        profileId: null,
        showCurrentYearShortcut: true,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    act(() => {
      result.current.selectYear(previousYear)
    })

    expect(result.current.selectedYear).toBe(previousYear)
    expect(result.current.canResetToCurrentYear).toBe(false)
    expect(result.current.waitingForYearRealignment).toBe(false)
  })

  test('preserves year options discovered from point fallbacks after navigating to an empty year', async () => {
    const currentYear = new Date().getFullYear()
    const middleYear = currentYear - 1
    const oldestYear = currentYear - 5
    getDiscoveryTrendMock.mockImplementation((dateRange: DateRange) => {
      const year = Number(dateRange.start.slice(0, 4))
      if (year === currentYear) {
        return Promise.resolve(
          trendFor(
            dateRange,
            [point(`${oldestYear}-04-01`, 9), point('not-a-date-key', 4)],
            [],
          ),
        )
      }

      return Promise.resolve(trendFor(dateRange, [], []))
    })

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'en',
        mode: 'year',
        profileId: null,
        showCurrentYearShortcut: true,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    await flushMicrotasks()
    expect(result.current.yearOptions).toContain(oldestYear)
    expect(result.current.yearOptions).not.toContain(0)

    act(() => {
      result.current.selectYear(middleYear)
    })

    await waitFor(() => expect(result.current.selectedYear).toBe(middleYear))
    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    await flushMicrotasks()

    expect(result.current.yearOptions).toContain(oldestYear)
    expect(result.current.olderYear).toBe(middleYear - 1)
  })

  test('resetting to the current year releases manual selection across a year rollover', async () => {
    vi.setSystemTime(new Date('2026-06-01T12:00:00Z'))
    getDiscoveryTrendMock.mockImplementation((dateRange: DateRange) =>
      Promise.resolve(
        trendFor(
          dateRange,
          [point(`${dateRange.start.slice(0, 4)}-01-02`, 1)],
          [2027, 2026, 2025],
        ),
      ),
    )

    const { rerender, result } = renderHook(
      ({ language }: { language: string }) =>
        useBrowsingRhythmCardState({
          language,
          mode: 'year',
          profileId: null,
          showCurrentYearShortcut: true,
          t,
        }),
      {
        initialProps: {
          language: 'en',
        },
      },
    )

    await flushMicrotasks()
    await waitFor(() => expect(result.current.selectedYear).toBe(2026))

    act(() => {
      result.current.selectYear(2025)
    })
    expect(result.current.selectedYear).toBe(2025)

    act(() => {
      result.current.resetToCurrentYear()
    })
    expect(result.current.selectedYear).toBe(2026)

    vi.setSystemTime(new Date('2027-01-01T12:00:00Z'))
    rerender({ language: 'zh-TW' })
    await flushMicrotasks()

    expect(result.current.selectedYear).toBe(2027)
  })

  test('realigns to a new current calendar year when the selected year remains available', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-06-01T12:00:00Z'))
    getDiscoveryTrendMock.mockImplementation((dateRange: DateRange) =>
      Promise.resolve(
        trendFor(
          dateRange,
          [point(`${dateRange.start.slice(0, 4)}-01-01`, 1)],
          [2027, 2026],
        ),
      ),
    )

    const { rerender, result } = renderHook(
      ({ language }: { language: string }) =>
        useBrowsingRhythmCardState({
          language,
          mode: 'year',
          profileId: null,
          showCurrentYearShortcut: true,
          t,
        }),
      {
        initialProps: {
          language: 'en',
        },
      },
    )

    expect(result.current.selectedYear).toBe(2026)
    await flushMicrotasks()
    expect(result.current.yearOptions).toEqual([2027, 2026])

    vi.setSystemTime(new Date('2027-01-01T12:00:00Z'))
    rerender({ language: 'zh-CN' })
    await flushMicrotasks()

    expect(result.current.selectedYear).toBe(2027)
  })

  test('uses cached trend data and hides visible-range copy for full-year coverage', async () => {
    const currentYear = new Date().getFullYear()
    const dateRange = calendarYearRange(currentYear)
    const cachedTrend = trendFor(
      dateRange,
      [point(`${currentYear}-01-01`, 3), point(`${currentYear}-12-31`, 5)],
      [currentYear],
    )

    peekDiscoveryTrendMock.mockReturnValue(cachedTrend)
    getDiscoveryTrendMock.mockResolvedValue(cachedTrend)

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'zh-TW',
        mode: 'year',
        profileId: 'profile-a',
        showCurrentYearShortcut: true,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))

    expect(peekDiscoveryTrendMock).toHaveBeenCalledWith(
      dateRange,
      'profile-a',
      'day',
    )
    expect(result.current.visibleRangeHint).toBeNull()
    expect(result.current.hasCalendarVisits).toBe(true)
    expect(result.current.visitSummary).toContain('rhythmVisitSummaryYear')
  })

  test('handles null refresh tokens, cancelled queued refreshes, and calendar-year realignment', async () => {
    vi.setSystemTime(new Date('2026-12-31T12:00:00Z'))
    getDiscoveryTrendMock.mockImplementation((dateRange: DateRange) =>
      Promise.resolve(
        trendFor(
          dateRange,
          [point(`${dateRange.start.slice(0, 4)}-01-01`, 1)],
          [Number(dateRange.start.slice(0, 4))],
        ),
      ),
    )

    const { rerender, result, unmount } = renderHook(
      ({
        language,
        refreshToken,
      }: {
        language: string
        refreshToken: number | null
      }) =>
        useBrowsingRhythmCardState({
          language,
          mode: 'year',
          profileId: null,
          refreshToken,
          showCurrentYearShortcut: true,
          t,
        }),
      {
        initialProps: {
          language: 'en',
          refreshToken: 1 as number | null,
        },
      },
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    expect(result.current.selectedYear).toBe(2026)

    rerender({
      language: 'en',
      refreshToken: null,
    })
    await waitFor(() => expect(result.current.trendLoading).toBe(false))

    vi.setSystemTime(new Date('2027-01-01T12:00:00Z'))
    rerender({
      language: 'zh-TW',
      refreshToken: null,
    })
    await waitFor(() => expect(result.current.selectedYear).toBe(2027))

    rerender({
      language: 'zh-TW',
      refreshToken: 2,
    })
    unmount()
  })

  test('clears manual year selection when the backend no longer offers that year', async () => {
    vi.setSystemTime(new Date('2026-01-01T12:00:00Z'))
    const currentRange = calendarYearRange(2026)
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(currentRange, [point('2025-05-01', 4)], [2025]),
    )

    const { result } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'en',
        mode: 'year',
        profileId: null,
        showCurrentYearShortcut: true,
        t,
      }),
    )

    await waitFor(() => expect(result.current.trendLoading).toBe(false))
    await act(async () => {
      await Promise.resolve()
    })

    expect(result.current.canResetToCurrentYear).toBe(false)
  })

  test('ignores queued calendar-year realignment after cleanup', async () => {
    const queuedMicrotasks: VoidFunction[] = []
    vi.stubGlobal('queueMicrotask', (callback: VoidFunction) => {
      queuedMicrotasks.push(callback)
    })
    vi.setSystemTime(new Date('2026-01-01T12:00:00Z'))
    const currentRange = calendarYearRange(2026)
    getDiscoveryTrendMock.mockResolvedValue(
      trendFor(currentRange, [point('2025-05-01', 4)], [2025]),
    )

    const { unmount } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'en',
        mode: 'year',
        profileId: null,
        showCurrentYearShortcut: true,
        t,
      }),
    )

    await waitFor(() => expect(queuedMicrotasks.length).toBeGreaterThan(0))
    unmount()
    act(() => {
      queuedMicrotasks.forEach((callback) => callback())
    })
  })

  test('skips known-year cache updates when the queued microtask runs after cleanup', async () => {
    const queuedMicrotasks: VoidFunction[] = []
    vi.stubGlobal('queueMicrotask', (callback: VoidFunction) => {
      queuedMicrotasks.push(callback)
    })
    const currentYear = new Date().getFullYear()
    const currentRange = calendarYearRange(currentYear)
    const cachedTrend = trendFor(
      currentRange,
      [point(`${currentYear}-01-01`, 3)],
      [currentYear],
    )

    peekDiscoveryTrendMock.mockReturnValue(cachedTrend)
    getDiscoveryTrendMock.mockResolvedValue(cachedTrend)

    const { unmount } = renderHook(() =>
      useBrowsingRhythmCardState({
        language: 'en',
        mode: 'year',
        profileId: null,
        showCurrentYearShortcut: true,
        t,
      }),
    )

    await waitFor(() => expect(queuedMicrotasks.length).toBeGreaterThan(0))
    unmount()

    act(() => {
      queuedMicrotasks.forEach((callback) => callback())
    })
  })
})

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve()
    await Promise.resolve()
    await Promise.resolve()
  })
}

async function drainQueuedMicrotasks(queuedMicrotasks: VoidFunction[]) {
  await act(async () => {
    while (queuedMicrotasks.length > 0) {
      queuedMicrotasks.shift()?.()
      await Promise.resolve()
    }
  })
}
