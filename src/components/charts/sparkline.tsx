/**
 * @file sparkline.tsx
 * @description Reusable SVG line+area sparkline primitive built on `chart-geometry.ts`.
 * @module components/charts
 *
 * ## Responsibilities
 * - Render an ordered numeric series as a filled-area line chart: a
 *   low-opacity `accent` area under a ~1.5px `accent` stroke line.
 * - Expose `role="img"` + a caller-supplied `aria-label` (charts are
 *   decorative-but-meaningful SVG, not interactive controls here), plus an
 *   optional `description` rendered as a nested `<title>` for a
 *   supplementary detail (e.g. a computed mean) distinct from the name.
 * - Optionally render `font-mono` tick labels under specific series
 *   indices (e.g. hour-of-day ticks under a 24-point series).
 * - Optionally render a small, general set of adornments in the series'
 *   own coordinate space — dot `markers` at specific indices, vertical
 *   `gridlines` at specific indices, and a dashed horizontal
 *   `referenceValue` line — plus a `children` render-prop escape hatch that
 *   receives the same computed points/scales for anything the built-in
 *   adornments don't cover, so call sites never need to re-hand-roll the
 *   base `<svg>`/line/area themselves.
 * - Degrade gracefully for 0 or 1 points: still render the `<svg>` shell
 *   (so the surrounding layout doesn't jump) but skip the line/area, since
 *   neither is meaningful with fewer than 2 points.
 *
 * ## Not responsible for
 * - Aggregating the series — callers pass already-computed values, and
 *   already-computed marker/gridline index lists.
 *
 * ## Dependencies
 * - `./chart-geometry` for the scale + path-building math.
 *
 * ## Performance notes
 * - O(values.length + markers.length + gridlines.length); sparklines render
 *   tens to low-hundreds of points, well within a single render pass.
 */

import { useMemo, type ReactNode } from 'react'
import { cn } from '@/lib/cn'
import {
  buildAreaPath,
  buildPolylinePoints,
  indexScale,
  scaleSeriesToPoints,
  seriesValueScale,
  type Point,
} from './chart-geometry'

export interface SparklineTick {
  /** Index into `values` this tick labels. */
  index: number
  label: string
}

export interface SparklineMarker {
  /** Index into `values` this marker is drawn over. */
  index: number
  /** Marker radius in user units. Defaults to 2. */
  radius?: number
}

export interface SparklineGridline {
  /** Index into `values` this gridline aligns under (via the tick x-scale). */
  index: number
}

export interface SparklineRenderArgs {
  /** The series mapped to plot-space points — the same ones the line/area use. */
  points: Point[]
  /** Index → x scale (matches tick/gridline/marker placement). */
  xScale: (index: number) => number
  /** Value → y scale (matches the line/area's own domain). */
  yScale: (value: number) => number
  width: number
  height: number
  padding: number
}

export interface SparklineProps {
  values: number[]
  /** SVG viewBox width, in user units. Defaults to 200. */
  width?: number
  /** SVG viewBox height (excludes tick label space), in user units. Defaults to 48. */
  height?: number
  /** Inset applied on all sides of the plotted area. Defaults to 4. Overridden per-axis by `paddingX`/`paddingY`. */
  padding?: number
  /** Inset applied on the left/right sides only. Defaults to `padding`. */
  paddingX?: number
  /** Inset applied on the top/bottom sides only. Defaults to `padding`. */
  paddingY?: number
  /**
   * Opt-in floor on the value domain's max, even if every value in
   * `values` is smaller (and even if the series is all positive). Use it
   * to keep a low-magnitude series (e.g. a percentage that rarely rises
   * above a point or two) visually flat near the baseline instead of
   * auto-scaling its tiny peak to full height. Omitted or 0 leaves the
   * default behavior (an all-zero series still floors to a degenerate
   * domain of 1, purely to avoid NaN) untouched.
   */
  minDomainMax?: number
  /** Required — describes the chart for screen readers (e.g. "24-hour activity"). */
  ariaLabel: string
  /** Optional supplementary detail (e.g. "Mean: 12%") rendered as a nested `<title>`. */
  description?: string
  /** Optional tick labels rendered under specific series indices. */
  ticks?: SparklineTick[]
  /** Optional dot markers drawn over the line at specific series indices. */
  markers?: SparklineMarker[]
  /** Optional vertical gridlines drawn behind the line at specific series indices. */
  gridlines?: SparklineGridline[]
  /** Optional dashed horizontal reference line at a value in the same domain as `values`. */
  referenceValue?: number
  /**
   * Escape hatch for adornments `markers`/`gridlines`/`referenceValue` don't
   * cover: receives the same points + scales the base line/area use, so a
   * caller can draw custom marks in the identical coordinate space instead
   * of re-hand-rolling the `<svg>` shell.
   */
  children?: (args: SparklineRenderArgs) => ReactNode
  testId?: string
  className?: string
}

const DEFAULT_WIDTH = 200
const DEFAULT_HEIGHT = 48
const DEFAULT_PADDING = 4
const DEFAULT_MARKER_RADIUS = 2
/** Extra vertical space reserved under the plot for tick labels. */
const TICK_LABEL_SPACE = 12

export function Sparkline({
  values,
  width = DEFAULT_WIDTH,
  height = DEFAULT_HEIGHT,
  padding = DEFAULT_PADDING,
  paddingX = padding,
  paddingY = padding,
  minDomainMax,
  ariaLabel,
  description,
  ticks,
  markers,
  gridlines,
  referenceValue,
  children,
  testId,
  className,
}: SparklineProps) {
  const points = useMemo(
    () =>
      scaleSeriesToPoints(values, {
        width,
        height,
        paddingX,
        paddingY,
        minDomainMax,
      }),
    [values, width, height, paddingX, paddingY, minDomainMax],
  )
  const hasLine = points.length >= 2
  const linePoints = useMemo(
    () => (hasLine ? buildPolylinePoints(points) : ''),
    [hasLine, points],
  )
  const areaPath = useMemo(
    () => (hasLine ? buildAreaPath(points, height - paddingY) : ''),
    [hasLine, points, height, paddingY],
  )
  const xScale = useMemo(
    () => indexScale(values.length, width, paddingX),
    [values.length, width, paddingX],
  )
  const yScale = useMemo(
    () => seriesValueScale(values, { height, padding: paddingY, minDomainMax }),
    [values, height, paddingY, minDomainMax],
  )
  const hasTicks = (ticks?.length ?? 0) > 0
  const viewHeight = height + (hasTicks ? TICK_LABEL_SPACE : 0)

  return (
    <svg
      role="img"
      aria-label={ariaLabel}
      viewBox={`0 0 ${width} ${viewHeight}`}
      preserveAspectRatio="none"
      className={cn('block w-full overflow-visible', className)}
      data-testid={testId}
    >
      {description ? <title>{description}</title> : null}
      {gridlines?.map((gridline) => (
        <line
          key={gridline.index}
          x1={xScale(gridline.index)}
          y1={0}
          x2={xScale(gridline.index)}
          y2={height}
          className="stroke-border-light"
          strokeWidth={0.5}
        />
      ))}
      {hasLine ? (
        <>
          <path d={areaPath} className="fill-accent opacity-10" />
          <polyline
            points={linePoints}
            className="fill-none stroke-accent"
            strokeWidth={1.5}
            strokeLinecap="round"
            strokeLinejoin="round"
            vectorEffect="non-scaling-stroke"
          />
        </>
      ) : null}
      {markers?.map((marker) => {
        const point = points[marker.index]
        if (!point) return null
        return (
          <circle
            key={marker.index}
            cx={point.x}
            cy={point.y}
            r={marker.radius ?? DEFAULT_MARKER_RADIUS}
            className="fill-accent"
          />
        )
      })}
      {referenceValue !== undefined ? (
        <line
          x1={paddingX}
          y1={yScale(referenceValue)}
          x2={width - paddingX}
          y2={yScale(referenceValue)}
          className="stroke-accent opacity-50"
          strokeWidth={0.75}
          strokeDasharray="4 3"
          vectorEffect="non-scaling-stroke"
        />
      ) : null}
      {children?.({ points, xScale, yScale, width, height, padding })}
      {ticks?.map((tick) => (
        <text
          key={tick.index}
          x={xScale(tick.index)}
          y={height + 10}
          textAnchor="middle"
          className="fill-ink-faint font-mono text-[7.5px]"
        >
          {tick.label}
        </text>
      ))}
    </svg>
  )
}
