/**
 * The dialect check against the case table the backend also runs
 * (`build_history_regex`'s `regex_dialect_cases` test): every pattern Rust
 * accepts must pass, every one it rejects must be stopped. A unit test because
 * the alternative, an E2E run per pattern, would take minutes for sixty cases.
 */
import { describe, expect, it } from 'vitest'
import cases from './regex-dialect-cases.json'
import { checkRustRegex } from './regex-dialect'

describe('checkRustRegex', () => {
  it.each(cases)('$pattern → accepted: $accepted', ({ pattern, accepted }) => {
    expect(checkRustRegex(pattern) === null).toBe(accepted)
  })

  it('names what it stops', () => {
    expect(checkRustRegex('tok(?=io)')).toEqual({ kind: 'lookAround' })
    expect(checkRustRegex('(?<!a)b')).toEqual({ kind: 'lookAround' })
    expect(checkRustRegex('(a)\\1')).toEqual({ kind: 'backreference' })
    expect(checkRustRegex('(?>a)')).toEqual({ kind: 'group' })
    expect(checkRustRegex('\\cA')).toEqual({ kind: 'escape', escape: '\\c' })
    expect(checkRustRegex('a{,5}')).toEqual({ kind: 'brace' })
    expect(checkRustRegex('(a')).toEqual({ kind: 'syntax' })
  })
})
