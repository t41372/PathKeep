import { afterEach, describe, expect, test, vi } from 'vitest'
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

  test('calendarDayKey honors the requested timezone for day boundaries', () => {
    const iso = '2026-04-07T00:30:00.000Z'

    expect(calendarDayKey(iso, 'UTC')).toBe('04-07')
    expect(calendarDayKey(iso, 'America/Phoenix')).toBe('04-06')
    expect(calendarDayKey(iso, 'Asia/Tokyo')).toBe('04-07')
  })

  test('calendarDayKey returns null for missing or invalid values', () => {
    expect(calendarDayKey(null)).toBeNull()
    expect(calendarDayKey(undefined)).toBeNull()
    expect(calendarDayKey('not-a-date')).toBeNull()
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
})
