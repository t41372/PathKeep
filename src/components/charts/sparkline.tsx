/**
 * @file sparkline.tsx
 * @description Reusable SVG line+area sparkline primitive built on `chart-geometry.ts`.
 * @module components/charts
 *
 * ## Responsibilities
 * - Render an ordered numeric series as a filled-area line chart: a
 *   low-opacity `accent` area under a ~1.5px `accent` stroke line.
 * - Expose `role="img"` + a caller-supplied `aria-label` (charts are
 *   decorative-but-meaningful SVG, not interactive controls here).
 * - Optionally render `font-mono` tick labels under specific series
 *   indices (e.g. hour-of-day ticks under a 24-point series).
 * - Degrade gracefully for 0 or 1 points: still render the `<svg>` shell
 *   (so the surrounding layout doesn't jump) but skip the line/area, since
 *   neither is meaningful with fewer than 2 points.
 *
 * ## Not responsible for
 * - Aggregating the series — callers pass already-computed values.
 * - This task does not migrate the two existing hand-rolled sparklines
 *   (`HourlySparkline` in `paper-day-insights.tsx`, `DiscoverySparkline` in
 *   `discovery-trend-section.tsx`) onto this component — it is built now so
 *   a later step can adopt it without redesigning the geometry contract.
 *
 * ## Dependencies
 * - `./chart-geometry` for the scale + path-building math.
 *
 * ## Performance notes
 * - O(values.length); sparklines render tens to low-hundreds of points, well
 *   within a single render pass.
 */

import { useMemo } from 'react'
import { cn } from '@/lib/cn'
import {
  buildAreaPath,
  buildPolylinePoints,
  indexScale,
  scaleSeriesToPoints,
} from './chart-geometry'

export interface SparklineTick {
  /** Index into `values` this tick labels. */
  index: number
  label: string
}

export interface SparklineProps {
  values: number[]
  /** SVG viewBox width, in user units. Defaults to 200. */
  width?: number
  /** SVG viewBox height (excludes tick label space), in user units. Defaults to 48. */
  height?: number
  /** Inset applied on all sides of the plotted area. Defaults to 4. */
  padding?: number
  /** Required — describes the chart for screen readers (e.g. "24-hour activity"). */
  ariaLabel: string
  /** Optional tick labels rendered under specific series indices. */
  ticks?: SparklineTick[]
  testId?: string
  className?: string
}

const DEFAULT_WIDTH = 200
const DEFAULT_HEIGHT = 48
const DEFAULT_PADDING = 4
/** Extra vertical space reserved under the plot for tick labels. */
const TICK_LABEL_SPACE = 12

export function Sparkline({
  values,
  width = DEFAULT_WIDTH,
  height = DEFAULT_HEIGHT,
  padding = DEFAULT_PADDING,
  ariaLabel,
  ticks,
  testId,
  className,
}: SparklineProps) {
  const points = useMemo(
    () => scaleSeriesToPoints(values, { width, height, padding }),
    [values, width, height, padding],
  )
  const hasLine = points.length >= 2
  const linePoints = useMemo(
    () => (hasLine ? buildPolylinePoints(points) : ''),
    [hasLine, points],
  )
  const areaPath = useMemo(
    () => (hasLine ? buildAreaPath(points, height - padding) : ''),
    [hasLine, points, height, padding],
  )
  const tickXScale = useMemo(
    () => indexScale(values.length, width, padding),
    [values.length, width, padding],
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
      {ticks?.map((tick) => (
        <text
          key={tick.index}
          x={tickXScale(tick.index)}
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
