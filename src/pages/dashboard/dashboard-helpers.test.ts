import { describe, expect, test } from 'vitest'
import type { BackupRunOverview, StorageSummary } from '@/lib/types'
import type { DiscoveryTrendPoint, PathFlow } from '@/lib/core-intelligence'
import {
  compactNumber,
  countRunsInRange,
  dashboardHeatmapRange,
  dashboardThreadsRange,
  dashboardWeekRange,
  firstRegistrableDomainStep,
  formatSpan,
  humanizeBytes,
  isoDateOnly,
  sumStorageBytes,
  sumWeekTrend,
} from './dashboard-helpers'

/**
 * Builds an ISO timestamp pinned to local noon on the given YYYY-MM-DD so the
 * `countRunsInRange` local-date comparison stays stable regardless of the test
 * machine's timezone (a UTC-midnight ISO string would slip a day in negative
 * offsets and flip the boundary assertions).
 */
function isoLocalNoon(dateKey: string): string {
  const [year, month, day] = dateKey.split('-').map(Number)
  return new Date(year, month - 1, day, 12, 0, 0).toISOString()
}

function makeFlow(steps: PathFlow['steps']): PathFlow {
  return {
    flowId: 'flow-test',
    flowPattern: steps.map((step) => step.label).join(' → '),
    stepCount: steps.length,
    occurrenceCount: 1,
    lastSeenAt: '2026-05-20T00:00:00Z',
    steps,
  }
}

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

  describe('firstRegistrableDomainStep', () => {
    test('returns the first step that carries a registrable domain', () => {
      const flow = makeFlow([
        { index: 0, label: 'Search: rust', registrableDomain: null },
        { index: 1, label: 'docs.rs', registrableDomain: 'docs.rs' },
        { index: 2, label: 'crates.io', registrableDomain: 'crates.io' },
      ])
      expect(firstRegistrableDomainStep(flow)).toBe('docs.rs')
    })

    test('skips steps with an undefined registrable domain', () => {
      const flow = makeFlow([
        { index: 0, label: 'Search: rust' },
        { index: 1, label: 'github.com', registrableDomain: 'github.com' },
      ])
      expect(firstRegistrableDomainStep(flow)).toBe('github.com')
    })

    test('returns null when no step carries a registrable domain', () => {
      const flow = makeFlow([
        { index: 0, label: 'Search: rust async', registrableDomain: null },
        { index: 1, label: 'Search: tokio', registrableDomain: undefined },
      ])
      expect(firstRegistrableDomainStep(flow)).toBeNull()
    })

    test('returns null for a flow with no steps', () => {
      expect(firstRegistrableDomainStep(makeFlow([]))).toBeNull()
    })
  })

  describe('dashboardHeatmapRange', () => {
    test('returns a 365-day rolling window ending today', () => {
      const range = dashboardHeatmapRange(new Date(2026, 4, 20))
      expect(range.end).toBe('2026-05-20')
      expect(range.start).toBe('2025-05-21')
    })
  })

  describe('dashboardWeekRange', () => {
    test('returns the Mon→Sun ISO week containing a mid-week date', () => {
      // Wednesday 2026-05-20 → ISO week is Mon 2026-05-18 .. Sun 2026-05-24.
      const range = dashboardWeekRange(new Date(2026, 4, 20))
      expect(range.start).toBe('2026-05-18')
      expect(range.end).toBe('2026-05-24')
    })

    test('anchors a Sunday to the week that started the preceding Monday', () => {
      // Sunday 2026-05-24 → still belongs to the Mon 2026-05-18 week, not the
      // next one. The `getDay() || 7` mapping keeps Sunday as offset 6.
      const range = dashboardWeekRange(new Date(2026, 4, 24))
      expect(range.start).toBe('2026-05-18')
      expect(range.end).toBe('2026-05-24')
    })

    test('keeps a Monday as its own week start', () => {
      const range = dashboardWeekRange(new Date(2026, 4, 18))
      expect(range.start).toBe('2026-05-18')
      expect(range.end).toBe('2026-05-24')
    })
  })

  describe('sumWeekTrend', () => {
    const point = (
      totalVisits: number,
      newDomainCount: number,
    ): DiscoveryTrendPoint => ({
      dateKey: '2026-05-20',
      discoveryRate: 0,
      newDomainCount,
      totalVisits,
    })

    test('sums visits and new domains across the week', () => {
      expect(sumWeekTrend([point(100, 3), point(47, 5)])).toEqual({
        totalVisits: 147,
        newDomains: 8,
      })
    })

    test('treats null / undefined points as a zero week', () => {
      expect(sumWeekTrend(null)).toEqual({ totalVisits: 0, newDomains: 0 })
      expect(sumWeekTrend(undefined)).toEqual({ totalVisits: 0, newDomains: 0 })
      expect(sumWeekTrend([])).toEqual({ totalVisits: 0, newDomains: 0 })
    })
  })

  describe('countRunsInRange', () => {
    const run = (
      id: number,
      startedAt: string | null | undefined,
    ): BackupRunOverview =>
      ({
        id,
        startedAt: startedAt as string,
        status: 'completed',
        profilesProcessed: 1,
        newVisits: 0,
        newUrls: 0,
        newDownloads: 0,
      }) as BackupRunOverview

    const range = { start: '2026-05-18', end: '2026-05-24' }

    test('counts only runs whose start date falls inside the inclusive window', () => {
      const runs = [
        run(1, '2026-05-18T23:30:00Z'), // boundary start (in)
        run(2, '2026-05-21T08:00:00Z'), // mid week (in)
        run(3, '2026-05-24T00:00:00Z'), // boundary end (in)
        run(4, '2026-05-17T23:59:00Z'), // before (out)
        run(5, '2026-05-25T00:01:00Z'), // after (out)
      ]
      // Use local-midnight timestamps so the assertion is timezone-stable:
      // construct via Date parts rather than UTC ISO for the boundary cases.
      const localRuns = [
        { ...runs[0], startedAt: isoLocalNoon('2026-05-18') },
        { ...runs[1], startedAt: isoLocalNoon('2026-05-21') },
        { ...runs[2], startedAt: isoLocalNoon('2026-05-24') },
        { ...runs[3], startedAt: isoLocalNoon('2026-05-17') },
        { ...runs[4], startedAt: isoLocalNoon('2026-05-25') },
      ]
      expect(countRunsInRange(localRuns, range)).toBe(3)
    })

    test('skips runs with a missing or malformed startedAt', () => {
      const runs = [
        run(1, isoLocalNoon('2026-05-20')),
        run(2, null),
        run(3, undefined),
        run(4, 'not-a-date'),
      ]
      expect(countRunsInRange(runs, range)).toBe(1)
    })

    test('returns zero for an empty run list', () => {
      expect(countRunsInRange([], range)).toBe(0)
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
      expect(formatSpan(last, t, null, now)).toBe(
        'dashboard.spanYearsAndMonths years=2,months=4',
      )
    })

    test('renders months when less than a year passed', () => {
      const last = new Date('2026-01-15T00:00:00Z').toISOString()
      const now = new Date('2026-05-20T00:00:00Z')
      expect(formatSpan(last, t, null, now)).toBe(
        'dashboard.spanMonths months=4',
      )
    })

    test('renders days when less than a month passed', () => {
      const last = new Date('2026-05-15T00:00:00Z').toISOString()
      const now = new Date('2026-05-20T00:00:00Z')
      expect(formatSpan(last, t, null, now)).toBe('dashboard.spanDays days=5')
    })

    test('renders the today label when less than a day passed', () => {
      const last = new Date('2026-05-20T08:00:00Z').toISOString()
      const now = new Date('2026-05-20T20:00:00Z')
      expect(formatSpan(last, t, null, now)).toBe('dashboard.spanToday')
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
      expect(formatSpan(last, throwingT, null, now)).toBe('—')
    })

    test('uses explicit end bound when archive has a fixed latest visit', () => {
      // Imported takeout that spans roughly a year and a half — start is
      // the earliest visit, end is the latest. The hero strip should label
      // archive coverage, not wall-clock gap to now.
      const start = new Date('2024-10-01T00:00:00Z').toISOString()
      const end = new Date('2026-04-22T00:00:00Z').toISOString()
      const now = new Date('2027-01-01T00:00:00Z')
      expect(formatSpan(start, t, end, now)).toBe(
        'dashboard.spanYearsAndMonths years=1,months=6',
      )
    })

    test('returns em-dash when the start bound is null/empty', () => {
      expect(formatSpan(null, t)).toBe('—')
      expect(formatSpan(undefined, t)).toBe('—')
      expect(formatSpan('', t)).toBe('—')
    })

    test('returns em-dash when the explicit end bound is malformed', () => {
      const start = new Date('2025-04-22T00:00:00Z').toISOString()
      expect(formatSpan(start, t, 'not-a-date')).toBe('—')
    })

    test('clamps to today when end is somehow earlier than start', () => {
      const start = new Date('2026-05-20T00:00:00Z').toISOString()
      const end = new Date('2026-05-19T00:00:00Z').toISOString()
      expect(formatSpan(start, t, end)).toBe('dashboard.spanToday')
    })
  })
})
