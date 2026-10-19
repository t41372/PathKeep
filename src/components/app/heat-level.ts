export type HeatLevel = 0 | 1 | 2 | 3 | 4

/** Buckets counts into five levels relative to the busiest cell. */
export function heatLevel(value: number, max: number): HeatLevel {
  if (value <= 0 || max <= 0) return 0
  const ratio = value / max
  if (ratio < 0.15) return 1
  if (ratio < 0.4) return 2
  if (ratio < 0.7) return 3
  return 4
}
