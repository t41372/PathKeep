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
 * - Expose the series' own value→y scale (`seriesValueScale`) so callers can
 *   plot one more value (a reference/mean line, a custom marker) in the
 *   exact same coordinate space as `scaleSeriesToPoints` output.
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
  /** Inset applied on all four sides. Defaults to 0. Overridden per-axis by `paddingX`/`paddingY`. */
  padding?: number
  /** Inset applied on the left/right sides only. Defaults to `padding`. */
  paddingX?: number
  /** Inset applied on the top/bottom sides only. Defaults to `padding`. */
  paddingY?: number
  /** Forwarded to {@link seriesValueScale}'s domain-max floor. Opt-in; defaults to 0 (no extra floor). */
  minDomainMax?: number
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

export interface SeriesValueScaleOptions {
  height: number
  /** Inset applied on top and bottom. Defaults to 0. */
  padding?: number
  /**
   * Additionally floors the effective domain max at this value, even if
   * every value in the series is smaller (and even if the series is all
   * positive, unlike the plain zero/NaN guard below). Opt-in only — omitted
   * or 0 leaves the plain zero/NaN-guarded behavior untouched. Lets a
   * caller keep a low-magnitude series (e.g. a percentage that rarely rises
   * above a point or two) visually flat near the baseline instead of
   * auto-scaling its tiny peak to full height.
   */
  minDomainMax?: number
}

/**
 * Builds a value→y scale mapping `[0, max(values, 0)]` onto
 * `[height - padding, padding]` (inverted, since SVG y grows downward).
 *
 * Shared by {@link scaleSeriesToPoints} (so a series' points land on this
 * exact domain) and available directly to callers that need to plot one
 * more value in the same coordinate space as an already-scaled series —
 * e.g. a mean/threshold reference line — without re-deriving the domain
 * themselves.
 *
 * An all-zero or empty `values` floors the domain max to 1 so every output
 * stays finite instead of producing NaN. A caller-supplied `minDomainMax`
 * floors it further still, preventing a series whose real (positive) peak
 * is smaller from being auto-scaled to full height — e.g. a percentage
 * series that never rises above 1% would otherwise look identical to one
 * that spikes to 100%.
 */
export function seriesValueScale(
  values: number[],
  { height, padding = 0, minDomainMax = 0 }: SeriesValueScaleOptions,
): (value: number) => number {
  const maxValue = Math.max(...values, 0)
  const guardedMax = maxValue > 0 ? maxValue : 1
  return createLinearScale({
    domainMin: 0,
    domainMax: Math.max(guardedMax, minDomainMax),
    rangeMin: height - padding,
    rangeMax: padding,
  })
}

/**
 * Maps an ordered numeric series onto `{x,y}` points inside a
 * width/height box.
 *
 * - x follows point index via {@link indexScale}.
 * - y follows value via {@link seriesValueScale}.
 *
 * `paddingX`/`paddingY` let a caller inset one axis more than the other
 * (e.g. a wider left/right margin to leave room for edge tick labels);
 * both default to `padding` when omitted.
 *
 * Returns `[]` for an empty series.
 */
export function scaleSeriesToPoints(
  values: number[],
  {
    width,
    height,
    padding = 0,
    paddingX = padding,
    paddingY = padding,
    minDomainMax,
  }: SeriesLayoutOptions,
): Point[] {
  if (values.length === 0) return []
  const xScale = indexScale(values.length, width, paddingX)
  const yScale = seriesValueScale(values, {
    height,
    padding: paddingY,
    minDomainMax,
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
