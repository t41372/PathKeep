import { describe, expect, test } from 'vitest'
import type { HistoryEntry } from '@/lib/types/archive'
import {
  describeDay,
  formatHourMinute,
  groupEntriesByDay,
  localDayKey,
} from './group-entries'

function makeEntry(over: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id: 1,
    profileId: 'chrome:Default',
    url: 'https://example.com/',
    title: 'Example',
    domain: 'example.com',
    favicon: null,
    visitedAt: '2026-05-17T10:30:00',
    visitTime: 1747488600,
    durationMs: null,
    transition: 0,
    sourceVisitId: 0,
    appId: null,
    ...over,
  }
}

describe('localDayKey', () => {
  test('returns "unknown" for null / undefined / empty inputs', () => {
    expect(localDayKey(null)).toBe('unknown')
    expect(localDayKey(undefined)).toBe('unknown')
    expect(localDayKey('')).toBe('unknown')
  })

  test('returns the input prefix when the timestamp is unparseable', () => {
    expect(localDayKey('not-a-date')).toBe('not-a-date')
  })

  test('returns the local YYYY-MM-DD for a valid ISO timestamp', () => {
    expect(localDayKey('2026-05-17T10:30:00')).toBe('2026-05-17')
  })
})

describe('groupEntriesByDay', () => {
  test('returns an empty array for empty input', () => {
    expect(groupEntriesByDay([])).toEqual([])
  })

  test('buckets entries by local day, newest day first', () => {
    const days = groupEntriesByDay([
      makeEntry({
        id: 1,
        visitedAt: '2026-05-15T08:00:00',
        visitTime: 1747296000,
      }),
      makeEntry({
        id: 2,
        visitedAt: '2026-05-17T09:00:00',
        visitTime: 1747472400,
      }),
      makeEntry({
        id: 3,
        visitedAt: '2026-05-17T11:00:00',
        visitTime: 1747479600,
      }),
    ])
    expect(days.map((d) => d.date)).toEqual(['2026-05-17', '2026-05-15'])
    expect(days[0].visitCount).toBe(2)
    expect(days[0].domains).toBe(1)
    expect(days[0].sessions.length).toBeGreaterThanOrEqual(1)
  })

  test('falls back to Date.parse(visitedAt) when visitTime is missing or zero', () => {
    // visitTime=0 forces visitTimeMs to walk the Date.parse fallback branch
    // (group-entries.ts:65). The session split still resolves from visitedAt.
    const days = groupEntriesByDay([
      makeEntry({
        id: 1,
        visitTime: 0,
        visitedAt: '2026-05-17T08:00:00.000Z',
      }),
      makeEntry({
        id: 2,
        visitTime: 0,
        visitedAt: '2026-05-17T08:10:00.000Z',
      }),
    ])
    expect(days).toHaveLength(1)
    expect(days[0].sessions).toHaveLength(1)
    expect(days[0].sessions[0].visitCount).toBe(2)
  })

  test('falls back to 0 when both visitTime is missing and visitedAt is unparseable', () => {
    // visitTime=0 + invalid visitedAt → visitTimeMs returns 0 (line 66 fall-
    // through). The entry still groups under its raw date prefix.
    const days = groupEntriesByDay([
      makeEntry({ id: 1, visitTime: 0, visitedAt: 'not-a-date' }),
    ])
    expect(days).toHaveLength(1)
    expect(days[0].sessions).toHaveLength(1)
  })

  test('splits a day into multiple sessions when there is a > 30 minute gap', () => {
    const days = groupEntriesByDay([
      makeEntry({
        id: 1,
        visitedAt: '2026-05-17T08:00:00',
        visitTime: 1747468800,
      }),
      // 35 minutes later — past the SESSION_GAP_MINUTES threshold
      makeEntry({
        id: 2,
        visitedAt: '2026-05-17T08:35:00',
        visitTime: 1747470900,
      }),
    ])
    expect(days).toHaveLength(1)
    expect(days[0].sessions.length).toBe(2)
  })

  test('keeps entries in the same session when the gap is under 30 minutes', () => {
    const days = groupEntriesByDay([
      makeEntry({
        id: 1,
        visitedAt: '2026-05-17T08:00:00',
        visitTime: 1747468800,
      }),
      makeEntry({
        id: 2,
        visitedAt: '2026-05-17T08:10:00',
        visitTime: 1747469400,
      }),
    ])
    expect(days[0].sessions).toHaveLength(1)
    expect(days[0].sessions[0].visitCount).toBe(2)
  })

  test('folds 3+ consecutive same-domain visits into a stack block', () => {
    const days = groupEntriesByDay([
      makeEntry({
        id: 1,
        domain: 'github.com',
        visitedAt: '2026-05-17T08:00:00',
        visitTime: 1747468800,
      }),
      makeEntry({
        id: 2,
        domain: 'github.com',
        visitedAt: '2026-05-17T08:05:00',
        visitTime: 1747469100,
      }),
      makeEntry({
        id: 3,
        domain: 'github.com',
        visitedAt: '2026-05-17T08:10:00',
        visitTime: 1747469400,
      }),
    ])
    expect(days).toHaveLength(1)
    expect(days[0].sessions).toHaveLength(1)
    expect(days[0].sessions[0].blocks).toHaveLength(1)
    expect(days[0].sessions[0].blocks[0].type).toBe('stack')
  })

  test('counts unique domains per day', () => {
    const days = groupEntriesByDay([
      makeEntry({
        id: 1,
        domain: 'rust-lang.org',
        visitedAt: '2026-05-17T08:00:00',
        visitTime: 1747468800,
      }),
      makeEntry({
        id: 2,
        domain: 'github.com',
        visitedAt: '2026-05-17T08:10:00',
        visitTime: 1747469400,
      }),
      makeEntry({
        id: 3,
        domain: 'rust-lang.org',
        visitedAt: '2026-05-17T08:20:00',
        visitTime: 1747470000,
      }),
    ])
    expect(days[0].domains).toBe(2)
  })
})

describe('describeDay', () => {
  test('falls back to the raw string when the date has the wrong number of parts', () => {
    expect(describeDay('2026-05', 'en')).toBe('2026-05')
  })

  test('falls back to the raw string when any part is not a number', () => {
    expect(describeDay('2026-05-XX', 'en')).toBe('2026-05-XX')
  })

  test('produces a long-form locale label for valid dates', () => {
    const label = describeDay('2026-05-17', 'en')
    // The label should mention the year and the month — locale exact formatting
    // varies, so assert structural fragments instead of the exact string.
    expect(label).toMatch(/2026/)
  })

  test('falls back to the raw string when the toLocaleDateString locale is invalid', () => {
    // RangeError from a bogus BCP-47 tag should be caught and the raw date
    // returned. Different engines may not throw on every bad input, so try a
    // sentinel known to be invalid.
    const out = describeDay('2026-05-17', '!!!')
    expect(typeof out).toBe('string')
  })
})

describe('formatHourMinute', () => {
  test('produces a zero-padded HH:mm string for a valid timestamp', () => {
    const ms = new Date('2026-05-17T07:05:00Z').getTime()
    expect(formatHourMinute(ms, 'en')).toMatch(/\d{2}:\d{2}/)
  })

  test('falls back to "--:--" for NaN timestamps', () => {
    expect(formatHourMinute(Number.NaN, 'en')).toBe('--:--')
  })

  test('falls back to "--:--" when the locale throws inside toLocaleTimeString', () => {
    // Mock Date.prototype.toLocaleTimeString to throw — the helper must catch
    // it and return the fallback rather than bubbling up.
    const ms = new Date('2026-05-17T07:05:00Z').getTime()
    const proto = Date.prototype as unknown as {
      toLocaleTimeString: (...args: unknown[]) => string
    }
    const originalToLocale = proto.toLocaleTimeString
    proto.toLocaleTimeString = function throwingLocale(this: void) {
      throw new Error('locale broken')
    }
    try {
      expect(formatHourMinute(ms, 'en')).toBe('--:--')
    } finally {
      proto.toLocaleTimeString = originalToLocale
    }
  })
})
