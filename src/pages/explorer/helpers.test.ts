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
  isSearchResultUrl,
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

describe('isSearchResultUrl', () => {
  test('matches Google / Bing / Baidu search-result pages', () => {
    expect(isSearchResultUrl('https://www.google.com/search?q=吉野家')).toBe(
      true,
    )
    expect(isSearchResultUrl('https://www.bing.com/search?q=pathkeep')).toBe(
      true,
    )
    expect(isSearchResultUrl('https://www.baidu.com/s?wd=foo&rsv_idx=1')).toBe(
      true,
    )
    expect(isSearchResultUrl('https://duckduckgo.com/?q=anything&ia=web')).toBe(
      true,
    )
  })

  test('does not match non-SERP URLs on the same hosts', () => {
    expect(isSearchResultUrl('https://www.google.com/')).toBe(false)
    expect(isSearchResultUrl('https://news.google.com/articles/abc')).toBe(
      false,
    )
    expect(isSearchResultUrl('https://www.bing.com/news')).toBe(false)
  })

  test('does not match non-SERP hosts even with a `q=` parameter', () => {
    expect(isSearchResultUrl('https://example.com/article?q=foo')).toBe(false)
    expect(isSearchResultUrl('https://github.com/search?q=pathkeep')).toBe(
      false,
    )
  })

  test('returns false for null / undefined / unparseable input', () => {
    expect(isSearchResultUrl(null)).toBe(false)
    expect(isSearchResultUrl(undefined)).toBe(false)
    expect(isSearchResultUrl('')).toBe(false)
    expect(isSearchResultUrl('not a url')).toBe(false)
  })

  test('returns false for URLs with an empty hostname (e.g. file://)', () => {
    // `new URL('file:///etc/hosts').hostname` is the empty string;
    // without the empty-host guard the SERP check would happily run on
    // a zero-length registrable and silently mis-bucket.
    expect(isSearchResultUrl('file:///etc/hosts?q=foo')).toBe(false)
  })

  test('uses public-suffix-aware registrable-domain detection for ccTLDs', () => {
    // Naive `slice(-2)` would yield `co.uk` and miss the SERP entirely;
    // the public-suffix-aware path collapses to `google.co.uk` and matches
    // via the `www.google.` prefix.
    expect(
      isSearchResultUrl('https://www.google.co.uk/search?q=pathkeep'),
    ).toBe(true)
    // ditto for a fake brand that LOOKS like google.co.uk but isn't a SERP
    expect(isSearchResultUrl('https://nope.co.uk/?q=foo')).toBe(false)
  })

  test('matches the broader Rust SEARCH_QUERY_KEYS list', () => {
    expect(isSearchResultUrl('https://www.baidu.com/s?word=sqlite')).toBe(true)
    expect(isSearchResultUrl('https://www.so.com/s?keyword=pathkeep')).toBe(
      true,
    )
  })

  test('does not match retired allowlist hosts (kagi/startpage) — Rust would say false', () => {
    expect(isSearchResultUrl('https://kagi.com/search?q=foo')).toBe(false)
    expect(isSearchResultUrl('https://startpage.com/do/search?q=foo')).toBe(
      false,
    )
  })
})
