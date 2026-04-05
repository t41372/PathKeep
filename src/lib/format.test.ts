import { describe, expect, test } from 'vitest'
import { formatDateTime, formatDuration } from './format'

describe('format utilities', () => {
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

  test('formatDuration handles edge cases', () => {
    expect(formatDuration(null)).toBe('0s')
    expect(formatDuration(undefined)).toBe('0s')
    expect(formatDuration(0)).toBe('0s')
    expect(formatDuration(-1000)).toBe('0s')
    expect(formatDuration(59_000)).toBe('59s')
    expect(formatDuration(125_000)).toBe('2m 5s')
  })
})
