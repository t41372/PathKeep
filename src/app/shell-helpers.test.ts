/**
 * Coverage tests for the shell-helper utilities.
 *
 * ## Responsibilities
 * - Pin the storage-read fallback paths (missing storage, missing entries,
 *   parse failures) so behavior under SSR / privacy-mode browsers stays
 *   predictable.
 * - Exercise epigraph daily-rotation: stored-but-stale, stored-and-fresh,
 *   missing entirely.
 * - Pin byte humanizer + storage summation + URL hostname extraction +
 *   timestamp formatters across happy + bail-out paths.
 */

import { describe, expect, test, vi } from 'vitest'
import type { StorageSummary } from '@/lib/types'
import {
  extractDomain,
  formatLastArchivedLabel,
  formatSinceLabel,
  humanizeBytes,
  readBoolean,
  readEpigraphIndex,
  readTheme,
  sumStorageBytes,
} from './shell-helpers'

function makeStorage(initial: Record<string, string> = {}): Storage {
  const map = new Map<string, string>(Object.entries(initial))
  return {
    get length() {
      return map.size
    },
    clear: () => map.clear(),
    getItem: (key: string) => map.get(key) ?? null,
    key: (index: number) => Array.from(map.keys())[index] ?? null,
    removeItem: (key: string) => map.delete(key),
    setItem: (key: string, value: string) => {
      map.set(key, value)
    },
  }
}

function makeThrowingStorage(): Storage {
  return {
    get length() {
      return 0
    },
    clear: () => {
      throw new Error('storage disabled')
    },
    getItem: () => {
      throw new Error('storage disabled')
    },
    key: () => null,
    removeItem: () => {
      throw new Error('storage disabled')
    },
    setItem: () => {
      throw new Error('storage disabled')
    },
  }
}

const t = (key: string, vars?: Record<string, string | number>): string =>
  vars ? `${key} ${JSON.stringify(vars)}` : key

describe('readBoolean', () => {
  test('returns the fallback when storage is unavailable', () => {
    expect(readBoolean('k', true, null)).toBe(true)
    expect(readBoolean('k', false, null)).toBe(false)
  })

  test('returns the stored boolean when present', () => {
    expect(readBoolean('k', false, makeStorage({ k: 'true' }))).toBe(true)
    expect(readBoolean('k', true, makeStorage({ k: 'false' }))).toBe(false)
  })

  test('returns the fallback when the stored value is not a boolean string', () => {
    expect(readBoolean('k', true, makeStorage({ k: 'banana' }))).toBe(true)
    expect(readBoolean('k', false, makeStorage())).toBe(false)
  })

  test('returns the fallback when storage throws', () => {
    expect(readBoolean('k', true, makeThrowingStorage())).toBe(true)
  })
})

describe('readTheme', () => {
  test("returns 'light' when storage is unavailable", () => {
    expect(readTheme('k', null)).toBe('light')
  })

  test('returns the stored theme when present', () => {
    expect(readTheme('k', makeStorage({ k: 'dark' }))).toBe('dark')
    expect(readTheme('k', makeStorage({ k: 'light' }))).toBe('light')
  })

  test('returns light when the stored value is not a theme', () => {
    expect(readTheme('k', makeStorage({ k: 'lemon' }))).toBe('light')
  })

  test('returns light when storage throws', () => {
    expect(readTheme('k', makeThrowingStorage())).toBe('light')
  })
})

describe('readEpigraphIndex', () => {
  const reference = new Date('2026-05-20T12:00:00Z')

  test('returns 0 when storage is unavailable', () => {
    expect(readEpigraphIndex('k', 6, null, reference)).toBe(0)
  })

  test('returns the stored index when the date matches', () => {
    const storage = makeStorage({ k: '2026-05-20:4' })
    expect(readEpigraphIndex('k', 6, storage, reference)).toBe(4)
  })

  test('rotates when the stored date is stale', () => {
    const storage = makeStorage({ k: '2025-01-01:9' })
    const rng = vi.fn(() => 0.5)
    const result = readEpigraphIndex('k', 6, storage, reference, rng)
    expect(result).toBe(3)
    expect(storage.getItem('k')).toBe('2026-05-20:3')
    expect(rng).toHaveBeenCalled()
  })

  test('rotates when the stored value is malformed', () => {
    const storage = makeStorage({ k: '2026-05-20:not-a-number' })
    const result = readEpigraphIndex('k', 6, storage, reference, () => 0)
    expect(result).toBe(0)
  })

  test('rotates when the stored value omits the index separator', () => {
    const storage = makeStorage({ k: '2026-05-20' })
    const result = readEpigraphIndex('k', 6, storage, reference, () => 0.25)
    expect(result).toBe(1)
    expect(storage.getItem('k')).toBe('2026-05-20:1')
  })

  test('rotates when no value has been stored yet', () => {
    const storage = makeStorage()
    const result = readEpigraphIndex('k', 6, storage, reference, () => 0.99)
    expect(result).toBe(5)
    expect(storage.getItem('k')).toBe('2026-05-20:5')
  })

  test('returns 0 when storage throws', () => {
    expect(readEpigraphIndex('k', 6, makeThrowingStorage(), reference)).toBe(0)
  })
})

describe('extractDomain', () => {
  test('returns empty for missing input', () => {
    expect(extractDomain(undefined)).toBe('')
    expect(extractDomain('')).toBe('')
  })

  test('returns the hostname for a valid URL', () => {
    expect(extractDomain('https://example.com/foo')).toBe('example.com')
  })

  test('returns empty for malformed URLs', () => {
    expect(extractDomain('not a url')).toBe('')
  })
})

describe('sumStorageBytes', () => {
  test('returns 0 when storage is missing', () => {
    expect(sumStorageBytes(undefined)).toBe(0)
  })

  test('sums every byte bucket', () => {
    const storage: StorageSummary = {
      archiveDatabaseBytes: 1,
      sourceEvidenceDatabaseBytes: 2,
      searchDatabaseBytes: 4,
      intelligenceDatabaseBytes: 8,
      manifestBytes: 16,
      snapshotBytes: 32,
      exportBytes: 64,
      stagingBytes: 128,
      quarantineBytes: 256,
      semanticSidecarBytes: 512,
      intelligenceBlobBytes: 1024,
    }
    expect(sumStorageBytes(storage)).toBe(2047)
  })
})

describe('humanizeBytes', () => {
  test('returns empty for zero or negative input', () => {
    expect(humanizeBytes(0)).toBe('')
    expect(humanizeBytes(-1)).toBe('')
  })

  test('formats bytes through kilo / mega / giga / tera scales', () => {
    expect(humanizeBytes(900)).toBe('900 B')
    expect(humanizeBytes(1024)).toBe('1.0 KB')
    expect(humanizeBytes(1024 * 1024)).toBe('1.0 MB')
    expect(humanizeBytes(1024 * 1024 * 1024)).toBe('1.0 GB')
    expect(humanizeBytes(1024 * 1024 * 1024 * 1024)).toBe('1.0 TB')
  })

  test('clamps oversized values to TB without overflowing the unit list', () => {
    expect(humanizeBytes(1024 ** 5)).toBe('1024.0 TB')
  })
})

describe('formatSinceLabel', () => {
  test("returns '' for malformed timestamps", () => {
    expect(formatSinceLabel('not a date', t, 'en')).toBe('')
  })

  test('passes locale + month/year vars to the translator', () => {
    const result = formatSinceLabel('2026-04-15T00:00:00Z', t, 'en')
    expect(result.startsWith('shell.since ')).toBe(true)
    expect(result).toMatch(/"year":2026/)
  })

  test('uses undefined locale when language is "system"', () => {
    expect(formatSinceLabel('2026-04-15T00:00:00Z', t, 'system')).toMatch(
      /"year":2026/,
    )
  })

  test("returns '' when toLocaleString throws on a malformed BCP-47 tag", () => {
    // Single-hyphen locale → RangeError from Intl. The try/catch must
    // fall through to the empty string.
    expect(formatSinceLabel('2026-04-15T00:00:00Z', t, '-')).toBe('')
  })
})

describe('formatLastArchivedLabel', () => {
  test("returns '' for malformed timestamps", () => {
    expect(formatLastArchivedLabel('garbage', t, 'en')).toBe('')
  })

  test('passes locale + 2-digit hour/minute vars to the translator', () => {
    const result = formatLastArchivedLabel('2026-04-15T08:09:00Z', t, 'en')
    expect(result.startsWith('shell.lastArchivedAt ')).toBe(true)
  })

  test('uses undefined locale when language is "system"', () => {
    expect(
      formatLastArchivedLabel('2026-04-15T08:09:00Z', t, 'system'),
    ).toMatch(/shell.lastArchivedAt/)
  })

  test("returns '' when toLocaleTimeString throws on a malformed BCP-47 tag", () => {
    expect(formatLastArchivedLabel('2026-04-15T08:09:00Z', t, '-')).toBe('')
  })
})
