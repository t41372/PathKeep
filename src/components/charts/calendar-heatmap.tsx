/**
 * @file calendar-heatmap.tsx
 * @description SVG GitHub-style calendar heatmap primitive — a real-date 7×N grid of `<rect>` cells rendered from pre-aggregated data, with a correct ARIA grid/row/gridcell structure, per-cell accessible names, a native hover tooltip, and keyboard activation.
 * @module components/charts
 *
 * ## Responsibilities
 * - Render `cells` (already-aggregated by the caller) as an SVG grid of
 *   `<rect>`s, positioned by `calendar-heatmap-geometry.ts`.
 * - Map each cell's 0-4 `level` onto the semantic accent ramp
 *   (`fill-hover` for the empty level, then `accent-soft` →
 *   `accent-medium` → `accent-strong` → `accent`) — never ad-hoc opacity.
 * - Expose an `onSelectDay(date, count)` callback for non-zero cells,
 *   reachable by mouse click AND keyboard — the same "click a day to
 *   preview it" contract the previous CSS-grid `year-heatmap.tsx` shipped,
 *   now keyboard-operable.
 * - Implement the real ARIA grid keyboard model (WAI-ARIA APG "Grid")
 *   instead of a per-cell tab stop: exactly one clickable cell is a roving
 *   tab stop (`tabIndex=0`, the rest of the clickable cells are
 *   `tabIndex=-1`), and ArrowUp/Down/Left/Right on a focused cell move that
 *   roving focus to the nearest clickable cell in that direction (stepping
 *   over zero-count cells, which never take part in tab order or arrow
 *   navigation — a quiet day is inert, not a focus stop). This keeps a
 *   year view (~365 cells) a single Tab stop to enter/leave, exactly what
 *   `role="grid"` promises, rather than forcing hundreds of Tab presses.
 *   When that straight-line scan finds nothing (the pressed direction runs
 *   off the grid edge), arrow-nav falls back to flattened reading order
 *   (week 0..N, day 0..6) so a clickable cell that is the *only* non-zero
 *   cell in both its row and its column — otherwise unreachable by any
 *   straight-line scan — stays reachable by repeated arrow presses.
 * - Give the whole grid a real `role="grid"` structure: one `role="row"`
 *   group per day-of-week band, each owning `role="gridcell"` data cells
 *   (a `role="grid"` must own `row`s, which must own `gridcell`s — a bare
 *   `role="button"` directly under the grid is a malformed ARIA grid).
 *   Decorative month/weekday `<text>` axis labels are `aria-hidden` so they
 *   don't confuse that structure or a grid-navigation screen reader.
 * - Give every data cell both an `aria-label` (via
 *   `copy.cellAccessibleName`) for assistive tech AND a nested SVG
 *   `<title>` with the same string, so sighted mouse users still get a
 *   native hover tooltip showing the date + visit count (`aria-label`
 *   alone produces no visual tooltip).
 * - Render month + weekday `<text>` labels and a "less…more" legend using
 *   the same opaque ramp tokens as the cells (guardrails §6: legend
 *   swatches must be real, opaque tokens — never nearbackground opacity
 *   tricks).
 *
 * ## Not responsible for
 * - Aggregating or bucketing visit counts — `cells` must already be
 *   aggregated (e.g. by `buildYearHeatmapCells` in a caller's `useMemo`).
 *   This component only maps the given array to geometry.
 * - Localizing copy — callers supply fully-resolved strings/formatters via
 *   `copy`.
 *
 * ## Dependencies
 * - `./calendar-heatmap-geometry` for the pure week-bucketing / month-marker
 *   / grid-constant math.
 * - Paper tokens via Tailwind utilities only (`fill-accent-soft` etc.) —
 *   see `docs/design/design-tokens.md`.
 *
 * ## Performance notes
 * - Renders at most a few thousand cells (a multi-year window); no
 *   virtualization needed at that size. The one non-trivial computation
 *   (`buildCalendarHeatmapLayout`) is `useMemo`d on `[cells, monthLabels]`
 *   so unrelated re-renders (e.g. a parent's unrelated state change) skip
 *   it entirely.
 */

import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react'
import { cn } from '@/lib/cn'
import {
  buildCalendarHeatmapLayout,
  CELL_RADIUS,
  CELL_SIZE,
  CELL_STRIDE,
  GRID_LEFT_PAD,
  GRID_TOP_PAD,
  gridHeight,
  gridWidth,
  type CalendarHeatmapCell,
  type CalendarHeatmapLayout,
} from './calendar-heatmap-geometry'

export type { CalendarHeatmapCell }

export interface CalendarHeatmapCopy {
  /** Describes the whole chart for screen readers, e.g. "Calendar heatmap of daily page visits over the past year". */
  ariaLabel: string
  legendLess: string
  legendMore: string
  /** Accessible name (and hover title) for a data cell — must carry both the date and the visit count. */
  cellAccessibleName: (date: string, count: number) => string
  /** Month labels, ordered Jan…Dec in the active locale. */
  monthLabels: readonly string[]
  /** Three-letter day labels for Sunday..Saturday. */
  dayLabels: readonly string[]
}

export interface CalendarHeatmapProps {
  cells: CalendarHeatmapCell[]
  copy: CalendarHeatmapCopy
  /** Called with (date, count) when a non-zero-visit cell is activated by click or keyboard. */
  onSelectDay?: (date: string, count: number) => void
  testId?: string
}

const LEVEL_FILL_CLASS: Record<CalendarHeatmapCell['level'], string> = {
  0: 'fill-hover',
  1: 'fill-accent-soft',
  2: 'fill-accent-medium',
  3: 'fill-accent-strong',
  4: 'fill-accent',
}

const LEVEL_SWATCH_CLASS: Record<CalendarHeatmapCell['level'], string> = {
  0: 'bg-hover',
  1: 'bg-accent-soft',
  2: 'bg-accent-medium',
  3: 'bg-accent-strong',
  4: 'bg-accent',
}

const LEVELS: CalendarHeatmapCell['level'][] = [0, 1, 2, 3, 4]

/** 0=Sun..6=Sat — one `role="row"` band per day-of-week, matching the fixed 7-row grid `gridHeight()` reserves. */
const DAYS_OF_WEEK = [0, 1, 2, 3, 4, 5, 6] as const

/** A cell's position in the `weeks` layout: `week` is the column index, `day` the 0=Sun..6=Sat row index. */
interface Coord {
  week: number
  day: number
}

const ARROW_STEP: Record<
  'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight',
  Coord
> = {
  ArrowUp: { week: 0, day: -1 },
  ArrowDown: { week: 0, day: 1 },
  ArrowLeft: { week: -1, day: 0 },
  ArrowRight: { week: 1, day: 0 },
}

function isArrowKey(
  key: string,
): key is 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight' {
  return (
    key === 'ArrowUp' ||
    key === 'ArrowDown' ||
    key === 'ArrowLeft' ||
    key === 'ArrowRight'
  )
}

/** A cell is part of the roving-tabindex/arrow-nav set only if it has a non-zero visit count — a quiet day is inert, never a focus stop. */
function isClickableCoord(
  weeks: CalendarHeatmapLayout['weeks'],
  coord: Coord,
): boolean {
  const cell = weeks[coord.week]?.[coord.day]
  return cell !== null && cell !== undefined && cell.count > 0
}

/** Finds the nearest clickable cell from `from` stepping in `key`'s direction, skipping zero-count/phantom cells; `null` if the edge is reached first. */
function findNextClickableCoord(
  weeks: CalendarHeatmapLayout['weeks'],
  from: Coord,
  key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight',
): Coord | null {
  const step = ARROW_STEP[key]
  let cursor: Coord = { week: from.week + step.week, day: from.day + step.day }
  while (
    cursor.week >= 0 &&
    cursor.week < weeks.length &&
    cursor.day >= 0 &&
    cursor.day < 7
  ) {
    if (isClickableCoord(weeks, cursor)) return cursor
    cursor = { week: cursor.week + step.week, day: cursor.day + step.day }
  }
  return null
}

/** All clickable cells in reading order (week 0..N, day 0..6) — the flattened order `findFirstClickableCoord` and the reading-order arrow-nav fallback below both walk. */
function flattenClickableCoords(
  weeks: CalendarHeatmapLayout['weeks'],
): Coord[] {
  const coords: Coord[] = []
  for (let week = 0; week < weeks.length; week += 1) {
    for (let day = 0; day < 7; day += 1) {
      const coord: Coord = { week, day }
      if (isClickableCoord(weeks, coord)) coords.push(coord)
    }
  }
  return coords
}

/** First clickable cell in reading order (week 0..N, day 0..6) — the roving tab stop's default before any arrow-key navigation happens. */
function findFirstClickableCoord(
  weeks: CalendarHeatmapLayout['weeks'],
): Coord | null {
  return flattenClickableCoords(weeks)[0] ?? null
}

/**
 * Reading-order fallback for arrow navigation, used when
 * `findNextClickableCoord`'s straight-line scan returns `null`. A
 * straight-line scan can never reach a clickable cell that is the *only*
 * non-zero cell in both its row (day-of-week) and its column (week) — no
 * direction from any other cell passes through it — so without this
 * fallback that cell would be permanently unreachable by keyboard,
 * breaking the "every clickable cell is keyboard-operable" contract.
 *
 * Falls back to flattened reading order (week 0..N, day 0..6):
 * ArrowRight/ArrowDown move to the next clickable cell after `from` in that
 * order, ArrowLeft/ArrowUp to the previous one. This guarantees every
 * clickable cell is reachable by repeated arrow presses. Mirrors the
 * straight-line scan's own edge behavior — at the first/last clickable
 * cell in reading order, a further prev/next has no target and returns
 * `null` (a no-op; never wraps around).
 */
function findReadingOrderFallback(
  weeks: CalendarHeatmapLayout['weeks'],
  from: Coord,
  key: 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight',
): Coord | null {
  const ordered = flattenClickableCoords(weeks)
  const index = ordered.findIndex(
    (coord) => coord.week === from.week && coord.day === from.day,
  )
  /* v8 ignore next -- `from` is always the coord the roving focus is
   * currently on, which is only ever set to a coord `isClickableCoord`
   * already confirmed, so it is always present in `ordered`. */
  if (index === -1) return null
  const forward = key === 'ArrowDown' || key === 'ArrowRight'
  return (forward ? ordered[index + 1] : ordered[index - 1]) ?? null
}

/** `a` is only ever the derived `activeCoord`, compared against a real cell's own `b` coord from the `clickable && …` check at the call site — `clickable` being true guarantees at least one clickable cell exists, so `activeCoord` can never be null there. */
function coordsEqual(a: Coord | null, b: Coord): boolean {
  /* v8 ignore next -- see doc comment: unreachable given the caller's invariant. */
  if (a === null) return false
  return a.week === b.week && a.day === b.day
}

export function CalendarHeatmap({
  cells,
  copy,
  onSelectDay,
  testId,
}: CalendarHeatmapProps) {
  const layout = useMemo(
    () => buildCalendarHeatmapLayout(cells, copy.monthLabels),
    [cells, copy.monthLabels],
  )
  const width = gridWidth(layout.weeks.length)
  const height = gridHeight()

  const svgRef = useRef<SVGSVGElement>(null)
  // Roving tabindex (WAI-ARIA APG "Grid"): exactly one clickable cell is a
  // tab stop at a time; arrow keys move it. This only tracks the
  // *last-explicitly-chosen* coordinate — `activeCoord` below derives the
  // coordinate actually used for rendering, falling back to the first
  // clickable cell whenever the explicit choice goes stale (e.g. the
  // cells prop changes and that day is no longer in range/non-zero), with
  // no state write (and so no cascading render) required to do it.
  const [explicitActiveCoord, setExplicitActiveCoord] = useState<Coord | null>(
    null,
  )
  const activeCoord = useMemo<Coord | null>(() => {
    if (
      explicitActiveCoord &&
      isClickableCoord(layout.weeks, explicitActiveCoord)
    ) {
      return explicitActiveCoord
    }
    return findFirstClickableCoord(layout.weeks)
  }, [explicitActiveCoord, layout])

  // Only imperatively move DOM focus after a keyboard-driven step — never on
  // the initial render or a cells-prop-driven reset, so mounting/updating
  // the chart never steals focus. `handleKeyDown` sets this just before
  // calling `setExplicitActiveCoord`; the effect below consumes it on the
  // very next render (the one where `activeCoord` reflects the new coord)
  // and clears it, so an unrelated later re-render never re-triggers it.
  const focusPendingRef = useRef(false)

  useEffect(() => {
    if (!focusPendingRef.current) return
    focusPendingRef.current = false
    // focusPendingRef is only set immediately before moving
    // `explicitActiveCoord` to a coordinate `findNextClickableCoord` already
    // confirmed clickable, so `activeCoord` and its cell lookup are never
    // null/undefined by the time this effect runs.
    /* v8 ignore next -- see comment above: unreachable given that invariant. */
    if (!activeCoord) return
    const cell = layout.weeks[activeCoord.week]?.[activeCoord.day]
    /* v8 ignore next -- see comment above: unreachable given that invariant. */
    if (!cell) return
    const node = svgRef.current?.querySelector<SVGRectElement>(
      `[data-date="${cell.date}"]`,
    )
    node?.focus()
  }, [activeCoord, layout])

  function activate(date: string, count: number) {
    onSelectDay?.(date, count)
  }

  function handleKeyDown(
    event: KeyboardEvent<SVGRectElement>,
    coord: Coord,
    date: string,
    count: number,
  ) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      activate(date, count)
      return
    }
    if (isArrowKey(event.key)) {
      event.preventDefault()
      const next =
        findNextClickableCoord(layout.weeks, coord, event.key) ??
        findReadingOrderFallback(layout.weeks, coord, event.key)
      if (next) {
        focusPendingRef.current = true
        setExplicitActiveCoord(next)
      }
    }
  }

  return (
    <div className="flex flex-col gap-2" data-testid={testId}>
      <svg
        ref={svgRef}
        role="grid"
        aria-label={copy.ariaLabel}
        viewBox={`0 0 ${width} ${height}`}
        width="100%"
        className="block overflow-visible"
        data-testid={testId ? `${testId}-svg` : undefined}
      >
        {layout.monthMarkers.map((marker, index) => (
          <text
            key={`month-${index}-${marker.column}`}
            x={GRID_LEFT_PAD + marker.column * CELL_STRIDE}
            y={GRID_TOP_PAD - 4}
            aria-hidden="true"
            className="fill-ink-faint font-mono text-[9px] tracking-[0.04em]"
          >
            {marker.label}
          </text>
        ))}
        {copy.dayLabels.map((label, dayOfWeek) =>
          dayOfWeek === 1 || dayOfWeek === 3 || dayOfWeek === 5 ? (
            <text
              key={`day-${dayOfWeek}`}
              x={GRID_LEFT_PAD - 6}
              y={GRID_TOP_PAD + dayOfWeek * CELL_STRIDE + CELL_SIZE - 2}
              textAnchor="end"
              aria-hidden="true"
              className="fill-ink-faint font-mono text-[9px]"
            >
              {label}
            </text>
          ) : null,
        )}
        {DAYS_OF_WEEK.map((dayOfWeek) => (
          <g key={`row-${dayOfWeek}`} role="row">
            {layout.weeks.map((week, weekIndex) => {
              const dayCell = week[dayOfWeek]
              const x = GRID_LEFT_PAD + weekIndex * CELL_STRIDE
              const y = GRID_TOP_PAD + dayOfWeek * CELL_STRIDE
              if (!dayCell) {
                return (
                  <rect
                    key={`empty-${weekIndex}-${dayOfWeek}`}
                    x={x}
                    y={y}
                    width={CELL_SIZE}
                    height={CELL_SIZE}
                    rx={CELL_RADIUS}
                    className="fill-transparent"
                    aria-hidden="true"
                  />
                )
              }
              const clickable = onSelectDay !== undefined && dayCell.count > 0
              const coord: Coord = { week: weekIndex, day: dayOfWeek }
              const isActiveCell = clickable && coordsEqual(activeCoord, coord)
              const accessibleName = copy.cellAccessibleName(
                dayCell.date,
                dayCell.count,
              )
              return (
                <rect
                  key={dayCell.date}
                  x={x}
                  y={y}
                  width={CELL_SIZE}
                  height={CELL_SIZE}
                  rx={CELL_RADIUS}
                  role="gridcell"
                  aria-label={accessibleName}
                  data-date={dayCell.date}
                  data-level={dayCell.level}
                  tabIndex={clickable ? (isActiveCell ? 0 : -1) : undefined}
                  onClick={
                    clickable
                      ? () => {
                          // A mouse click also moves DOM focus to this cell
                          // (it carries a tabIndex); keep the roving tab
                          // stop in sync so a later Tab-away/Tab-back lands
                          // here instead of snapping back to the old one.
                          setExplicitActiveCoord(coord)
                          activate(dayCell.date, dayCell.count)
                        }
                      : undefined
                  }
                  onKeyDown={
                    clickable
                      ? (event) =>
                          handleKeyDown(
                            event,
                            coord,
                            dayCell.date,
                            dayCell.count,
                          )
                      : undefined
                  }
                  className={cn(
                    'stroke-border-light transition-[stroke,stroke-width] duration-100',
                    LEVEL_FILL_CLASS[dayCell.level],
                    clickable
                      ? 'cursor-pointer hover:stroke-ink focus:outline-none focus-visible:stroke-ink focus-visible:[stroke-width:1.5px]'
                      : 'cursor-default',
                  )}
                  strokeWidth={0.75}
                >
                  <title>{accessibleName}</title>
                </rect>
              )
            })}
          </g>
        ))}
      </svg>
      <div className="flex items-center justify-end gap-2 font-mono text-[9.5px] text-ink-faint">
        <span>{copy.legendLess}</span>
        <span className="flex items-center gap-[2px]">
          {LEVELS.map((level) => (
            <span
              key={level}
              className={cn(
                'h-[10px] w-[10px] rounded-tight border border-border-light',
                LEVEL_SWATCH_CLASS[level],
              )}
              aria-hidden="true"
            />
          ))}
        </span>
        <span>{copy.legendMore}</span>
      </div>
    </div>
  )
}
