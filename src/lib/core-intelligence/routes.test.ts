import { describe, expect, test } from 'vitest'
import {
  buildDayInsightsSearchParams,
  buildIntelligenceSearchParams,
  formatLocalDateKey,
  isLocalDateKey,
  localDateKeyFromIso,
  singleDayDateRange,
} from './routes'

describe('core intelligence routes helpers', () => {
  test('builds intelligence search params for shared domain routes', () => {
    expect(
      buildIntelligenceSearchParams({
        dateRange: { start: '2026-04-01', end: '2026-04-30' },
        preset: 'custom',
        profileId: 'chrome:Default',
      }).toString(),
    ).toBe(
      'range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault',
    )
  })

  test('builds compact day-insights search params', () => {
    expect(buildDayInsightsSearchParams('chrome:Default').toString()).toBe(
      'profileId=chrome%3ADefault',
    )
    expect(buildDayInsightsSearchParams(null).toString()).toBe('')
  })

  test('validates and formats local date keys', () => {
    expect(isLocalDateKey('2026-04-18')).toBe(true)
    expect(isLocalDateKey('2026-02-30')).toBe(false)
    expect(formatLocalDateKey(new Date(2026, 3, 18))).toBe('2026-04-18')
  })

  test('derives local day ranges from ISO timestamps', () => {
    expect(localDateKeyFromIso('2026-04-18T12:00:00Z')).toBe('2026-04-18')
    expect(singleDayDateRange('2026-04-18')).toEqual({
      start: '2026-04-18',
      end: '2026-04-18',
    })
  })
})
