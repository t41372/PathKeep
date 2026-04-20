import { describe, expect, test } from 'vitest'
import {
  buildDayInsightsSearchParams,
  buildIntelligenceSearchParams,
  formatLocalDateKey,
  isLocalDateKey,
  localDateKeyFromIso,
  parseInsightRouteFocus,
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

  test('persists shared focus params for promoted routes', () => {
    expect(
      buildIntelligenceSearchParams({
        dateRange: { start: '2026-04-01', end: '2026-04-30' },
        preset: 'custom',
        profileId: 'chrome:Default',
        focus: {
          focusType: 'compare-set',
          focusId: 'compare:trail-1:docs_page',
        },
      }).toString(),
    ).toBe(
      'range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=compare%3Atrail-1%3Adocs_page',
    )
    expect(
      buildDayInsightsSearchParams('chrome:Default', {
        focusType: 'path-flow',
        focusId: 'flow:chrome:Default:docs',
      }).toString(),
    ).toBe(
      'profileId=chrome%3ADefault&focusType=path-flow&focusId=flow%3Achrome%3ADefault%3Adocs',
    )
  })

  test('parses supported focus params and drops invalid values', () => {
    expect(
      parseInsightRouteFocus(
        new URLSearchParams('focusType=compare-set&focusId=compare:1'),
      ),
    ).toEqual({
      focusType: 'compare-set',
      focusId: 'compare:1',
    })
    expect(
      parseInsightRouteFocus(
        new URLSearchParams('focusType=invalid&focusId=compare:1'),
      ),
    ).toBeNull()
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
