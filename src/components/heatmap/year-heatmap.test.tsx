import { describe, expect, test, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { YearHeatmap, type YearHeatmapCopy } from './year-heatmap'
import { buildYearHeatmapCells } from './year-heatmap-helpers'

const COPY: YearHeatmapCopy = {
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
  test('renders a button per cell with tooltip + level dataset', () => {
    const cells = buildYearHeatmapCells(
      [
        { dateKey: '2026-05-19', totalVisits: 4 },
        { dateKey: '2026-05-21', totalVisits: 16 },
      ],
      new Date(2026, 4, 19),
      3,
    )
    render(<YearHeatmap cells={cells} copy={COPY} testId="heatmap" />)
    const buttons = screen.getAllByRole('button')
    expect(buttons.length).toBe(3)
    expect(buttons[0]).toHaveAttribute('title', '2026-05-19 · 4')
    expect(buttons[0]).toHaveAttribute('data-level', '1')
    expect(buttons[2]).toHaveAttribute('data-level', '4')
  })

  test('disables zero-count cells so users cannot drill into empty days', () => {
    const cells = buildYearHeatmapCells(
      [{ dateKey: '2026-05-19', totalVisits: 4 }],
      new Date(2026, 4, 19),
      3,
    )
    render(<YearHeatmap cells={cells} copy={COPY} onSelectDate={vi.fn()} />)
    const buttons = screen.getAllByRole('button')
    expect(buttons[0]).not.toBeDisabled() // 2026-05-19, count=4
    expect(buttons[1]).toBeDisabled() // empty
    expect(buttons[2]).toBeDisabled() // empty
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
    await user.click(screen.getAllByRole('button')[0])
    expect(onSelectDate).toHaveBeenCalledWith('2026-05-19', 4)
  })

  test('renders the less / more legend swatches', () => {
    render(<YearHeatmap cells={[]} copy={COPY} />)
    expect(screen.getByText('Less')).toBeInTheDocument()
    expect(screen.getByText('More')).toBeInTheDocument()
  })
})
