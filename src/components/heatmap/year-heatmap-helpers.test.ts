import { describe, expect, test } from 'vitest'
import {
  bucketLevel,
  buildYearHeatmapCells,
  isoDateOnly,
  longestRecentStreak,
} from './year-heatmap-helpers'

describe('year-heatmap-helpers', () => {
  describe('isoDateOnly', () => {
    test('renders YYYY-MM-DD with zero-padded month / day', () => {
      expect(isoDateOnly(new Date(2026, 0, 3))).toBe('2026-01-03')
      expect(isoDateOnly(new Date(2026, 4, 20))).toBe('2026-05-20')
      expect(isoDateOnly(new Date(2026, 11, 31))).toBe('2026-12-31')
    })
  })

  describe('bucketLevel', () => {
    test('returns 0 when the count is zero or negative', () => {
      expect(bucketLevel(0, 100)).toBe(0)
      expect(bucketLevel(-5, 100)).toBe(0)
    })

    test('returns 0 when the max is zero (corrupt input)', () => {
      expect(bucketLevel(10, 0)).toBe(0)
    })

    test('quartile buckets scale linearly off the max', () => {
      expect(bucketLevel(20, 100)).toBe(1) // 0.2
      expect(bucketLevel(40, 100)).toBe(2) // 0.4
      expect(bucketLevel(60, 100)).toBe(3) // 0.6
      expect(bucketLevel(90, 100)).toBe(4) // 0.9
      expect(bucketLevel(100, 100)).toBe(4)
      expect(bucketLevel(1, 100)).toBe(1) // every non-zero day lights up
    })
  })

  describe('buildYearHeatmapCells', () => {
    test('produces dense cells for the requested window', () => {
      const cells = buildYearHeatmapCells(
        [
          { dateKey: '2026-05-19', totalVisits: 4 },
          { dateKey: '2026-05-21', totalVisits: 12 },
        ],
        new Date(2026, 4, 19),
        3,
      )
      expect(cells).toHaveLength(3)
      expect(cells[0]).toEqual({
        date: '2026-05-19',
        count: 4,
        level: 2,
        dayOfWeek: 2, // Tuesday
      })
      expect(cells[1]).toEqual({
        date: '2026-05-20',
        count: 0,
        level: 0,
        dayOfWeek: 3,
      })
      expect(cells[2]).toEqual({
        date: '2026-05-21',
        count: 12,
        level: 4,
        dayOfWeek: 4,
      })
    })

    test('returns all level-0 cells when no points carry data', () => {
      const cells = buildYearHeatmapCells([], new Date(2026, 4, 19), 5)
      expect(cells.map((cell) => cell.level)).toEqual([0, 0, 0, 0, 0])
    })
  })

  describe('longestRecentStreak', () => {
    test('returns the longest run of non-zero days', () => {
      const cells = buildYearHeatmapCells(
        [
          { dateKey: '2026-05-15', totalVisits: 1 },
          { dateKey: '2026-05-16', totalVisits: 1 },
          { dateKey: '2026-05-17', totalVisits: 1 },
          // gap on 5-18
          { dateKey: '2026-05-19', totalVisits: 1 },
          { dateKey: '2026-05-20', totalVisits: 1 },
        ],
        new Date(2026, 4, 15),
        6,
      )
      expect(longestRecentStreak(cells)).toBe(3)
    })

    test('returns 0 when all cells are empty', () => {
      const cells = buildYearHeatmapCells([], new Date(2026, 4, 15), 4)
      expect(longestRecentStreak(cells)).toBe(0)
    })
  })
})
