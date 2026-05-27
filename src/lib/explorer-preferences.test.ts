/**
 * @file explorer-preferences.test.ts
 * @description Bounds coverage for the shared Explorer prefetch preference helper.
 * @module lib/explorer-preferences
 */

import { afterEach, describe, expect, test, vi } from 'vitest'
import {
  CLOCK_FORMAT_EVENT,
  defaultClockFormat,
  defaultExplorerBackgroundPrefetchPages,
  defaultExplorerViewMode,
  explorerBackgroundPrefetchPageOptions,
  maxExplorerBackgroundPrefetchPages,
  normalizeExplorerBackgroundPrefetchPages,
  persistClockFormat,
  persistExplorerViewMode,
  readClockFormat,
  readExplorerViewMode,
} from './explorer-preferences'

afterEach(() => {
  vi.unstubAllGlobals()
  window.localStorage.clear()
  vi.restoreAllMocks()
})

describe('Explorer background prefetch preferences', () => {
  test('normalizes invalid, low, high, and fractional values', () => {
    expect(normalizeExplorerBackgroundPrefetchPages(null)).toBe(
      defaultExplorerBackgroundPrefetchPages,
    )
    expect(normalizeExplorerBackgroundPrefetchPages(Number.NaN)).toBe(
      defaultExplorerBackgroundPrefetchPages,
    )
    expect(normalizeExplorerBackgroundPrefetchPages(-2)).toBe(0)
    expect(normalizeExplorerBackgroundPrefetchPages(99)).toBe(
      maxExplorerBackgroundPrefetchPages,
    )
    expect(normalizeExplorerBackgroundPrefetchPages(3.9)).toBe(3)
  })

  test('exposes one option for every allowed prefetch count', () => {
    expect(explorerBackgroundPrefetchPageOptions).toEqual([
      0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    ])
  })
})

// ── Browse view-mode persistence ──────────────────────────────────────

describe('readExplorerViewMode', () => {
  test('returns the cards default when window is unavailable', () => {
    withWindowUnavailable(() => {
      expect(readExplorerViewMode()).toBe(defaultExplorerViewMode)
    })
  })

  test('returns "cards" when localStorage is empty', () => {
    expect(readExplorerViewMode()).toBe('cards')
  })

  test('returns "list" when stored value is "list"', () => {
    window.localStorage.setItem('pathkeep.explorerViewMode', 'list')
    expect(readExplorerViewMode()).toBe('list')
  })

  test('returns "cards" for unrecognised stored values', () => {
    window.localStorage.setItem('pathkeep.explorerViewMode', 'grid')
    expect(readExplorerViewMode()).toBe('cards')
  })

  test('returns default when localStorage.getItem throws', () => {
    withThrowingLocalStorageGetItem(() => {
      expect(readExplorerViewMode()).toBe(defaultExplorerViewMode)
    })
  })
})

describe('persistExplorerViewMode', () => {
  test('does nothing when window is unavailable', () => {
    withWindowUnavailable(() => {
      expect(() => persistExplorerViewMode('list')).not.toThrow()
    })
  })

  test('writes mode to localStorage', () => {
    persistExplorerViewMode('list')
    expect(window.localStorage.getItem('pathkeep.explorerViewMode')).toBe(
      'list',
    )
  })

  test('skips write when current mode already matches', () => {
    window.localStorage.setItem('pathkeep.explorerViewMode', 'list')
    const spy = vi.spyOn(Storage.prototype, 'setItem')
    persistExplorerViewMode('list')
    expect(spy).not.toHaveBeenCalled()
  })

  test('swallows localStorage.setItem errors', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota exceeded')
    })
    expect(() => persistExplorerViewMode('list')).not.toThrow()
  })
})

// ── Clock format persistence ──────────────────────────────────────────

describe('readClockFormat', () => {
  test('returns the 12h default when window is unavailable', () => {
    withWindowUnavailable(() => {
      expect(readClockFormat()).toBe(defaultClockFormat)
    })
  })

  test('returns "12h" when localStorage is empty', () => {
    expect(readClockFormat()).toBe('12h')
  })

  test('returns "24h" when stored value is "24h"', () => {
    window.localStorage.setItem('pathkeep.clockFormat', '24h')
    expect(readClockFormat()).toBe('24h')
  })

  test('returns default for unrecognised stored values', () => {
    window.localStorage.setItem('pathkeep.clockFormat', 'military')
    expect(readClockFormat()).toBe(defaultClockFormat)
  })

  test('returns default when localStorage.getItem throws', () => {
    withThrowingLocalStorageGetItem(() => {
      expect(readClockFormat()).toBe(defaultClockFormat)
    })
  })
})

describe('persistClockFormat', () => {
  test('does nothing when window is unavailable', () => {
    withWindowUnavailable(() => {
      expect(() => persistClockFormat('24h')).not.toThrow()
    })
  })

  test('writes format to localStorage and dispatches event', () => {
    const events: string[] = []
    const listener = (e: Event) => {
      const detail = (e as CustomEvent<{ format: string }>).detail
      events.push(detail.format)
    }
    window.addEventListener(CLOCK_FORMAT_EVENT, listener)
    try {
      persistClockFormat('24h')
      expect(window.localStorage.getItem('pathkeep.clockFormat')).toBe('24h')
      expect(events).toEqual(['24h'])
    } finally {
      window.removeEventListener(CLOCK_FORMAT_EVENT, listener)
    }
  })

  test('skips write when current format already matches', () => {
    window.localStorage.setItem('pathkeep.clockFormat', '24h')
    const spy = vi.spyOn(Storage.prototype, 'setItem')
    const events: string[] = []
    const listener = (e: Event) => {
      const detail = (e as CustomEvent<{ format: string }>).detail
      events.push(detail.format)
    }
    window.addEventListener(CLOCK_FORMAT_EVENT, listener)
    try {
      persistClockFormat('24h')
      expect(spy).not.toHaveBeenCalled()
      expect(events).toEqual([])
    } finally {
      window.removeEventListener(CLOCK_FORMAT_EVENT, listener)
    }
  })

  test('swallows localStorage.setItem errors but still dispatches event', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota exceeded')
    })
    const events: string[] = []
    const listener = (e: Event) => {
      const detail = (e as CustomEvent<{ format: string }>).detail
      events.push(detail.format)
    }
    window.addEventListener(CLOCK_FORMAT_EVENT, listener)
    try {
      expect(() => persistClockFormat('24h')).not.toThrow()
      expect(events).toEqual(['24h'])
    } finally {
      window.removeEventListener(CLOCK_FORMAT_EVENT, listener)
    }
  })

  test('swallows CustomEvent dispatch errors', () => {
    const original = window.dispatchEvent.bind(window)
    window.dispatchEvent = vi.fn(() => {
      throw new Error('dispatchEvent unsupported')
    })
    try {
      expect(() => persistClockFormat('24h')).not.toThrow()
    } finally {
      window.dispatchEvent = original
    }
  })
})

function withWindowUnavailable(assertion: () => void) {
  const originalWindow = globalThis.window
  vi.stubGlobal('window', undefined)
  try {
    assertion()
  } finally {
    vi.stubGlobal('window', originalWindow)
  }
}

function withThrowingLocalStorageGetItem(assertion: () => void) {
  const originalWindow = globalThis.window
  vi.stubGlobal('window', {
    localStorage: {
      getItem: () => {
        throw new Error('storage disabled')
      },
    },
  })
  try {
    assertion()
  } finally {
    vi.stubGlobal('window', originalWindow)
  }
}
