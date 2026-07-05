/**
 * Unit tests for the enriched-content helpers (W-ENRICH-1).
 *
 * The detail panel must render an honest state for every fetch outcome and must
 * never throw while parsing the opaque stored metadata. These tests cover the
 * status taxonomy, the forgiving metadata parse, and the best-record pick.
 */

import { describe, expect, test } from 'vitest'
import type { VisitEnrichmentRecord } from '@/lib/types'
import {
  enrichmentSourceKind,
  enrichmentStatusKind,
  parseEnrichmentMetadata,
  pickBestEnrichment,
  toEnrichmentView,
} from './paper-enriched-content-helpers'

function record(
  overrides: Partial<VisitEnrichmentRecord> = {},
): VisitEnrichmentRecord {
  return {
    contentSource: 'github-repo',
    fetchStatus: 'success',
    fetchedAt: '2026-06-21T00:00:00Z',
    ...overrides,
  }
}

describe('enrichmentSourceKind', () => {
  test('maps the built-in sources and falls back to unknown', () => {
    expect(enrichmentSourceKind('github-repo')).toBe('github')
    expect(enrichmentSourceKind('generic-readable')).toBe('generic')
    expect(enrichmentSourceKind('something-else')).toBe('unknown')
  })
})

describe('enrichmentStatusKind', () => {
  test('recognizes each honest status family case-insensitively', () => {
    expect(enrichmentStatusKind('success')).toBe('success')
    expect(enrichmentStatusKind('OK')).toBe('success')
    expect(enrichmentStatusKind('empty')).toBe('empty')
    expect(enrichmentStatusKind('blocked')).toBe('blocked')
    expect(enrichmentStatusKind('ssrf')).toBe('blocked')
    expect(enrichmentStatusKind('login-required')).toBe('login')
    expect(enrichmentStatusKind('paywall')).toBe('login')
    expect(enrichmentStatusKind('non-html')).toBe('unsupported')
    expect(enrichmentStatusKind('rate-limited')).toBe('rate-limited')
    expect(enrichmentStatusKind('throttled')).toBe('rate-limited')
  })

  test('treats an unrecognized non-success status as an error, never success', () => {
    expect(enrichmentStatusKind('weird-marker')).toBe('error')
    expect(enrichmentStatusKind('')).toBe('error')
  })
})

describe('parseEnrichmentMetadata', () => {
  test('returns empty fields for null / malformed / non-object JSON', () => {
    expect(parseEnrichmentMetadata(null)).toEqual({ topics: [] })
    expect(parseEnrichmentMetadata('not json')).toEqual({ topics: [] })
    expect(parseEnrichmentMetadata('[1,2,3]')).toEqual({ topics: [] })
    expect(parseEnrichmentMetadata('"a string"')).toEqual({ topics: [] })
  })

  test('extracts topics + description, dropping non-strings and blanks', () => {
    const parsed = parseEnrichmentMetadata(
      JSON.stringify({
        description: '  A repo  ',
        topics: ['rust', '', 42, '  cli  '],
      }),
    )
    expect(parsed.description).toBe('A repo')
    expect(parsed.topics).toEqual(['rust', 'cli'])
  })

  test('falls back to alternate keys (tags, repoDescription)', () => {
    const parsed = parseEnrichmentMetadata(
      JSON.stringify({ repoDescription: 'desc', tags: ['a'] }),
    )
    expect(parsed.description).toBe('desc')
    expect(parsed.topics).toEqual(['a'])
  })

  test('drops a whitespace-only description to undefined', () => {
    const parsed = parseEnrichmentMetadata(
      JSON.stringify({ description: '   ', topics: [] }),
    )
    expect(parsed.description).toBeUndefined()
  })
})

describe('toEnrichmentView', () => {
  test('builds an ok view from a successful github record', () => {
    const view = toEnrichmentView(
      record({
        readableTitle: 'owner/repo',
        summary: 'A summary',
        metadataJson: JSON.stringify({
          description: 'A repo',
          topics: ['rust'],
        }),
      }),
    )
    expect(view).toMatchObject({
      sourceKind: 'github',
      ok: true,
      statusKind: 'success',
      title: 'owner/repo',
      summary: 'A summary',
      description: 'A repo',
      topics: ['rust'],
    })
  })

  test('a non-success record is not ok and keeps its honest status', () => {
    const view = toEnrichmentView(
      record({ contentSource: 'generic-readable', fetchStatus: 'login' }),
    )
    expect(view.ok).toBe(false)
    expect(view.statusKind).toBe('login')
    expect(view.sourceKind).toBe('generic')
  })
})

describe('pickBestEnrichment', () => {
  test('returns null for an empty list', () => {
    expect(pickBestEnrichment([])).toBeNull()
  })

  test('prefers a successful row over a failure regardless of input order', () => {
    // success-first and failure-first both resolve to the success row, which
    // also exercises both arms of the ranking comparator.
    expect(
      pickBestEnrichment([
        record({ contentSource: 'github-repo', fetchStatus: 'error' }),
        record({ contentSource: 'generic-readable', fetchStatus: 'success' }),
      ])?.contentSource,
    ).toBe('generic-readable')
    expect(
      pickBestEnrichment([
        record({ contentSource: 'generic-readable', fetchStatus: 'success' }),
        record({ contentSource: 'github-repo', fetchStatus: 'error' }),
      ])?.contentSource,
    ).toBe('generic-readable')
  })

  test('among equal statuses, prefers the most recent fetch', () => {
    const best = pickBestEnrichment([
      record({ fetchStatus: 'success', fetchedAt: '2026-06-20T00:00:00Z' }),
      record({ fetchStatus: 'success', fetchedAt: '2026-06-21T00:00:00Z' }),
    ])
    expect(best?.fetchedAt).toBe('2026-06-21T00:00:00Z')
  })
})
