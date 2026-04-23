/**
 * Keeps Explorer display text from leaking callback parameters or email-like strings into titles.
 */

import { describe, expect, test } from 'vitest'
import {
  buildHistoryPrefetchPages,
  historyFaviconLookupKey,
  sanitizeExplorerDisplayText,
} from './helpers'

describe('sanitizeExplorerDisplayText', () => {
  test('redacts token-like query params by collapsing URLs to safe host/path output', () => {
    expect(
      sanitizeExplorerDisplayText(
        'https://example.com/callback?code=secret&token=secret&email=test@example.com',
      ),
    ).toBe('example.com/callback')
  })

  test('redacts plain email-like strings outside URLs', () => {
    expect(
      sanitizeExplorerDisplayText('Follow up with test@example.com about auth'),
    ).toBe('Follow up with … about auth')
  })
})

describe('buildHistoryPrefetchPages', () => {
  test('biases forward pages first when the user is paging forward', () => {
    expect(buildHistoryPrefetchPages(4, 10, 3, 'forward')).toEqual([
      5, 6, 7, 3, 2, 1,
    ])
  })

  test('biases backward pages first when the user is paging backward', () => {
    expect(buildHistoryPrefetchPages(4, 10, 3, 'backward')).toEqual([
      3, 2, 1, 5, 6, 7,
    ])
  })

  test('keeps the neutral nearest-page order for first load', () => {
    expect(buildHistoryPrefetchPages(4, 10, 2, 'neutral')).toEqual([3, 5, 2, 6])
  })
})

describe('historyFaviconLookupKey', () => {
  test('keeps profile and url in the cache identity', () => {
    expect(
      historyFaviconLookupKey('chrome:Default', 'https://example.com/path'),
    ).toBe('chrome:Default\nhttps://example.com/path')
  })
})
