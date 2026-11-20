/**
 * Byte offsets from the backend drawn as string indexes. Ways it can go
 * wrong: counting bytes as characters (CJK is 3 bytes, an emoji 4), cutting
 * a surrogate pair in half (an emoji is 2 UTF-16 units), an offset inside a
 * character, an offset past the end, and an empty or reversed range.
 */
import { describe, expect, it } from 'vitest'
import { utf8RangeToIndexes, utf8ToIndex } from './utf8-range'

describe('utf8RangeToIndexes', () => {
  it('finds the match in the contract example (CJK, emoji, ASCII)', () => {
    const text = '甲🙂 NEEDLE'
    const [from, to] = utf8RangeToIndexes(text, [8, 14])
    expect(text.slice(from, to)).toBe('NEEDLE')
  })

  it('keeps an emoji whole', () => {
    const text = 'a🙂b'
    const [from, to] = utf8RangeToIndexes(text, [1, 5])
    expect(text.slice(from, to)).toBe('🙂')
  })

  it('counts each CJK character as three bytes', () => {
    const text = '記帳本的匯出'
    const [from, to] = utf8RangeToIndexes(text, [12, 18])
    expect(text.slice(from, to)).toBe('匯出')
  })

  it('moves an offset inside a character to its end', () => {
    expect(utf8ToIndex('甲乙', 1)).toBe(1)
    expect(utf8ToIndex('🙂x', 2)).toBe(2)
  })

  it('clamps offsets before the start and past the end', () => {
    expect(utf8ToIndex('abc', -4)).toBe(0)
    expect(utf8ToIndex('abc', 99)).toBe(3)
  })

  it('never returns a range that ends before it starts', () => {
    const [from, to] = utf8RangeToIndexes('abcdef', [4, 2])
    expect(to).toBeGreaterThanOrEqual(from)
  })
})
