/**
 * @file chart-geometry.ts
 * @description Pure, framework-free SVG scale + path-building helpers shared by every PathKeep chart primitive.
 * @module components/charts
 *
 * ## Responsibilities
 * - Provide a linear scale (`createLinearScale`) that maps a numeric domain
 *   onto a pixel range without dividing by zero on a degenerate (single-value)
 *   domain.
 * - Map an ordered numeric series onto `{x,y}` points inside a width/height/
 *   padding box (`scaleSeriesToPoints`), and a standalone index→x scale
 *   (`indexScale`) for tick placement that doesn't need a value.
 * - Turn `{x,y}` points into SVG `path`/`polyline` strings (`buildLinePath`,
 *   `buildPolylinePoints`, `buildAreaPath`).
 *
 * ## Not responsible for
 * - Rendering, tokens/colors, or React — this module has zero DOM/React
 *   dependencies so it can be unit-tested and reused by any future chart
 *   (line, area, calendar grid) without dragging in JSX.
 * - Data aggregation — callers must already have pre-aggregated values; see
 *   the "never aggregate on the render path" constraint in
 *   `docs/design/chart-primitive-tradeoff.md`.
 *
 * ## Dependencies
 * - None (pure TypeScript).
 *
 * ## Performance notes
 * - Every function here is O(n) in the number of points/values, with n
 *   bounded by the small (≤ a few thousand) point counts a chart renders.
 *   Safe to call directly inside a render path (no memoization required by
 *   this module itself; callers may still memoize around expensive upstream
 *   aggregation).
 */

export interface Point {
  x: number
  y: number
}

export interface LinearScaleConfig {
  domainMin: number
  domainMax: number
  rangeMin: number
  rangeMax: number
}

/**
 * Builds a linear scale function mapping `[domainMin, domainMax]` onto
 * `[rangeMin, rangeMax]`.
 *
 * Degenerate domains (`domainMin === domainMax`, e.g. a single-value or
 * all-zero series) would divide by zero under a naive implementation; this
 * pins the output to the range midpoint instead so callers always get a
 * finite number back.
 */
export function createLinearScale({
  domainMin,
  domainMax,
  rangeMin,
  rangeMax,
}: LinearScaleConfig): (value: number) => number {
  const domainSpan = domainMax - domainMin
  if (domainSpan === 0) {
    const mid = rangeMin + (rangeMax - rangeMin) / 2
    return () => mid
  }
  return (value: number) =>
    rangeMin + ((value - domainMin) / domainSpan) * (rangeMax - rangeMin)
}

export interface SeriesLayoutOptions {
  width: number
  height: number
  /** Inset applied on all four sides. Defaults to 0. */
  padding?: number
}

/**
 * Builds an index→x scale for `length` ordered points inside `width`,
 * inset by `padding` on both sides. Used for tick placement that needs to
 * line up with `scaleSeriesToPoints` output without requiring the values
 * themselves (e.g. an hour-of-day tick under a 24-point sparkline).
 *
 * A `length` of 0 or 1 would otherwise divide by zero walking the index
 * domain; the domain max is floored to 1 so a single point renders at the
 * left inset instead of producing NaN.
 */
export function indexScale(
  length: number,
  width: number,
  padding = 0,
): (index: number) => number {
  return createLinearScale({
    domainMin: 0,
    domainMax: Math.max(length - 1, 1),
    rangeMin: padding,
    rangeMax: width - padding,
  })
}

/**
 * Maps an ordered numeric series onto `{x,y}` points inside a
 * width/height box.
 *
 * - x follows point index via {@link indexScale}.
 * - y follows value via a scale from `[0, max(values)]` to
 *   `[height - padding, padding]` (inverted, since SVG y grows downward).
 *   An all-zero or empty series floors the domain max to 1 so every point
 *   renders at the baseline instead of producing NaN.
 *
 * Returns `[]` for an empty series.
 */
export function scaleSeriesToPoints(
  values: number[],
  { width, height, padding = 0 }: SeriesLayoutOptions,
): Point[] {
  if (values.length === 0) return []
  const xScale = indexScale(values.length, width, padding)
  const maxValue = Math.max(...values, 0)
  const yScale = createLinearScale({
    domainMin: 0,
    domainMax: maxValue > 0 ? maxValue : 1,
    rangeMin: height - padding,
    rangeMax: padding,
  })
  return values.map((value, index) => ({ x: xScale(index), y: yScale(value) }))
}

function formatCoord(value: number): string {
  return value.toFixed(1)
}

/**
 * Builds an SVG `<path>` `d` attribute (`M`/`L` commands) from ordered
 * points. Returns `''` for an empty series; a single point yields a
 * moveto-only path (no line segment) so callers can render it as a dot or
 * skip it without the path string itself throwing.
 */
export function buildLinePath(points: Point[]): string {
  if (points.length === 0) return ''
  return points
    .map(
      (point, index) =>
        `${index === 0 ? 'M' : 'L'}${formatCoord(point.x)},${formatCoord(point.y)}`,
    )
    .join(' ')
}

/**
 * Builds an SVG `<polyline>` `points` attribute from ordered points.
 * Returns `''` for an empty series.
 */
export function buildPolylinePoints(points: Point[]): string {
  if (points.length === 0) return ''
  return points
    .map((point) => `${formatCoord(point.x)},${formatCoord(point.y)}`)
    .join(' ')
}

/**
 * Builds a closed SVG `<path>` `d` attribute that traces `points` and then
 * drops straight down to `baselineY` at the last point, back along the
 * baseline to below the first point, and closes — i.e. the filled "area
 * under the line" shape sparklines render at low opacity.
 *
 * Requires at least 2 points to describe a meaningful area; returns `''`
 * for 0 or 1 points (a single point has no line to fill under).
 */
export function buildAreaPath(points: Point[], baselineY: number): string {
  if (points.length < 2) return ''
  const linePath = buildLinePath(points)
  const last = points[points.length - 1]
  const first = points[0]
  return `${linePath} L${formatCoord(last.x)},${formatCoord(baselineY)} L${formatCoord(first.x)},${formatCoord(baselineY)} Z`
}
