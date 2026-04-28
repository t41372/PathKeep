/**
 * @file browsing-rhythm-card.test.tsx
 * @description Render-shell coverage for the shared browsing rhythm card.
 * @module components/intelligence
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type { BrowsingRhythmCardState } from './browsing-rhythm-card-state'
import { BrowsingRhythmCard } from './browsing-rhythm-card'

const mockUseBrowsingRhythmCardState = vi.hoisted(() => vi.fn())

vi.mock('./browsing-rhythm-card-state', () => ({
  useBrowsingRhythmCardState: mockUseBrowsingRhythmCardState,
}))

const t = (key: string) => key

describe('BrowsingRhythmCard', () => {
  beforeEach(() => {
    mockUseBrowsingRhythmCardState.mockReset()
    mockUseBrowsingRhythmCardState.mockReturnValue(stateFixture())
  })

  test('renders loading and error states from the shared state hook', () => {
    const { rerender } = renderCard({
      trendLoading: true,
    })

    expect(
      document.querySelector('.browsing-rhythm-card__skeleton'),
    ).toBeTruthy()

    mockUseBrowsingRhythmCardState.mockReturnValue(
      stateFixture({
        trendError: 'trend unavailable',
      }),
    )
    rerender(cardNode())

    expect(screen.getByText('trend unavailable')).toBeVisible()
  })
})

function renderCard(overrides: Partial<BrowsingRhythmCardState> = {}) {
  mockUseBrowsingRhythmCardState.mockReturnValue(stateFixture(overrides))
  return render(cardNode())
}

function cardNode() {
  return (
    <MemoryRouter>
      <BrowsingRhythmCard
        dayHref={(date) => `/intelligence/day/${date}`}
        language="en"
        mode="range"
        t={t}
      />
    </MemoryRouter>
  )
}

function stateFixture(
  overrides: Partial<BrowsingRhythmCardState> = {},
): BrowsingRhythmCardState {
  const cell = {
    date: new Date('2026-04-25T12:00:00Z'),
    dateKey: '2026-04-25',
    inRange: true,
    newDomainCount: 1,
    totalVisits: 3,
  }

  return {
    calendarDays: [cell],
    calendarWeeks: [[cell]],
    canResetToCurrentYear: false,
    hasCalendarVisits: true,
    maxVisits: 3,
    monthLabels: ['Apr'],
    newerYear: null,
    olderYear: null,
    resetToCurrentYear: vi.fn(),
    selectDay: vi.fn(),
    selectYear: vi.fn(),
    selectedDay: null,
    selectedDayDetail: null,
    selectedDayError: null,
    selectedDayLoading: false,
    selectedYear: 2026,
    trendError: null,
    trendLoading: false,
    visibleRangeHint: null,
    visitSummary: '3 visits',
    waitingForYearRealignment: false,
    weekdayLabels: ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'],
    yearOptions: [2026],
    ...overrides,
  }
}
