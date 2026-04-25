/**
 * This test file protects the front-end helper and contract logic in Format.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { afterEach, describe, expect, test, vi } from 'vitest'
import { localeTag } from './i18n'
import {
  calendarDayKey,
  formatBytes,
  formatDateTime,
  formatDuration,
  formatRelativeTime,
} from './format'

describe('format utilities', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('formatDateTime returns null for falsy input', () => {
    expect(formatDateTime(null, 'en')).toBeNull()
    expect(formatDateTime(undefined, 'en')).toBeNull()
    expect(formatDateTime('', 'en')).toBeNull()
  })

  test('formatDateTime formats dates for all locales', () => {
    const iso = '2026-04-03T12:00:00.000Z'
    expect(formatDateTime(iso, 'en')).toContain('2026')
    expect(formatDateTime(iso, 'zh-CN')).toContain('2026')
    expect(formatDateTime(iso, 'zh-TW')).toContain('2026')
  })

  test('formatDateTime uses medium-date and short-time formatting', () => {
    const iso = '2026-04-03T12:34:00.000Z'
    const date = new Date(iso)

    expect(formatDateTime(iso, 'en')).toBe(
      new Intl.DateTimeFormat(localeTag('en'), {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(date),
    )
    expect(formatDateTime(iso, 'zh-TW')).toBe(
      new Intl.DateTimeFormat(localeTag('zh-TW'), {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(date),
    )
  })

  test('calendarDayKey honors the requested timezone for day boundaries', () => {
    const iso = '2026-04-07T00:30:00.000Z'

    expect(calendarDayKey(iso, 'UTC')).toBe('04-07')
    expect(calendarDayKey(new Date(iso), 'UTC')).toBe('04-07')
    expect(calendarDayKey(iso, 'America/Phoenix')).toBe('04-06')
    expect(calendarDayKey(iso, 'Asia/Tokyo')).toBe('04-07')
  })

  test('calendarDayKey returns null for missing or invalid values', () => {
    expect(calendarDayKey(null)).toBeNull()
    expect(calendarDayKey(undefined)).toBeNull()
    expect(calendarDayKey('not-a-date')).toBeNull()
  })

  test('calendarDayKey returns null when the formatter omits month or day parts', () => {
    const formatToParts = vi.spyOn(
      Intl.DateTimeFormat.prototype,
      'formatToParts',
    )
    formatToParts.mockReturnValue([{ type: 'literal', value: '/' }])

    expect(calendarDayKey('2026-04-07T00:30:00.000Z')).toBeNull()
  })

  test('formatDuration handles edge cases', () => {
    expect(formatDuration(null)).toBe('0s')
    expect(formatDuration(undefined)).toBe('0s')
    expect(formatDuration(0)).toBe('0s')
    expect(formatDuration(-1000)).toBe('0s')
    expect(formatDuration(59_000)).toBe('59s')
    expect(formatDuration(125_000)).toBe('2m 5s')
  })

  test('formatBytes handles empty and scaled values', () => {
    expect(formatBytes(null)).toBe('0 B')
    expect(formatBytes(undefined)).toBe('0 B')
    expect(formatBytes(0)).toBe('0 B')
    expect(formatBytes(512)).toBe('512 B')
    expect(formatBytes(2048)).toBe('2 KB')
    expect(formatBytes(1_572_864)).toBe('1.5 MB')
  })

  test('formatBytes respects unit boundaries and integer cutovers', () => {
    expect(formatBytes(1024)).toBe('1 KB')
    expect(formatBytes(10 * 1024)).toBe('10 KB')
    expect(formatBytes(1024 ** 3)).toBe('1 GB')
    expect(formatBytes(1024 ** 4)).toBe('1 TB')
  })

  test('formatRelativeTime handles missing and recent values', () => {
    expect(formatRelativeTime(null)).toBe('Not yet')
    expect(formatRelativeTime(undefined)).toBe('Not yet')
    expect(formatRelativeTime(null, 'zh-CN')).toBe('尚未发生')
    expect(formatRelativeTime(undefined, 'zh-TW')).toBe('尚未發生')
    expect(formatRelativeTime('not-a-date')).toBe('not-a-date')
    expect(
      formatRelativeTime(new Date(Date.now() - 10 * 60_000).toISOString()),
    ).toMatch(/10/)
    expect(
      formatRelativeTime(new Date(Date.now() + 3 * 60 * 60_000).toISOString()),
    ).toMatch(/3/)
    expect(
      formatRelativeTime(new Date(Date.now() + 10 * 60_000).toISOString()),
    ).toMatch(/10/)
  })

  test('formatRelativeTime handles day-scale timestamps', () => {
    vi.spyOn(Date, 'now').mockReturnValue(
      new Date('2026-04-07T12:00:00.000Z').getTime(),
    )

    expect(formatRelativeTime('2026-04-10T12:00:00.000Z')).toMatch(/3/)
    expect(formatRelativeTime('2026-04-04T12:00:00.000Z')).toMatch(/3/)
  })

  test('formatRelativeTime switches units at exact minute, hour, and day boundaries', () => {
    vi.spyOn(Date, 'now').mockReturnValue(
      new Date('2026-04-07T12:00:00.000Z').getTime(),
    )

    const formatter = new Intl.RelativeTimeFormat(localeTag('en'), {
      numeric: 'auto',
    })

    expect(formatRelativeTime('2026-04-07T12:00:29.000Z')).toBe(
      formatter.format(0, 'second'),
    )
    expect(formatRelativeTime('2026-04-07T13:00:00.000Z')).toBe(
      formatter.format(1, 'hour'),
    )
    expect(formatRelativeTime('2026-04-07T11:00:00.000Z')).toBe(
      formatter.format(-1, 'hour'),
    )
    expect(formatRelativeTime('2026-04-09T12:00:00.000Z')).toBe(
      formatter.format(2, 'day'),
    )
    expect(formatRelativeTime('2026-04-05T12:00:00.000Z')).toBe(
      formatter.format(-2, 'day'),
    )
  })
})
