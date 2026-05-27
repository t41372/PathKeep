/**
 * @file browsing-rhythm-calendar.test.tsx
 * @description Focused render coverage for browsing-rhythm year controls and calendar grid.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Verify selector-mode year changes and calendar heat-level/day selection behavior.
 *
 * ## Not responsible for
 * - Re-testing date-grid construction or route-level browsing-rhythm data loading.
 *
 * ## Dependencies
 * - Uses the prop-level calendar models produced by browsing-rhythm helpers.
 *
 * ## Performance notes
 * - Pure component fixtures avoid overview fetches and keep heat-level coverage deterministic.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import type { BrowsingRhythmCalendarCell } from './browsing-rhythm-card-helpers'
import {
  BrowsingRhythmCalendarGrid,
  BrowsingRhythmYearControls,
} from './browsing-rhythm-calendar'

const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('BrowsingRhythmCalendar', () => {
  test('select-mode year controls emit numeric year changes', async () => {
    const user = userEvent.setup()
    const onSelectYear = vi.fn()

    render(
      <BrowsingRhythmYearControls
        canResetToCurrentYear={false}
        mode="year"
        newerYear={null}
        olderYear={null}
        onResetToCurrentYear={vi.fn()}
        onSelectYear={onSelectYear}
        selectedYear={2026}
        t={t}
        yearNavigation="select"
        yearOptions={[2026, 2025]}
      />,
    )

    await user.selectOptions(
      screen.getByTestId('browsing-rhythm-year-select'),
      ['2025'],
    )
    expect(onSelectYear).toHaveBeenCalledWith(2025)
  })

  test('button-mode year controls navigate older and newer years', async () => {
    const user = userEvent.setup()
    const onResetToCurrentYear = vi.fn()
    const onSelectYear = vi.fn()

    render(
      <BrowsingRhythmYearControls
        canResetToCurrentYear
        mode="year"
        newerYear={2027}
        olderYear={2025}
        onResetToCurrentYear={onResetToCurrentYear}
        onSelectYear={onSelectYear}
        selectedYear={2026}
        t={t}
        yearNavigation="pager"
        yearOptions={[2027, 2026, 2025]}
      />,
    )

    await user.click(screen.getByTestId('browsing-rhythm-year-previous'))
    await user.click(screen.getByTestId('browsing-rhythm-year-next'))
    await user.click(
      screen.getByRole('button', { name: 'rhythmCurrentYearAction' }),
    )

    expect(onSelectYear).toHaveBeenCalledWith(2025)
    expect(onSelectYear).toHaveBeenCalledWith(2027)
    expect(onResetToCurrentYear).toHaveBeenCalledTimes(1)
  })

  test('button-mode year controls expose disabled boundary years without reset action', () => {
    render(
      <BrowsingRhythmYearControls
        canResetToCurrentYear={false}
        mode="year"
        newerYear={null}
        olderYear={null}
        onResetToCurrentYear={vi.fn()}
        onSelectYear={vi.fn()}
        selectedYear={2026}
        t={t}
        yearNavigation="pager"
        yearOptions={[2026]}
      />,
    )

    expect(
      screen.queryByTestId('browsing-rhythm-current-year-shortcut'),
    ).toBeNull()
    expect(screen.getByTestId('browsing-rhythm-year-previous')).toBeDisabled()
    expect(screen.getByTestId('browsing-rhythm-year-next')).toBeDisabled()
    expect(
      screen.getByLabelText('rhythmPreviousYearAria:{"year":2026}'),
    ).toBeVisible()
    expect(
      screen.getByLabelText('rhythmNextYearAria:{"year":2026}'),
    ).toBeVisible()
  })

  test('calendar grid renders heat levels and emits day selection only for enabled cells', async () => {
    const user = userEvent.setup()
    const onSelectDay = vi.fn()

    render(
      <BrowsingRhythmCalendarGrid
        calendarWeeks={[
          [
            cell('2026-04-01', 0),
            cell('2026-04-02', 1),
            cell('2026-04-03', 3),
            cell('2026-04-04', 5),
            cell('2026-04-05', 8),
            cell('2026-04-06', 10),
            cell('2026-04-07', 2, false),
          ],
        ]}
        maxVisits={10}
        monthLabels={['Apr']}
        onSelectDay={onSelectDay}
        selectedDateKey="2026-04-04"
        t={t}
        weekdayLabels={['S', 'M', 'T', 'W', 'T', 'F', 'S']}
      />,
    )

    const days = screen.getAllByRole('button')
    expect(days.map((day) => day.getAttribute('data-level'))).toEqual([
      '0',
      '1',
      '2',
      '3',
      '4',
      '4',
      '1',
    ])
    expect(days[3]).toHaveClass('rhythm-calendar__day--active')
    expect(days[6]).toBeDisabled()

    await user.click(days[4])
    await user.click(days[6])
    expect(onSelectDay).toHaveBeenCalledTimes(1)
    expect(onSelectDay).toHaveBeenCalledWith('2026-04-05')
  })
})

function cell(
  dateKey: string,
  totalVisits: number,
  inRange = true,
): BrowsingRhythmCalendarCell {
  return {
    date: new Date(`${dateKey}T00:00:00.000Z`),
    dateKey,
    inRange,
    newDomainCount: 1,
    totalVisits,
  }
}
