/**
 * Tests for PaperCalendarPopover — the month-grid + year-picker that hangs
 * off the day-nav pill.
 *
 * Why this file exists:
 * - Calendar UIs concentrate a lot of subtle behaviour (Monday-first leading
 *   blanks, future-day guard, today/selected ring, density tinting, hover
 *   preview, year picker round-trip). A regression here lands as a visible
 *   navigation bug, so the tests cover each branch explicitly.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperCalendarPopover,
  type PaperCalendarPopoverBounds,
  type PaperCalendarPopoverCopy,
} from './paper-calendar-popover'

const COPY: PaperCalendarPopoverCopy = {
  prevMonth: 'Previous month',
  nextMonth: 'Next month',
  months: [
    'January',
    'February',
    'March',
    'April',
    'May',
    'June',
    'July',
    'August',
    'September',
    'October',
    'November',
    'December',
  ],
  dowLabels: ['M', 'T', 'W', 'T', 'F', 'S', 'S'],
  today: 'Today',
  oneYearAgo: '1 year ago',
  pagesArchived: '{count} pages archived',
  monthSummary: '{active} active days · {total} pages',
  boundsMeta: '{firstYear}–{lastYear} · {totalDays} days',
}

const BOUNDS: PaperCalendarPopoverBounds = {
  firstIso: '1966-01-01',
  lastIso: '2026-05-17',
  firstYear: 1966,
  lastYear: 2026,
  totalDays: 21_900,
}

function baseProps(
  overrides: Partial<Parameters<typeof PaperCalendarPopover>[0]> = {},
): Parameters<typeof PaperCalendarPopover>[0] {
  const densityByDate = new Map<string, number>([
    ['2026-05-01', 12],
    ['2026-05-10', 80],
    ['2026-05-15', 220],
    ['2026-05-16', 1234],
    ['2026-05-17', 0],
  ])
  const densityByYear = new Map<number, number>([
    [2026, 200_000],
    [2025, 80_000],
    [2024, 10_000],
  ])
  const loadedDates = new Set<string>(['2026-05-16', '2026-05-15'])
  return {
    value: '2026-05-16',
    todayIso: '2026-05-17',
    densityByDate,
    densityByYear,
    loadedDates,
    bounds: BOUNDS,
    peakDailyCount: 1500,
    onSelect: vi.fn(),
    copy: COPY,
    testId: 'cal',
    ...overrides,
  }
}

describe('PaperCalendarPopover', () => {
  test('renders the active month, day-of-week labels, and the bounds meta', () => {
    render(<PaperCalendarPopover {...baseProps()} />)

    expect(screen.getByText('May')).toBeVisible()
    expect(screen.getByText('2026')).toBeVisible()
    expect(screen.getByText(/1966–2026/)).toBeVisible()
    // 7 DOW labels rendered exactly once each (with duplicates for T and S).
    const dows = ['M', 'T', 'W', 'T', 'F', 'S', 'S']
    dows.forEach((label) => {
      expect(screen.getAllByText(label).length).toBeGreaterThan(0)
    })
  })

  test('marks selected, today, and loaded cells with data-attributes', () => {
    const { container } = render(<PaperCalendarPopover {...baseProps()} />)

    const selected = container.querySelector('[data-iso="2026-05-16"]')
    expect(selected?.getAttribute('data-selected')).toBe('true')
    expect(selected?.getAttribute('data-loaded')).toBe('true')

    const today = container.querySelector('[data-iso="2026-05-17"]')
    expect(today?.getAttribute('data-today')).toBe('true')
  })

  test('disables future days inside the visible month', () => {
    const { container } = render(
      <PaperCalendarPopover
        {...baseProps({ todayIso: '2026-05-10', value: '2026-05-10' })}
      />,
    )
    const future = container.querySelector<HTMLButtonElement>(
      '[data-iso="2026-05-20"]',
    )
    expect(future).not.toBeNull()
    expect(future?.disabled).toBe(true)
  })

  test('prev / next month buttons walk the grid, clamped to bounds', () => {
    render(<PaperCalendarPopover {...baseProps()} />)

    fireEvent.click(screen.getByRole('button', { name: 'Previous month' }))
    expect(screen.getByText('April')).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: 'Next month' }))
    fireEvent.click(screen.getByRole('button', { name: 'Next month' }))
    expect(screen.getByText('June')).toBeVisible()
  })

  test('clamps month stepping at the archive bounds', () => {
    const lowerView = render(
      <PaperCalendarPopover {...baseProps({ value: '1966-01-15' })} />,
    )
    expect(screen.getByText('January')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: 'Previous month' }))
    expect(screen.getByText('January')).toBeVisible()
    lowerView.unmount()

    render(<PaperCalendarPopover {...baseProps({ value: '2026-12-01' })} />)
    expect(screen.getByText('December')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: 'Next month' }))
    expect(screen.getByText('December')).toBeVisible()
  })

  test('clicking a day calls onSelect with its ISO string', () => {
    const onSelect = vi.fn()
    const { container } = render(
      <PaperCalendarPopover {...baseProps({ onSelect })} />,
    )
    const cell = container.querySelector(
      '[data-iso="2026-05-10"]',
    ) as HTMLElement
    fireEvent.click(cell)
    expect(onSelect).toHaveBeenCalledWith('2026-05-10')
  })

  test('hover over a cell swaps the preview label and sub-line', () => {
    const { container } = render(<PaperCalendarPopover {...baseProps()} />)

    const cell = container.querySelector(
      '[data-iso="2026-05-15"]',
    ) as HTMLElement
    fireEvent.mouseEnter(cell)
    expect(screen.getByText('2026-05-15')).toBeVisible()
    expect(screen.getByText('220 pages archived')).toBeVisible()

    fireEvent.mouseLeave(cell)
    // Reverts to the month summary.
    expect(screen.getByText(/active days · [\d,]+ pages/)).toBeVisible()
  })

  test('opens the year picker and selecting a year jumps the grid', () => {
    render(<PaperCalendarPopover {...baseProps()} />)

    // The month/year title doubles as the year-picker toggle.
    fireEvent.click(
      screen.getByRole('button', { expanded: false, name: /May 2026/ }),
    )
    // Year picker rows.
    const picker = screen.getByRole('button', { name: /2024/ })
    fireEvent.click(picker)
    // Back to the grid, now showing 2024.
    expect(screen.getByText('2024')).toBeVisible()
  })

  test('footer Today button calls onSelect with todayIso', () => {
    const onSelect = vi.fn()
    render(<PaperCalendarPopover {...baseProps({ onSelect })} />)

    fireEvent.click(screen.getByRole('button', { name: /Today/ }))
    expect(onSelect).toHaveBeenCalledWith('2026-05-17')
  })

  test('footer "1 year ago" steps the selection by 365 days', () => {
    const onSelect = vi.fn()
    render(<PaperCalendarPopover {...baseProps({ onSelect })} />)

    fireEvent.click(screen.getByRole('button', { name: /1 year ago/ }))
    expect(onSelect).toHaveBeenCalledWith('2025-05-16')
  })

  test('falls back to the archive boundary when value is malformed', () => {
    render(<PaperCalendarPopover {...baseProps({ value: 'garbage' })} />)
    // Falls back to viewing the bounds.lastIso month (May 2026).
    expect(screen.getByText('May')).toBeVisible()
  })

  test('stops propagation so calendar clicks do not bubble to the toolbar', () => {
    const onParentClick = vi.fn()
    render(
      <div onClick={onParentClick}>
        <PaperCalendarPopover {...baseProps()} />
      </div>,
    )

    fireEvent.click(screen.getByTestId('cal'))
    expect(onParentClick).not.toHaveBeenCalled()
  })

  test('year picker shows the current year with an active marker', () => {
    render(<PaperCalendarPopover {...baseProps()} />)
    fireEvent.click(
      screen.getByRole('button', { expanded: false, name: /May 2026/ }),
    )
    // 2026 row has data-current
    const yearRow = screen
      .getByTestId('cal')
      .querySelector('[data-current="true"]')
    expect(yearRow).not.toBeNull()
    expect(within(yearRow as HTMLElement).getByText('2026')).toBeVisible()
  })

  test('year picker renders all the count-label format branches (zero / raw / 1.5k / 10k)', () => {
    // We need years exercising:
    //   count === 0          → renders '—'
    //   1 ≤ count < 1000     → toLocaleString (e.g. "850")
    //   1000 ≤ count < 10000 → "1.5k" via toFixed(1)
    //   count ≥ 10000        → "12k" via toFixed(0)
    // And density tiers 1/2/3 (not just 4) so opacityForTier walks every arm.
    const densityByYear = new Map<number, number>([
      [2026, 12_000], // ≥10k → "12k", tier 4
      [2025, 1_500], // 1k–10k → "1.5k", tier 4 (capped at 500)
      [2024, 200], // raw "200", tier 3
      [2023, 80], // raw "80", tier 2
      [2022, 5], // raw "5", tier 1
      [2021, 0], // "—", tier 0
    ])
    render(
      <PaperCalendarPopover
        {...baseProps({
          densityByYear,
          bounds: { ...BOUNDS, firstYear: 2021, lastYear: 2026 },
        })}
      />,
    )
    fireEvent.click(
      screen.getByRole('button', { expanded: false, name: /May 2026/ }),
    )
    expect(screen.getByText('12k')).toBeVisible()
    expect(screen.getByText('1.5k')).toBeVisible()
    expect(screen.getByText('200')).toBeVisible()
    expect(screen.getByText('80')).toBeVisible()
    expect(screen.getByText('5')).toBeVisible()
    expect(screen.getByText('—')).toBeVisible()
  })

  test('1 year ago footer falls back to bounds.lastIso when value is empty', () => {
    const onSelect = vi.fn()
    render(<PaperCalendarPopover {...baseProps({ value: '', onSelect })} />)
    fireEvent.click(screen.getByRole('button', { name: /1 year ago/ }))
    expect(onSelect).toHaveBeenCalledWith('2025-05-17')
  })
})
