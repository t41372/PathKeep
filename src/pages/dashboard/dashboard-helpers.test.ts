import { describe, expect, test } from 'vitest'
import type { StorageSummary } from '@/lib/types'
import {
  compactNumber,
  dashboardHeatmapRange,
  dashboardThreadsRange,
  formatSpan,
  humanizeBytes,
  isoDateOnly,
  sumStorageBytes,
} from './dashboard-helpers'

describe('dashboard-helpers', () => {
  describe('isoDateOnly', () => {
    test('formats Date instances as YYYY-MM-DD', () => {
      expect(isoDateOnly(new Date(2026, 4, 20))).toBe('2026-05-20')
      expect(isoDateOnly(new Date(2026, 0, 1))).toBe('2026-01-01')
      expect(isoDateOnly(new Date(2026, 11, 9))).toBe('2026-12-09')
    })
  })

  describe('dashboardThreadsRange', () => {
    test('returns a 30-day window ending today', () => {
      const range = dashboardThreadsRange(new Date(2026, 4, 20))
      expect(range.end).toBe('2026-05-20')
      expect(range.start).toBe('2026-04-21')
    })

    test('range spans exactly 29 calendar days (30 inclusive)', () => {
      const range = dashboardThreadsRange(new Date(2026, 4, 20))
      const start = new Date(range.start)
      const end = new Date(range.end)
      const diffDays = Math.round(
        (end.getTime() - start.getTime()) / (24 * 60 * 60 * 1000),
      )
      expect(diffDays).toBe(29)
    })
  })

  describe('dashboardHeatmapRange', () => {
    test('returns a 365-day rolling window ending today', () => {
      const range = dashboardHeatmapRange(new Date(2026, 4, 20))
      expect(range.end).toBe('2026-05-20')
      expect(range.start).toBe('2025-05-21')
    })
  })

  describe('compactNumber', () => {
    test('formats millions with one decimal and an M suffix', () => {
      expect(compactNumber(1_500_000)).toBe('1.5M')
      expect(compactNumber(14_400_000)).toBe('14.4M')
    })

    test('formats thousands with one decimal and a K suffix', () => {
      expect(compactNumber(2_400)).toBe('2.4K')
      expect(compactNumber(1_000)).toBe('1.0K')
    })

    test('renders values below 1000 verbatim', () => {
      expect(compactNumber(42)).toBe('42')
      expect(compactNumber(0)).toBe('0')
    })
  })

  describe('humanizeBytes', () => {
    test('returns empty string for zero / negative input', () => {
      expect(humanizeBytes(0)).toBe('')
      expect(humanizeBytes(-100)).toBe('')
    })

    test('escalates through B / KB / MB / GB / TB', () => {
      expect(humanizeBytes(512)).toBe('512 B')
      expect(humanizeBytes(2 * 1024)).toBe('2.0 KB')
      expect(humanizeBytes(5 * 1024 * 1024)).toBe('5.0 MB')
      expect(humanizeBytes(3 * 1024 * 1024 * 1024)).toBe('3.0 GB')
      expect(humanizeBytes(2 * 1024 ** 4)).toBe('2.0 TB')
    })
  })

  describe('sumStorageBytes', () => {
    test('sums every storage category', () => {
      const storage: StorageSummary = {
        archiveDatabaseBytes: 1,
        sourceEvidenceDatabaseBytes: 2,
        searchDatabaseBytes: 4,
        intelligenceDatabaseBytes: 8,
        manifestBytes: 16,
        snapshotBytes: 32,
        exportBytes: 64,
        stagingBytes: 128,
        quarantineBytes: 256,
        semanticSidecarBytes: 512,
        intelligenceBlobBytes: 1024,
      }
      expect(sumStorageBytes(storage)).toBe(2047)
    })
  })

  describe('formatSpan', () => {
    const t = (key: string, vars?: Record<string, string | number>) =>
      vars
        ? `${key} ${Object.entries(vars)
            .map(([k, v]) => `${k}=${v}`)
            .join(',')}`
        : key

    test('renders years and remainder months when more than a year passed', () => {
      const last = new Date('2024-01-01T00:00:00Z').toISOString()
      const now = new Date('2026-05-20T00:00:00Z')
      expect(formatSpan(last, t, now)).toBe(
        'dashboard.spanYearsAndMonths years=2,months=4',
      )
    })

    test('renders months when less than a year passed', () => {
      const last = new Date('2026-01-15T00:00:00Z').toISOString()
      const now = new Date('2026-05-20T00:00:00Z')
      expect(formatSpan(last, t, now)).toBe('dashboard.spanMonths months=4')
    })

    test('renders days when less than a month passed', () => {
      const last = new Date('2026-05-15T00:00:00Z').toISOString()
      const now = new Date('2026-05-20T00:00:00Z')
      expect(formatSpan(last, t, now)).toBe('dashboard.spanDays days=5')
    })

    test('renders the today label when less than a day passed', () => {
      const last = new Date('2026-05-20T08:00:00Z').toISOString()
      const now = new Date('2026-05-20T20:00:00Z')
      expect(formatSpan(last, t, now)).toBe('dashboard.spanToday')
    })

    test('returns em-dash for malformed timestamps', () => {
      expect(formatSpan('not-a-date', t)).toBe('—')
    })

    test('returns em-dash when the translator throws (catch fall-through)', () => {
      // Simulates a translator catalog mishap: the try/catch must absorb
      // the throw and produce the em-dash placeholder instead of crashing
      // the hero strip.
      const throwingT: Parameters<typeof formatSpan>[1] = () => {
        throw new Error('catalog miss')
      }
      const last = new Date('2025-12-20T08:00:00Z').toISOString()
      const now = new Date('2026-05-20T08:00:00Z')
      expect(formatSpan(last, throwingT, now)).toBe('—')
    })
  })
})
