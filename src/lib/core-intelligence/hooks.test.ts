import { describe, expect, test } from 'vitest'
import { dateRangeForCalendarYear } from './hooks'

describe('core intelligence hooks', () => {
  test('builds an inclusive calendar-year range', () => {
    expect(dateRangeForCalendarYear(2024)).toEqual({
      start: '2024-01-01',
      end: '2024-12-31',
    })
  })
})
