import { describe, expect, test } from 'vitest'
import {
  buildAreaPath,
  buildLinePath,
  buildPolylinePoints,
  createLinearScale,
  indexScale,
  scaleSeriesToPoints,
} from './chart-geometry'

describe('createLinearScale', () => {
  test('maps domain to range linearly', () => {
    const scale = createLinearScale({
      domainMin: 0,
      domainMax: 10,
      rangeMin: 0,
      rangeMax: 100,
    })
    expect(scale(0)).toBe(0)
    expect(scale(5)).toBe(50)
    expect(scale(10)).toBe(100)
  })

  test('supports an inverted range (e.g. SVG y grows downward)', () => {
    const scale = createLinearScale({
      domainMin: 0,
      domainMax: 10,
      rangeMin: 100,
      rangeMax: 0,
    })
    expect(scale(0)).toBe(100)
    expect(scale(10)).toBe(0)
    expect(scale(5)).toBe(50)
  })

  test('extrapolates outside the domain (no clamping)', () => {
    const scale = createLinearScale({
      domainMin: 0,
      domainMax: 10,
      rangeMin: 0,
      rangeMax: 100,
    })
    expect(scale(20)).toBe(200)
    expect(scale(-10)).toBe(-100)
  })

  test('pins to the range midpoint for a degenerate (zero-span) domain', () => {
    const scale = createLinearScale({
      domainMin: 5,
      domainMax: 5,
      rangeMin: 0,
      rangeMax: 100,
    })
    expect(scale(5)).toBe(50)
    expect(scale(999)).toBe(50) // any input still resolves to the midpoint
  })
})

describe('indexScale', () => {
  test('spreads indices evenly across the width, inset by padding', () => {
    const scale = indexScale(5, 100, 10)
    expect(scale(0)).toBe(10)
    expect(scale(4)).toBe(90)
    expect(scale(2)).toBe(50)
  })

  test('floors a 0 or 1 length domain to avoid divide-by-zero', () => {
    const single = indexScale(1, 100, 10)
    expect(single(0)).toBe(10)
    const empty = indexScale(0, 100, 10)
    expect(empty(0)).toBe(10)
  })
})

describe('scaleSeriesToPoints', () => {
  test('returns [] for an empty series', () => {
    expect(scaleSeriesToPoints([], { width: 100, height: 50 })).toEqual([])
  })

  test('maps a single value to the left inset, at the value height', () => {
    const points = scaleSeriesToPoints([10], {
      width: 100,
      height: 50,
      padding: 5,
    })
    expect(points).toEqual([{ x: 5, y: 5 }]) // only value === max => top
  })

  test('maps a multi-point series across width, high values near the top', () => {
    const points = scaleSeriesToPoints([0, 5, 10], {
      width: 100,
      height: 50,
      padding: 0,
    })
    expect(points).toHaveLength(3)
    expect(points[0]).toEqual({ x: 0, y: 50 }) // value 0 -> baseline (bottom)
    expect(points[1]).toEqual({ x: 50, y: 25 }) // value 5 (half of max 10) -> mid height
    expect(points[2]).toEqual({ x: 100, y: 0 }) // value 10 (max) -> top
  })

  test('floors an all-zero series to the baseline instead of NaN', () => {
    const points = scaleSeriesToPoints([0, 0, 0], {
      width: 90,
      height: 30,
    })
    expect(points.every((p) => Number.isFinite(p.y))).toBe(true)
    expect(points.map((p) => p.y)).toEqual([30, 30, 30])
  })

  test('defaults padding to 0 when omitted', () => {
    const points = scaleSeriesToPoints([1, 2], { width: 10, height: 10 })
    expect(points[0].x).toBe(0)
    expect(points[1].x).toBe(10)
  })
})

describe('buildLinePath', () => {
  test('returns "" for an empty series', () => {
    expect(buildLinePath([])).toBe('')
  })

  test('returns a moveto-only path for a single point', () => {
    expect(buildLinePath([{ x: 1, y: 2 }])).toBe('M1.0,2.0')
  })

  test('joins multiple points with M then L commands', () => {
    const path = buildLinePath([
      { x: 0, y: 0 },
      { x: 10, y: 5 },
      { x: 20, y: 10 },
    ])
    expect(path).toBe('M0.0,0.0 L10.0,5.0 L20.0,10.0')
  })
})

describe('buildPolylinePoints', () => {
  test('returns "" for an empty series', () => {
    expect(buildPolylinePoints([])).toBe('')
  })

  test('joins points as "x,y x,y" pairs', () => {
    expect(
      buildPolylinePoints([
        { x: 0, y: 0 },
        { x: 5, y: 2.5 },
      ]),
    ).toBe('0.0,0.0 5.0,2.5')
  })
})

describe('buildAreaPath', () => {
  test('returns "" for an empty series', () => {
    expect(buildAreaPath([], 100)).toBe('')
  })

  test('returns "" for a single point (nothing to fill under)', () => {
    expect(buildAreaPath([{ x: 1, y: 2 }], 100)).toBe('')
  })

  test('closes the shape down to the baseline for 2+ points', () => {
    const path = buildAreaPath(
      [
        { x: 0, y: 10 },
        { x: 10, y: 0 },
      ],
      20,
    )
    expect(path).toBe('M0.0,10.0 L10.0,0.0 L10.0,20.0 L0.0,20.0 Z')
  })
})
