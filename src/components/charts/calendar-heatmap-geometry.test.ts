import { describe, expect, test } from 'vitest'
import {
  buildCalendarHeatmapLayout,
  CELL_GAP,
  CELL_RADIUS,
  CELL_SIZE,
  CELL_STRIDE,
  GRID_LEFT_PAD,
  GRID_TOP_PAD,
  gridHeight,
  gridWidth,
  type CalendarHeatmapCell,
} from './calendar-heatmap-geometry'

const MONTH_LABELS = [
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
]

function cell(
  date: string,
  dayOfWeek: number,
  count = 0,
  level: CalendarHeatmapCell['level'] = 0,
): CalendarHeatmapCell {
  return { date, count, level, dayOfWeek }
}

describe('constants', () => {
  test('stride is size + gap', () => {
    expect(CELL_STRIDE).toBe(CELL_SIZE + CELL_GAP)
  })

  test('radius mirrors the --radius-tight design token (2px)', () => {
    expect(CELL_RADIUS).toBe(2)
  })
})

describe('gridWidth / gridHeight', () => {
  test('scales width with column count, inset by the left pad', () => {
    expect(gridWidth(3)).toBe(GRID_LEFT_PAD + 3 * CELL_STRIDE)
  })

  test('clamps a negative column count to 0 columns', () => {
    expect(gridWidth(-5)).toBe(GRID_LEFT_PAD)
  })

  test('height is always 7 day-of-week rows plus the top pad', () => {
    expect(gridHeight()).toBe(GRID_TOP_PAD + 7 * CELL_STRIDE)
  })
})

describe('buildCalendarHeatmapLayout', () => {
  test('returns no weeks and no markers for an empty cell list', () => {
    const layout = buildCalendarHeatmapLayout([], MONTH_LABELS)
    expect(layout.weeks).toEqual([])
    expect(layout.monthMarkers).toEqual([])
  })

  test('places a single cell in a one-column week at its day-of-week slot', () => {
    const layout = buildCalendarHeatmapLayout(
      [cell('2026-05-19', 2, 4, 1)],
      MONTH_LABELS,
    )
    expect(layout.weeks).toHaveLength(1)
    expect(layout.weeks[0][2]).toEqual(cell('2026-05-19', 2, 4, 1))
    expect(layout.weeks[0].filter((c) => c !== null)).toHaveLength(1)
  })

  test('starts a new week column once a day-of-week slot repeats', () => {
    const cells = [
      cell('2026-05-19', 2), // Tue, week 0
      cell('2026-05-20', 3), // Wed, week 0
      cell('2026-05-26', 2), // Tue again -> week 0 slot 2 already filled -> week 1
    ]
    const layout = buildCalendarHeatmapLayout(cells, MONTH_LABELS)
    expect(layout.weeks).toHaveLength(2)
    expect(layout.weeks[0][2]?.date).toBe('2026-05-19')
    expect(layout.weeks[0][3]?.date).toBe('2026-05-20')
    expect(layout.weeks[1][2]?.date).toBe('2026-05-26')
  })

  test('fills leftover slots within the same week before starting a new one', () => {
    // Starting mid-week (Wed), the days before Wed (Sun/Mon/Tue) still have
    // open slots in week 0, so they land there rather than starting a new
    // week — this mirrors the original CSS-grid implementation exactly.
    const cells = [
      cell('2026-05-20', 3), // Wed
      cell('2026-05-21', 4), // Thu
      cell('2026-05-24', 0), // Sun (slot 0 open in week 0)
      cell('2026-05-25', 1), // Mon (slot 1 open in week 0)
    ]
    const layout = buildCalendarHeatmapLayout(cells, MONTH_LABELS)
    expect(layout.weeks).toHaveLength(1)
    expect(layout.weeks[0][0]?.date).toBe('2026-05-24')
    expect(layout.weeks[0][1]?.date).toBe('2026-05-25')
    expect(layout.weeks[0][3]?.date).toBe('2026-05-20')
  })

  test('emits one month marker at column 0 for a single-month run', () => {
    const cells = [cell('2026-05-19', 2), cell('2026-05-20', 3)]
    const layout = buildCalendarHeatmapLayout(cells, MONTH_LABELS)
    expect(layout.monthMarkers).toEqual([{ column: 0, label: 'May' }])
  })

  test('emits a new marker on every month transition, tracking the Sunday-incremented column', () => {
    const cells = [
      cell('2026-05-30', 6), // Sat, May, week 0
      cell('2026-05-31', 0), // Sun, May -> column becomes 1
      cell('2026-06-01', 1), // Mon, June -> month changed, marker at column 1
    ]
    const layout = buildCalendarHeatmapLayout(cells, MONTH_LABELS)
    expect(layout.monthMarkers).toEqual([
      { column: 0, label: 'May' },
      { column: 1, label: 'Jun' },
    ])
  })
})
