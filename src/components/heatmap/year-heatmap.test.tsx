import { describe, expect, test, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { YearHeatmap, type YearHeatmapCopy } from './year-heatmap'
import { buildYearHeatmapCells } from './year-heatmap-helpers'

const COPY: YearHeatmapCopy = {
  ariaLabel: 'Calendar heatmap of daily page visits',
  legendLess: 'Less',
  legendMore: 'More',
  cellTooltip: (date, count) => `${date} · ${count}`,
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

describe('YearHeatmap', () => {
  test('renders a grid cell per day with an accessible name carrying date + level dataset', () => {
    const cells = buildYearHeatmapCells(
      [
        { dateKey: '2026-05-19', totalVisits: 4 },
        { dateKey: '2026-05-21', totalVisits: 16 },
      ],
      new Date(2026, 4, 19),
      3,
    )
    render(<YearHeatmap cells={cells} copy={COPY} testId="heatmap" />)
    const buttons = screen.getAllByRole('gridcell')
    expect(buttons.length).toBe(3)
    expect(buttons[0]).toHaveAccessibleName('2026-05-19 · 4')
    expect(buttons[0]).toHaveAttribute('data-level', '1')
    expect(buttons[2]).toHaveAttribute('data-level', '4')
  })

  test('renders the grid with role=grid and the chart aria-label', () => {
    render(<YearHeatmap cells={[]} copy={COPY} testId="heatmap" />)
    expect(
      screen.getByRole('grid', {
        name: 'Calendar heatmap of daily page visits',
      }),
    ).toBeInTheDocument()
  })

  test('makes zero-count cells non-focusable without flagging them aria-disabled', () => {
    const cells = buildYearHeatmapCells(
      [{ dateKey: '2026-05-19', totalVisits: 4 }],
      new Date(2026, 4, 19),
      3,
    )
    render(<YearHeatmap cells={cells} copy={COPY} onSelectDate={vi.fn()} />)
    const buttons = screen.getAllByRole('gridcell')
    expect(buttons[0]).not.toHaveAttribute('aria-disabled') // 2026-05-19, count=4
    expect(buttons[0]).toHaveAttribute('tabindex', '0')
    expect(buttons[1]).not.toHaveAttribute('aria-disabled') // empty
    expect(buttons[1]).not.toHaveAttribute('tabindex')
    expect(buttons[2]).not.toHaveAttribute('aria-disabled') // empty
    expect(buttons[2]).not.toHaveAttribute('tabindex')
  })

  test('clicking a non-zero cell forwards the date and count', async () => {
    const onSelectDate = vi.fn()
    const cells = buildYearHeatmapCells(
      [{ dateKey: '2026-05-19', totalVisits: 4 }],
      new Date(2026, 4, 19),
      1,
    )
    render(
      <YearHeatmap cells={cells} copy={COPY} onSelectDate={onSelectDate} />,
    )
    const user = userEvent.setup()
    await user.click(screen.getAllByRole('gridcell')[0])
    expect(onSelectDate).toHaveBeenCalledWith('2026-05-19', 4)
  })

  test('activating a non-zero cell via keyboard forwards the date and count', async () => {
    const onSelectDate = vi.fn()
    const cells = buildYearHeatmapCells(
      [{ dateKey: '2026-05-19', totalVisits: 4 }],
      new Date(2026, 4, 19),
      1,
    )
    render(
      <YearHeatmap cells={cells} copy={COPY} onSelectDate={onSelectDate} />,
    )
    const user = userEvent.setup()
    const button = screen.getAllByRole('gridcell')[0]
    button.focus()
    await user.keyboard('{Enter}')
    expect(onSelectDate).toHaveBeenCalledWith('2026-05-19', 4)
  })

  test('renders the less / more legend swatches', () => {
    render(<YearHeatmap cells={[]} copy={COPY} />)
    expect(screen.getByText('Less')).toBeInTheDocument()
    expect(screen.getByText('More')).toBeInTheDocument()
  })
})
