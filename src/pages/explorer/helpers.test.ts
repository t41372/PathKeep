/**
 * Keeps Explorer display text from leaking callback parameters or email-like strings into titles.
 */

import type { KeyboardEvent } from 'react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import {
  activateRecordSelection,
  browserLabel,
  buildHistoryPrefetchPages,
  defaultKeywordPageSize,
  endOfDayMs,
  explorerPageSizeStorageKey,
  historyFaviconLookupKey,
  loadRecentSearches,
  loadStoredKeywordPageSize,
  parseKeywordPageSize,
  persistKeywordPageSize,
  recentSearchesStorageKey,
  sanitizeExplorerDisplayText,
} from './helpers'

beforeEach(() => {
  window.localStorage.clear()
})

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

  test('compacts unsafe non-url text and root urls', () => {
    expect(sanitizeExplorerDisplayText('https://www.example.com/', 24)).toBe(
      'example.com',
    )
    expect(sanitizeExplorerDisplayText('https://%', 24)).toBe('https://%')
    expect(
      sanitizeExplorerDisplayText(
        'not a url but it is long enough to compact',
        20,
      ),
    ).toContain('…')
    expect(sanitizeExplorerDisplayText('   ')).toBe('')
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
  test('keeps profile, url, and visit time in the cache identity', () => {
    expect(
      historyFaviconLookupKey(
        'chrome:Default',
        'https://example.com/path',
        1770000000000,
      ),
    ).toBe('chrome:Default\nhttps://example.com/path\n1770000000000')
    expect(
      historyFaviconLookupKey(
        'chrome:Default',
        'https://example.com/path',
        null,
      ),
    ).toBe('chrome:Default\nhttps://example.com/path\n0')
  })
})

describe('Explorer helper contracts', () => {
  test('loads legacy and structured recent searches defensively', () => {
    window.localStorage.setItem(
      recentSearchesStorageKey,
      JSON.stringify([
        'docs',
        {
          label: 'tickets',
          params: {
            q: 'tickets',
            sort: 'newest',
          },
        },
        { label: 'invalid' },
        null,
      ]),
    )

    expect(loadRecentSearches()).toEqual([
      {
        label: 'docs',
        params: {
          q: 'docs',
          sort: 'newest',
        },
      },
      {
        label: 'tickets',
        params: {
          q: 'tickets',
          sort: 'newest',
        },
      },
    ])

    window.localStorage.setItem(recentSearchesStorageKey, '{"not":"array"}')
    expect(loadRecentSearches()).toEqual([])
    window.localStorage.setItem(recentSearchesStorageKey, '{')
    expect(loadRecentSearches()).toEqual([])
    window.localStorage.removeItem(recentSearchesStorageKey)
    expect(loadRecentSearches()).toEqual([])

    vi.stubGlobal('window', undefined)
    expect(loadRecentSearches()).toEqual([])
    vi.unstubAllGlobals()
  })

  test('normalizes browser labels and persisted page sizes', () => {
    expect(browserLabel('chrome')).toBe('Chrome')
    expect(browserLabel('arc')).toBe('Arc')
    expect(browserLabel('firefox')).toBe('Firefox')
    expect(browserLabel('safari')).toBe('Safari')
    expect(browserLabel('comet')).toBe('comet')

    expect(parseKeywordPageSize('100')).toBe(100)
    expect(parseKeywordPageSize('999')).toBe(defaultKeywordPageSize)
    expect(loadStoredKeywordPageSize()).toBe(defaultKeywordPageSize)

    persistKeywordPageSize(200)
    expect(window.localStorage.getItem(explorerPageSizeStorageKey)).toBe('200')
    persistKeywordPageSize(999)
    expect(window.localStorage.getItem(explorerPageSizeStorageKey)).toBe(
      String(defaultKeywordPageSize),
    )

    vi.stubGlobal('window', undefined)
    expect(loadStoredKeywordPageSize()).toBe(defaultKeywordPageSize)
    persistKeywordPageSize(200)
    vi.unstubAllGlobals()
  })

  test('activates record selection only for keyboard action keys', () => {
    const onSelect = vi.fn()
    const preventDefault = vi.fn()

    activateRecordSelection(
      {
        key: 'ArrowDown',
        preventDefault,
      } as unknown as KeyboardEvent<HTMLDivElement>,
      onSelect,
    )
    expect(preventDefault).not.toHaveBeenCalled()
    expect(onSelect).not.toHaveBeenCalled()

    activateRecordSelection(
      {
        key: 'Enter',
        preventDefault,
      } as unknown as KeyboardEvent<HTMLDivElement>,
      onSelect,
    )
    activateRecordSelection(
      {
        key: ' ',
        preventDefault,
      } as unknown as KeyboardEvent<HTMLDivElement>,
      onSelect,
    )

    expect(preventDefault).toHaveBeenCalledTimes(2)
    expect(onSelect).toHaveBeenCalledTimes(2)
  })

  test('returns null for malformed end-of-day dates', () => {
    expect(endOfDayMs('not-a-date')).toBeNull()
  })
})
