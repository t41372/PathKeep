import { describe, expect, test } from 'vitest'
import {
  buildAreaPath,
  buildLinePath,
  buildPolylinePoints,
  createLinearScale,
  indexScale,
  scaleSeriesToPoints,
  seriesValueScale,
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

describe('seriesValueScale', () => {
  test('maps [0, max(values)] to [height - padding, padding]', () => {
    const scale = seriesValueScale([0, 5, 10], { height: 50, padding: 5 })
    expect(scale(0)).toBe(45)
    expect(scale(5)).toBe(25)
    expect(scale(10)).toBe(5)
  })

  test('floors an all-zero or empty domain to avoid divide-by-zero', () => {
    const zeroScale = seriesValueScale([0, 0, 0], { height: 30 })
    expect(zeroScale(0)).toBe(30)
    const emptyScale = seriesValueScale([], { height: 30 })
    expect(emptyScale(0)).toBe(30)
  })

  test('defaults padding to 0 when omitted', () => {
    const scale = seriesValueScale([10], { height: 10 })
    expect(scale(10)).toBe(0)
    expect(scale(0)).toBe(10)
  })

  test('matches the y values scaleSeriesToPoints derives internally', () => {
    const values = [2, 8, 4]
    const points = scaleSeriesToPoints(values, {
      width: 100,
      height: 40,
      padding: 4,
    })
    const scale = seriesValueScale(values, { height: 40, padding: 4 })
    values.forEach((value, index) => {
      expect(points[index].y).toBe(scale(value))
    })
  })

  test('floors the domain max at a custom minDomainMax even when the real peak is smaller', () => {
    // A peak of 0.003 would otherwise stretch to the very top of the chart;
    // flooring the domain at 0.01 keeps a near-zero series visually flat.
    const scale = seriesValueScale([0.001, 0.003, 0.002], {
      height: 40,
      minDomainMax: 0.01,
    })
    expect(scale(0.01)).toBe(0) // domain max is the floor, not the real peak
    expect(scale(0.003)).toBeCloseTo(28, 5) // 40 - (0.003/0.01)*40
  })

  test('leaves a real peak above minDomainMax untouched', () => {
    const scale = seriesValueScale([0, 5, 20], {
      height: 40,
      minDomainMax: 0.01,
    })
    expect(scale(20)).toBe(0) // the real peak (20), not the tiny floor, is the domain max
    expect(scale(0)).toBe(40)
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

  test('applies paddingX/paddingY independently when both are given', () => {
    const points = scaleSeriesToPoints([0, 10], {
      width: 100,
      height: 50,
      paddingX: 10,
      paddingY: 5,
    })
    expect(points[0]).toEqual({ x: 10, y: 45 }) // horizontal inset 10, vertical inset 5
    expect(points[1]).toEqual({ x: 90, y: 5 })
  })

  test('falls back paddingX/paddingY to padding when only padding is given', () => {
    const withPadding = scaleSeriesToPoints([0, 10], {
      width: 100,
      height: 50,
      padding: 8,
    })
    const withBoth = scaleSeriesToPoints([0, 10], {
      width: 100,
      height: 50,
      paddingX: 8,
      paddingY: 8,
    })
    expect(withPadding).toEqual(withBoth)
  })

  test('forwards minDomainMax to the internal value scale', () => {
    const floored = scaleSeriesToPoints([0.001, 0.002], {
      width: 100,
      height: 40,
      minDomainMax: 0.01,
    })
    // Peak (0.002) scaled against the 0.01 floor stays well short of the top.
    expect(floored[1].y).toBeCloseTo(32, 5) // 40 - (0.002/0.01)*40

    const unfloored = scaleSeriesToPoints([0.001, 0.002], {
      width: 100,
      height: 40,
    })
    // Without the floor, the same tiny peak auto-scales all the way to the top.
    expect(unfloored[1].y).toBeCloseTo(0, 5)
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
