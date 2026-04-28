import { describe, expect, test } from 'vitest'
import type { CoreIntelligenceSectionMeta } from '../../../lib/core-intelligence'
import {
  firstSectionMeta,
  formatDuration,
  formatHourRange,
  formatIsoDate,
  formatNumber,
  singleDayRange,
} from './shared'

describe('intelligence section shared helpers', () => {
  test('returns the first available section metadata', () => {
    const meta = metaFixture('search-activity')

    expect(firstSectionMeta(null, undefined, { meta })).toBe(meta)
    expect(firstSectionMeta(null, undefined)).toBeNull()
  })

  test('builds single-day ranges and compact formatting labels', () => {
    expect(singleDayRange('2026-04-25')).toEqual({
      start: '2026-04-25',
      end: '2026-04-25',
    })
    expect(formatDuration(999)).toBe('999ms')
    expect(formatDuration(1200)).toBe('1.2s')
    expect(formatDuration(90_000)).toBe('1.5m')
    expect(formatDuration(7_200_000)).toBe('2.0h')
    expect(formatHourRange(23)).toBe('23:00-00:00')
    expect(formatIsoDate('2026-04-25T12:34:56Z')).toBe('2026-04-25')
    expect(formatNumber(42)).toBe('42')
    expect(formatNumber(1500)).toBe('1.5K')
    expect(formatNumber(2_500_000)).toBe('2.5M')
  })
})

function metaFixture(sectionId: string): CoreIntelligenceSectionMeta {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    includesEnrichment: false,
    moduleIds: ['test'],
    notes: [],
    sectionId,
    sourceTables: ['test_table'],
    state: 'ready',
    stateReason: null,
    window: {
      dateRange: {
        start: '2026-04-01',
        end: '2026-04-30',
      },
      kind: 'date-range',
    },
  }
}
