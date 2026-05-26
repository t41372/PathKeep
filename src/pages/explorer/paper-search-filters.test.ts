/**
 * Tests for the Paper search filter helpers — tokeniser, active-chip
 * projection, append + remove primitives.
 */

import { describe, expect, test } from 'vitest'
import {
  appendOperator,
  parseActiveSearchFilters,
  removeFilterToken,
  tokenizeQuery,
} from './paper-search-filters'

describe('tokenizeQuery', () => {
  test('splits on whitespace and tracks substring positions', () => {
    const tokens = tokenizeQuery('rust async')
    expect(tokens).toEqual([
      { literal: 'rust', startIndex: 0, endIndex: 4 },
      { literal: 'async', startIndex: 5, endIndex: 10 },
    ])
  })

  test('keeps quoted phrases as a single token, including smart quotes and escapes', () => {
    const tokens = tokenizeQuery(
      '"release notes" tag:“async runtime” -note:legacy escape\\ slash',
    )
    const literals = tokens.map((token) => token.literal)
    expect(literals).toEqual([
      '"release notes"',
      'tag:“async runtime”',
      '-note:legacy',
      'escape\\',
      'slash',
    ])
  })

  test('returns no tokens for an empty or whitespace-only string', () => {
    expect(tokenizeQuery('')).toEqual([])
    expect(tokenizeQuery('   \t\n  ')).toEqual([])
  })
})

describe('parseActiveSearchFilters', () => {
  test('extracts tag / note operators (positive + negated) with display value', () => {
    const filters = parseActiveSearchFilters(
      'tag:rust -tag:archived note:"design doc" -note:legacy',
    )
    expect(
      filters.map(({ kind, value, negated, label }) => ({
        kind,
        value,
        negated,
        label,
      })),
    ).toEqual([
      { kind: 'tag', value: 'rust', negated: false, label: 'tag:rust' },
      { kind: 'tag', value: 'archived', negated: true, label: '-tag:archived' },
      {
        kind: 'note',
        value: 'design doc',
        negated: false,
        label: 'note:design doc',
      },
      { kind: 'note', value: 'legacy', negated: true, label: '-note:legacy' },
    ])
    // Each chip carries the tokenIndex so the panel can remove the exact
    // token even if the user typed two identical operators.
    expect(filters.map((filter) => filter.tokenIndex)).toEqual([0, 1, 2, 3])
  })

  test('extracts site, filetype, intitle, inurl, after, before; aliases collapse to canonical kind', () => {
    const filters = parseActiveSearchFilters(
      'site:github.com ext:pdf intitle:notes inurl:issues after:2026-05-01 before:2026',
    )
    expect(filters.map(({ kind, value }) => ({ kind, value }))).toEqual([
      { kind: 'site', value: 'github.com' },
      { kind: 'filetype', value: 'pdf' },
      { kind: 'intitle', value: 'notes' },
      { kind: 'inurl', value: 'issues' },
      { kind: 'after', value: '2026-05-01' },
      { kind: 'before', value: '2026' },
    ])
  })

  test('ignores empty operands, unknown operators, and non-operator tokens', () => {
    const filters = parseActiveSearchFilters(
      'tag: note: rust unknown:value 123:abc "release notes" -',
    )
    // tag: / note: → empty operand, skipped. unknown:value → not a known
    // operator. 123:abc → operator part not all-alphabetic. Plain words
    // and bare "-" are not filters.
    expect(filters).toEqual([])
  })

  test('builds an identity-based id (kind::neg/pos::value::occurrence) that survives unrelated query edits', () => {
    // Duplicates of the same (kind, negation, value) tuple get
    // disambiguated by occurrenceIndex.
    const filters = parseActiveSearchFilters('tag:rust tag:rust')
    expect(filters.map((filter) => filter.id)).toEqual([
      'tag::pos::rust::0',
      'tag::pos::rust::1',
    ])
    expect(filters.map((filter) => filter.occurrenceIndex)).toEqual([0, 1])

    // If unrelated tokens are added (e.g. user types a new operator
    // before the chip click commits), the original chip's id still
    // resolves the same active filter on re-parse.
    const reparsed = parseActiveSearchFilters(
      'note:foo tag:rust async tag:rust',
    )
    expect(
      reparsed.find((filter) => filter.id === 'tag::pos::rust::0'),
    ).toBeDefined()
    expect(
      reparsed.find((filter) => filter.id === 'tag::pos::rust::1'),
    ).toBeDefined()
  })
})

describe('appendOperator', () => {
  test('appends `op:` with a leading space when the query already has content', () => {
    expect(appendOperator('rust async', 'tag')).toBe('rust async tag:')
  })

  test('omits the leading space when the query is empty or trailing whitespace only', () => {
    expect(appendOperator('', 'tag')).toBe('tag:')
    expect(appendOperator('   ', 'note')).toBe('note:')
    expect(appendOperator('rust  ', 'tag')).toBe('rust tag:')
  })

  test('lowercases the operator and rejects junk operators silently', () => {
    expect(appendOperator('rust', 'TAG')).toBe('rust tag:')
    expect(appendOperator('rust', 'tag:value')).toBe('rust')
    expect(appendOperator('rust', '')).toBe('rust')
    expect(appendOperator('rust', '   ')).toBe('rust')
  })
})

describe('removeFilterToken', () => {
  test('strips the token at the supplied tokenIndex and collapses whitespace', () => {
    const query = 'rust tag:rust async note:legacy'
    expect(removeFilterToken(query, 1)).toBe('rust async note:legacy')
    expect(removeFilterToken(query, 3)).toBe('rust tag:rust async')
  })

  test('preserves quoted phrases on either side of the removed token', () => {
    const query = '"release notes" tag:rust -note:legacy'
    expect(removeFilterToken(query, 1)).toBe('"release notes" -note:legacy')
  })

  test('preserves whitespace inside surviving quoted phrases (does not collapse tabs / runs)', () => {
    // Regression for the BROWSE-VIRT code-review §1 finding: a naïve
    // `replace(/[ \t]+/g, ' ')` over the full stitched string would
    // rewrite the user's `\t` inside the quoted phrase to a single
    // space, making vault-core's exact-phrase match silently fail.
    const query = 'tag:rust "release\tnotes\there" note:legacy'
    expect(removeFilterToken(query, 0)).toBe(
      '"release\tnotes\there" note:legacy',
    )
  })

  test('drops leading whitespace when the removed token was at the start', () => {
    const query = 'tag:rust  rust async'
    expect(removeFilterToken(query, 0)).toBe('rust async')
  })

  test('drops trailing whitespace when the removed token was at the end', () => {
    const query = 'rust async  tag:rust'
    expect(removeFilterToken(query, 2)).toBe('rust async')
  })

  test('is a no-op when the index is out of range', () => {
    const query = 'rust tag:rust'
    expect(removeFilterToken(query, 99)).toBe('rust tag:rust')
    expect(removeFilterToken('', 0)).toBe('')
  })
})
