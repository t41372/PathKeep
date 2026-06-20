/**
 * Pure SVG-geometry helper for the Discovery Trend sparkline.
 *
 * ## Responsibilities
 * - Map discovery-rate points to an SVG polyline `points` string.
 *
 * ## Not responsible for
 * - Rendering the sparkline or section (that stays in `discovery-trend-section.tsx`).
 *
 * ## Dependencies
 * - `DiscoveryTrendPoint` shape from `lib/core-intelligence`.
 *
 * ## Performance notes
 * - O(points); kept in a `.ts` module so the section file only exports a
 *   component (Fast Refresh) while staying directly unit-testable.
 */

import type { DiscoveryTrendPoint } from '../../../../lib/core-intelligence/types-analysis'

/**
 * Builds an SVG polyline `points` attribute from discovery-rate values, mapping
 * each point to an x/y coordinate inside the given viewBox dimensions. Returns
 * an empty string for fewer than two points so callers can skip rendering.
 */
export function buildSparklinePath(
  points: DiscoveryTrendPoint[],
  width: number,
  height: number,
  padding: number,
): string {
  if (points.length < 2) return ''

  const maxRate = Math.max(...points.map((p) => p.discoveryRate), 0.01)
  const innerWidth = width - padding * 2
  const innerHeight = height - padding * 2

  return points
    .map((point, index) => {
      const x = padding + (index / (points.length - 1)) * innerWidth
      const y =
        padding + innerHeight - (point.discoveryRate / maxRate) * innerHeight
      return `${x.toFixed(1)},${y.toFixed(1)}`
    })
    .join(' ')
}
