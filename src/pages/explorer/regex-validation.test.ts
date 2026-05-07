import { describe, expect, test } from 'vitest'
import { isRustRegexCompatible } from './regex-validation'

describe('Explorer Rust regex validation', () => {
  test('accepts ordinary Rust-compatible regex syntax', () => {
    expect(isRustRegexCompatible('archive\\sdocs')).toBe(true)
    expect(isRustRegexCompatible('^https://github\\.com/.+')).toBe(true)
    expect(isRustRegexCompatible('literal\\\\1')).toBe(true)
  })

  test('rejects JavaScript regex features unsupported by Rust regex', () => {
    expect(isRustRegexCompatible('^((?!pathkeep).)*$')).toBe(false)
    expect(isRustRegexCompatible('(?<=github)actions')).toBe(false)
    expect(isRustRegexCompatible('(github)\\1')).toBe(false)
    expect(isRustRegexCompatible('(?<word>github)')).toBe(false)
  })

  test('rejects baseline syntax errors', () => {
    expect(isRustRegexCompatible('archive(')).toBe(false)
  })
})
