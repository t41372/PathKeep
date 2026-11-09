/**
 * Responsibilities: preserve typed words while composing the backend's query syntax.
 * Not responsible for: parsing queries; there is no TypeScript query parser, so
 * these assertions check the produced string. Rust tests check how it is read.
 *
 * Ways this can fail:
 * - Any operator or alias in any of the four word fields becomes a filter.
 * - A minus typed in "all" or "any" becomes an exclusion or breaks an OR
 *   chain ("none" already means it: the word fields drop it).
 * - "None" negates an operator instead of excluding the literal text.
 * - Typed OR becomes syntax, or an operator-looking word breaks the "any" run.
 * - Whole phrases, CJK, curly quotes, backslashes or quotes lose their text.
 * - Empty or unclosed quotes leave syntax behind or split a phrase.
 * - A field split unlike the parser splits it: a backslash-escaped quote
 *   inside a phrase, a quote inside a word, a phrase after a minus.
 * - Quoting words changes the dedicated filter fields or tag links.
 */
import { describe, expect, it } from 'vitest'
import { tagQuery } from './tags'
import { composeWordsQuery, emptyWordsForm } from './words-query'

const operatorWords = [
  'site:github.com',
  'intitle:manual',
  'title:manual',
  'inurl:manual',
  'url:manual',
  'filetype:pdf',
  'ext:pdf',
  'tag:reading',
  'note:rust',
  'after:2026-05-07',
  'before:2026-05-07',
  'is:starred',
]
const fields = ['all', 'any', 'phrase', 'none'] as const

describe.each(fields)('%s words', (field) => {
  it.each([...operatorWords, 'Note:會議', 'OR'])(
    'keeps %s literal',
    (value) => {
      const query = composeWordsQuery({ ...emptyWordsForm, [field]: value })
      expect(query).toBe(`${field === 'none' ? '-' : ''}"${value}"`)
    },
  )
})

describe.each(['all', 'any', 'none'] as const)(
  'a minus typed in %s',
  (field) => {
    const sign = field === 'none' ? '-' : ''
    it.each([
      ['-foo', `${sign}foo`],
      ['--foo', `${sign}foo`],
      ['-site:x', `${sign}"site:x"`],
      ['-', ''],
    ])('%s is the word, %s', (value, query) => {
      expect(composeWordsQuery({ ...emptyWordsForm, [field]: value })).toBe(
        query,
      )
    })
  },
)

it.each(['-foo', '-site:x'])('an exact phrase keeps its minus: %s', (value) => {
  expect(composeWordsQuery({ ...emptyWordsForm, phrase: value })).toBe(
    `"${value}"`,
  )
})

it('keeps syntax-looking alternatives in one OR run', () => {
  expect(
    composeWordsQuery({
      ...emptyWordsForm,
      all: 'required',
      any: 'site:x -foo fallback',
      none: 'note:rust -bar',
    }),
  ).toBe('required "site:x" OR foo OR fallback -"note:rust" -bar')
})

it.each(fields)('keeps quoted phrases in %s', (field) => {
  expect(
    composeWordsQuery({ ...emptyWordsForm, [field]: '"Note: 會議"' }),
  ).toBe(
    field === 'phrase'
      ? '"\\"Note: 會議\\""'
      : `${field === 'none' ? '-' : ''}"Note: 會議"`,
  )
})

it('escapes the characters that could end an exact phrase', () => {
  expect(
    composeWordsQuery({ ...emptyWordsForm, phrase: 'site:"x" “會議” \\docs' }),
  ).toBe('"site:\\"x\\" \\“會議\\” \\\\docs"')
})

it('keeps an unclosed field quote as one phrase and skips empty quotes', () => {
  expect(composeWordsQuery({ ...emptyWordsForm, all: '"" "site:x y' })).toBe(
    '"site:x y"',
  )
  expect(composeWordsQuery(emptyWordsForm)).toBe('')
})

it('continues to quote operands after their dedicated operators', () => {
  expect(
    composeWordsQuery({
      ...emptyWordsForm,
      site: 'github.com',
      title: '"release notes"',
      url: 'pull',
      fileType: '.pdf',
      tags: 'a b',
      note: '會議 記錄',
      starred: true,
    }),
  ).toBe(
    'site:github.com intitle:"release notes" inurl:pull filetype:pdf tag:"a b" note:"會議 記錄" is:starred',
  )
  expect(tagQuery('a b')).toBe('tag:"a b"')
  expect(tagQuery('a "b"')).toBe('tag:"a \\"b\\""')
})

describe('splits a field as the parser splits a query', () => {
  it.each([
    // An escaped quote stays inside its phrase: one term, not three.
    [
      'any',
      '"release \\"candidate\\" notes" other',
      '"release \\"candidate\\" notes" OR other',
    ],
    [
      'none',
      '"release \\"candidate\\" notes"',
      '-"release \\"candidate\\" notes"',
    ],
    // Inside quotes a backslash escapes the next character, whatever it is.
    ['all', '"a\\b"', '"ab"'],
    // A quote inside a word is part of it, outside quotes a backslash too.
    ['all', 'don"t', '"don\\"t"'],
    ['all', 'a\\b', '"a\\\\b"'],
    // A quote closed inside a word ends the quoting, not the word.
    ['all', '"rust"lang', '"rustlang"'],
    // Spaces are Unicode White_Space, as Rust splits on: U+0085 splits, U+FEFF does not.
    ['any', 'alpha\u0085beta', 'alpha OR beta'],
    ['all', 'alpha\ufeffbeta', 'alpha\ufeffbeta'],
    // A minus before a phrase: the field drops the minus, the phrase stays one.
    ['any', '-"release notes" other', '"release notes" OR other'],
    ['none', '-"release notes"', '-"release notes"'],
  ] as const)('%s %s', (field, value, query) => {
    expect(composeWordsQuery({ ...emptyWordsForm, [field]: value })).toBe(query)
  })
})
