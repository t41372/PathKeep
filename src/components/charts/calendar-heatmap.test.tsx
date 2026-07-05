import { describe, expect, test, vi } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { CalendarHeatmap, type CalendarHeatmapCopy } from './calendar-heatmap'
import type { CalendarHeatmapCell } from './calendar-heatmap-geometry'

const COPY: CalendarHeatmapCopy = {
  ariaLabel: 'Calendar heatmap of daily page visits',
  legendLess: 'Less',
  legendMore: 'More',
  cellAccessibleName: (date, count) => `${count} visits on ${date}`,
  monthLabels: [
    'Jan',
    'Feb',
    'Mar',
    'Apr',
    'May',
    'Jun',
    'Jul',
    'Aug',
    'Sep',
    'Oct',
    'Nov',
    'Dec',
  ],
  dayLabels: ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'],
}

function cells(): CalendarHeatmapCell[] {
  return [
    { date: '2026-05-19', count: 4, level: 1, dayOfWeek: 2 },
    { date: '2026-05-20', count: 0, level: 0, dayOfWeek: 3 },
    { date: '2026-05-21', count: 16, level: 4, dayOfWeek: 4 },
  ]
}

describe('CalendarHeatmap', () => {
  test('renders a grid with an aria-label and the right number of data cells', () => {
    render(<CalendarHeatmap cells={cells()} copy={COPY} testId="heatmap" />)
    const grid = screen.getByRole('grid', {
      name: 'Calendar heatmap of daily page visits',
    })
    expect(grid).toBeInTheDocument()
    const gridCells = within(grid).getAllByRole('gridcell')
    expect(gridCells).toHaveLength(3)
  })

  test('every gridcell is owned by a row, which is owned by the grid (correct ARIA grid structure)', () => {
    render(<CalendarHeatmap cells={cells()} copy={COPY} />)
    const grid = screen.getByRole('grid')
    const rows = within(grid).getAllByRole('row')
    expect(rows.length).toBeGreaterThan(0)
    for (const cell of within(grid).getAllByRole('gridcell')) {
      expect(cell.closest('[role="row"]')).not.toBeNull()
    }
  })

  test('gives each cell an accessible name carrying date + count', () => {
    render(<CalendarHeatmap cells={cells()} copy={COPY} />)
    expect(
      screen.getByRole('gridcell', { name: '4 visits on 2026-05-19' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('gridcell', { name: '0 visits on 2026-05-20' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('gridcell', { name: '16 visits on 2026-05-21' }),
    ).toBeInTheDocument()
  })

  test('gives each cell a native SVG <title> so sighted mouse users see a hover tooltip with the same date + count', () => {
    render(<CalendarHeatmap cells={cells()} copy={COPY} testId="heatmap" />)
    const cell = screen.getByRole('gridcell', {
      name: '4 visits on 2026-05-19',
    })
    expect(cell.querySelector('title')?.textContent).toBe(
      '4 visits on 2026-05-19',
    )
  })

  test('maps each level to its ramp fill class, 0 = neutral, 1-4 = accent ramp', () => {
    const rampCells: CalendarHeatmapCell[] = [
      { date: '2026-01-01', count: 0, level: 0, dayOfWeek: 0 },
      { date: '2026-01-02', count: 1, level: 1, dayOfWeek: 1 },
      { date: '2026-01-03', count: 2, level: 2, dayOfWeek: 2 },
      { date: '2026-01-04', count: 3, level: 3, dayOfWeek: 3 },
      { date: '2026-01-05', count: 4, level: 4, dayOfWeek: 4 },
    ]
    render(<CalendarHeatmap cells={rampCells} copy={COPY} testId="ramp" />)
    const svg = screen.getByTestId('ramp-svg')
    const byDate = (date: string) =>
      svg.querySelector(`[data-date="${date}"]`) as SVGRectElement
    expect(byDate('2026-01-01')).toHaveClass('fill-hover')
    expect(byDate('2026-01-02')).toHaveClass('fill-accent-soft')
    expect(byDate('2026-01-03')).toHaveClass('fill-accent-medium')
    expect(byDate('2026-01-04')).toHaveClass('fill-accent-strong')
    expect(byDate('2026-01-05')).toHaveClass('fill-accent')
  })

  test('makes zero-count cells non-focusable without flagging them aria-disabled — a quiet day is a plain data cell, not a disabled control', () => {
    render(
      <CalendarHeatmap cells={cells()} copy={COPY} onSelectDay={vi.fn()} />,
    )
    const empty = screen.getByRole('gridcell', {
      name: '0 visits on 2026-05-20',
    })
    expect(empty).not.toHaveAttribute('aria-disabled')
    expect(empty).not.toHaveAttribute('tabindex')
    const nonEmpty = screen.getByRole('gridcell', {
      name: '4 visits on 2026-05-19',
    })
    expect(nonEmpty).not.toHaveAttribute('aria-disabled')
    expect(nonEmpty).toHaveAttribute('tabindex', '0')
  })

  test('gives clickable cells a painted stroke-based focus/hover ring, not a CSS outline (WebKit does not paint outline on SVG shapes)', () => {
    render(
      <CalendarHeatmap
        cells={cells()}
        copy={COPY}
        onSelectDay={vi.fn()}
        testId="heatmap"
      />,
    )
    const svg = screen.getByTestId('heatmap-svg')
    const clickableCell = svg.querySelector(
      '[data-date="2026-05-19"]',
    ) as SVGRectElement
    expect(clickableCell.className.baseVal).toMatch(/hover:stroke-ink/)
    expect(clickableCell.className.baseVal).toMatch(/focus-visible:stroke-ink/)
    expect(clickableCell.className.baseVal).not.toMatch(/outline-ink/)
    const nonClickableCell = svg.querySelector(
      '[data-date="2026-05-20"]',
    ) as SVGRectElement
    expect(nonClickableCell.className.baseVal).not.toMatch(/outline-ink/)
  })

  test('clicking a non-zero cell forwards the date and count', async () => {
    const onSelectDay = vi.fn()
    render(
      <CalendarHeatmap cells={cells()} copy={COPY} onSelectDay={onSelectDay} />,
    )
    const user = userEvent.setup()
    await user.click(
      screen.getByRole('gridcell', { name: '4 visits on 2026-05-19' }),
    )
    expect(onSelectDay).toHaveBeenCalledWith('2026-05-19', 4)
  })

  test('activating a non-zero cell via keyboard (Enter) forwards the date and count', async () => {
    const onSelectDay = vi.fn()
    render(
      <CalendarHeatmap cells={cells()} copy={COPY} onSelectDay={onSelectDay} />,
    )
    const user = userEvent.setup()
    const target = screen.getByRole('gridcell', {
      name: '16 visits on 2026-05-21',
    })
    target.focus()
    await user.keyboard('{Enter}')
    expect(onSelectDay).toHaveBeenCalledWith('2026-05-21', 16)
  })

  test('activating a non-zero cell via keyboard (Space) forwards the date and count', async () => {
    const onSelectDay = vi.fn()
    render(
      <CalendarHeatmap cells={cells()} copy={COPY} onSelectDay={onSelectDay} />,
    )
    const user = userEvent.setup()
    const target = screen.getByRole('gridcell', {
      name: '4 visits on 2026-05-19',
    })
    target.focus()
    await user.keyboard(' ')
    expect(onSelectDay).toHaveBeenCalledWith('2026-05-19', 4)
  })

  test('ignores non-activation keys on a focused cell', async () => {
    const onSelectDay = vi.fn()
    render(
      <CalendarHeatmap cells={cells()} copy={COPY} onSelectDay={onSelectDay} />,
    )
    const user = userEvent.setup()
    const target = screen.getByRole('gridcell', {
      name: '4 visits on 2026-05-19',
    })
    target.focus()
    await user.keyboard('{Tab}')
    expect(onSelectDay).not.toHaveBeenCalled()
  })

  test('clicking without onSelectDay is a no-op (no crash)', async () => {
    render(<CalendarHeatmap cells={cells()} copy={COPY} />)
    const user = userEvent.setup()
    // Zero-count cells never get onClick wired regardless of onSelectDay.
    await user.click(
      screen.getByRole('gridcell', { name: '0 visits on 2026-05-20' }),
    )
  })

  test('renders month + weekday labels as text', () => {
    render(<CalendarHeatmap cells={cells()} copy={COPY} testId="labels" />)
    const svg = screen.getByTestId('labels-svg')
    expect(within(svg).getByText('May')).toBeInTheDocument()
    expect(within(svg).getByText('Mon')).toBeInTheDocument()
    expect(within(svg).getByText('Wed')).toBeInTheDocument()
    expect(within(svg).getByText('Fri')).toBeInTheDocument()
    // Sun/Tue/Thu/Sat are intentionally skipped to avoid crowding.
    expect(within(svg).queryByText('Sun')).not.toBeInTheDocument()
  })

  test('renders the less / more legend with opaque ramp swatches', () => {
    render(<CalendarHeatmap cells={[]} copy={COPY} />)
    expect(screen.getByText('Less')).toBeInTheDocument()
    expect(screen.getByText('More')).toBeInTheDocument()
  })

  test('renders an empty grid with no data cells for an empty cell list', () => {
    render(<CalendarHeatmap cells={[]} copy={COPY} />)
    const grid = screen.getByRole('grid')
    expect(within(grid).queryAllByRole('gridcell')).toHaveLength(0)
  })

  test('renders phantom (aria-hidden) placeholders for unfilled week slots', () => {
    // A single cell mid-week leaves 6 other day-of-week slots in that week
    // unfilled; those render as aria-hidden fillers, not accessible cells.
    render(
      <CalendarHeatmap
        cells={[{ date: '2026-05-19', count: 4, level: 1, dayOfWeek: 2 }]}
        copy={COPY}
        testId="phantom"
      />,
    )
    const svg = screen.getByTestId('phantom-svg')
    const hiddenRects = svg.querySelectorAll('rect[aria-hidden="true"]')
    expect(hiddenRects).toHaveLength(6)
  })

  describe('roving-tabindex grid keyboard model', () => {
    /**
     * Two full 7-day weeks. Week 0 (Sun..Sat) has clickable Tue (day2) and
     * Fri (day5), with Wed/Thu (day3/day4) zero-count in between so
     * ArrowDown from Tue must skip two inert cells to reach Fri. Week 1's
     * Tue (day2) is also clickable, directly right of week 0's Tue, so
     * ArrowRight is a direct one-column hop. Everything else is zero-count
     * filler that must never take part in tab order or arrow navigation.
     */
    function twoWeekGrid(): CalendarHeatmapCell[] {
      const week0Counts = [0, 0, 5, 0, 0, 3, 0]
      const week1Counts = [0, 0, 7, 0, 0, 0, 0]
      const dates = [
        '2026-06-07',
        '2026-06-08',
        '2026-06-09',
        '2026-06-10',
        '2026-06-11',
        '2026-06-12',
        '2026-06-13',
        '2026-06-14',
        '2026-06-15',
        '2026-06-16',
        '2026-06-17',
        '2026-06-18',
        '2026-06-19',
        '2026-06-20',
      ]
      const counts = [...week0Counts, ...week1Counts]
      return dates.map((date, index) => ({
        date,
        count: counts[index],
        level: counts[index] > 0 ? 1 : 0,
        dayOfWeek: index % 7,
      }))
    }

    test('gives exactly one clickable cell tabIndex 0 by default (the first clickable in reading order) and never steals DOM focus on mount', () => {
      render(
        <CalendarHeatmap
          cells={twoWeekGrid()}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const svg = screen.getByTestId('rove-svg')
      const tabbable = svg.querySelectorAll('[tabindex="0"]')
      expect(tabbable).toHaveLength(1)
      expect((tabbable[0] as SVGRectElement).dataset.date).toBe('2026-06-09')
      const others = svg.querySelectorAll(
        'rect[role="gridcell"][tabindex="-1"]',
      )
      expect(others).toHaveLength(2)
      expect(document.activeElement).toBe(document.body)
    })

    test('ArrowRight moves the roving tab stop to the nearest clickable cell in the next column', async () => {
      render(
        <CalendarHeatmap
          cells={twoWeekGrid()}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const svg = screen.getByTestId('rove-svg')
      const start = screen.getByRole('gridcell', {
        name: '5 visits on 2026-06-09',
      })
      start.focus()
      const user = userEvent.setup()
      await user.keyboard('{ArrowRight}')
      const next = svg.querySelector('[data-date="2026-06-16"]')
      expect(next).toHaveAttribute('tabindex', '0')
      expect(start).toHaveAttribute('tabindex', '-1')
      expect(document.activeElement).toBe(next)
    })

    test('ArrowDown skips zero-count cells to land on the next clickable cell in the same column', async () => {
      render(
        <CalendarHeatmap
          cells={twoWeekGrid()}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const svg = screen.getByTestId('rove-svg')
      const start = screen.getByRole('gridcell', {
        name: '5 visits on 2026-06-09',
      })
      start.focus()
      const user = userEvent.setup()
      await user.keyboard('{ArrowDown}')
      const next = svg.querySelector('[data-date="2026-06-12"]')
      expect(next).toHaveAttribute('tabindex', '0')
      expect(document.activeElement).toBe(next)
    })

    test('an arrow key toward a direction with no further clickable cell leaves the roving tab stop in place', async () => {
      render(
        <CalendarHeatmap
          cells={twoWeekGrid()}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const start = screen.getByRole('gridcell', {
        name: '5 visits on 2026-06-09',
      })
      start.focus()
      const user = userEvent.setup()
      await user.keyboard('{ArrowUp}')
      expect(start).toHaveAttribute('tabindex', '0')
      await user.keyboard('{ArrowLeft}')
      expect(start).toHaveAttribute('tabindex', '0')
    })

    test('clicking a non-default clickable cell moves the roving tab stop to it', async () => {
      render(
        <CalendarHeatmap
          cells={twoWeekGrid()}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const svg = screen.getByTestId('rove-svg')
      const defaultCell = screen.getByRole('gridcell', {
        name: '5 visits on 2026-06-09',
      })
      const clicked = screen.getByRole('gridcell', {
        name: '3 visits on 2026-06-12',
      })
      const user = userEvent.setup()
      await user.click(clicked)
      expect(clicked).toHaveAttribute('tabindex', '0')
      expect(defaultCell).toHaveAttribute('tabindex', '-1')
      expect(svg.querySelectorAll('[tabindex="0"]')).toHaveLength(1)
    })

    test('when no cell is clickable, no gridcell gets a roving tab stop', () => {
      const allZero: CalendarHeatmapCell[] = twoWeekGrid().map((cell) => ({
        ...cell,
        count: 0,
        level: 0,
      }))
      render(
        <CalendarHeatmap
          cells={allZero}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const svg = screen.getByTestId('rove-svg')
      expect(svg.querySelectorAll('[tabindex="0"]')).toHaveLength(0)
    })

    test('a cells-prop change resets the roving tab stop to the new first clickable cell, without stealing DOM focus, when the previously active cell stops being clickable', () => {
      const { rerender } = render(
        <CalendarHeatmap
          cells={twoWeekGrid()}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const changed = twoWeekGrid().map((cell) =>
        cell.date === '2026-06-09'
          ? { ...cell, count: 0, level: 0 as const }
          : cell,
      )
      rerender(
        <CalendarHeatmap
          cells={changed}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const svg = screen.getByTestId('rove-svg')
      const tabbable = svg.querySelectorAll('[tabindex="0"]')
      expect(tabbable).toHaveLength(1)
      expect((tabbable[0] as SVGRectElement).dataset.date).toBe('2026-06-12')
      expect(document.activeElement).toBe(document.body)
    })

    test('a cells-prop change keeps the roving tab stop on the still-clickable active cell instead of resetting it', async () => {
      const { rerender } = render(
        <CalendarHeatmap
          cells={twoWeekGrid()}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const svg = screen.getByTestId('rove-svg')
      const user = userEvent.setup()
      await user.click(
        screen.getByRole('gridcell', { name: '3 visits on 2026-06-12' }),
      )
      rerender(
        <CalendarHeatmap
          cells={twoWeekGrid()}
          copy={COPY}
          onSelectDay={vi.fn()}
          testId="rove"
        />,
      )
      const stillActive = svg.querySelector('[data-date="2026-06-12"]')
      expect(stillActive).toHaveAttribute('tabindex', '0')
    })

    test('ArrowRight moves the roving stop, and a following Enter activates the newly-focused cell, not the original one', async () => {
      const onSelectDay = vi.fn()
      render(
        <CalendarHeatmap
          cells={twoWeekGrid()}
          copy={COPY}
          onSelectDay={onSelectDay}
          testId="rove"
        />,
      )
      const start = screen.getByRole('gridcell', {
        name: '5 visits on 2026-06-09',
      })
      start.focus()
      const user = userEvent.setup()
      await user.keyboard('{ArrowRight}')
      await user.keyboard('{Enter}')
      expect(onSelectDay).toHaveBeenCalledTimes(1)
      expect(onSelectDay).toHaveBeenCalledWith('2026-06-16', 7)
    })

    describe('reading-order fallback for a cell isolated in both its row and column', () => {
      /**
       * Three full 7-day weeks. Week 0's Sun (day0, 2026-06-07) is the only
       * clickable cell reachable from the grid's edges by a straight-line
       * scan. Week 2's Thu (day4, 2026-06-25) is clickable and isolated: no
       * other clickable cell shares its row (day4) or its column (week2), so
       * `findNextClickableCoord`'s straight-line scan can never reach it
       * from any other cell — only the reading-order fallback can.
       */
      function isolatedCellGrid(): CalendarHeatmapCell[] {
        const dates = [
          '2026-06-07',
          '2026-06-08',
          '2026-06-09',
          '2026-06-10',
          '2026-06-11',
          '2026-06-12',
          '2026-06-13',
          '2026-06-14',
          '2026-06-15',
          '2026-06-16',
          '2026-06-17',
          '2026-06-18',
          '2026-06-19',
          '2026-06-20',
          '2026-06-21',
          '2026-06-22',
          '2026-06-23',
          '2026-06-24',
          '2026-06-25',
          '2026-06-26',
          '2026-06-27',
        ]
        const counts = dates.map((date) =>
          date === '2026-06-07' || date === '2026-06-25' ? 5 : 0,
        )
        return dates.map((date, index) => ({
          date,
          count: counts[index],
          level: counts[index] > 0 ? 1 : 0,
          dayOfWeek: index % 7,
        }))
      }

      test('a straight-line arrow press with no in-line target lands on the isolated cell via reading order', async () => {
        const onSelectDay = vi.fn()
        render(
          <CalendarHeatmap
            cells={isolatedCellGrid()}
            copy={COPY}
            onSelectDay={onSelectDay}
            testId="isolated"
          />,
        )
        const svg = screen.getByTestId('isolated-svg')
        const start = screen.getByRole('gridcell', {
          name: '5 visits on 2026-06-07',
        })
        start.focus()
        const user = userEvent.setup()
        // Straight down week 0's column has nothing (day1..day6 are all
        // zero-count) — only the reading-order fallback reaches 2026-06-25.
        await user.keyboard('{ArrowDown}')
        const isolated = svg.querySelector('[data-date="2026-06-25"]')
        expect(isolated).toHaveAttribute('tabindex', '0')
        expect(start).toHaveAttribute('tabindex', '-1')
        expect(document.activeElement).toBe(isolated)
        await user.keyboard('{Enter}')
        expect(onSelectDay).toHaveBeenCalledWith('2026-06-25', 5)
      })

      test('ArrowUp from the isolated cell falls back to the previous cell in reading order', async () => {
        render(
          <CalendarHeatmap
            cells={isolatedCellGrid()}
            copy={COPY}
            onSelectDay={vi.fn()}
            testId="isolated"
          />,
        )
        const svg = screen.getByTestId('isolated-svg')
        const isolated = screen.getByRole('gridcell', {
          name: '5 visits on 2026-06-25',
        })
        isolated.focus()
        const user = userEvent.setup()
        // Straight up week 2's column and straight left along day4's row are
        // both empty — only the reading-order fallback reaches 2026-06-07.
        await user.keyboard('{ArrowUp}')
        const start = svg.querySelector('[data-date="2026-06-07"]')
        expect(start).toHaveAttribute('tabindex', '0')
        expect(document.activeElement).toBe(start)
      })

      test('a further prev/next past the ends of reading order is a no-op, not a wraparound', async () => {
        render(
          <CalendarHeatmap
            cells={isolatedCellGrid()}
            copy={COPY}
            onSelectDay={vi.fn()}
            testId="isolated"
          />,
        )
        const start = screen.getByRole('gridcell', {
          name: '5 visits on 2026-06-07',
        })
        start.focus()
        const user = userEvent.setup()
        // The first cell in reading order has no "previous" — must stay put,
        // not wrap around to the isolated cell at the end.
        await user.keyboard('{ArrowUp}')
        expect(start).toHaveAttribute('tabindex', '0')
        await user.keyboard('{ArrowLeft}')
        expect(start).toHaveAttribute('tabindex', '0')
      })
    })
  })
})
