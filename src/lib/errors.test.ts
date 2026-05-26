import { describe, expect, test } from 'vitest'
import { describeError } from './errors'

describe('describeError', () => {
  test('returns the message of an Error instance', () => {
    expect(describeError(new Error('boom'))).toBe('boom')
  })

  test('falls back to the Error class name when message is empty', () => {
    class TimeoutError extends Error {
      constructor() {
        super('')
        this.name = 'TimeoutError'
      }
    }
    expect(describeError(new TimeoutError())).toBe('TimeoutError')
  })

  test('passes through a non-empty string', () => {
    expect(describeError('disk is full')).toBe('disk is full')
  })

  test('extracts a message field from a Tauri-style plugin error object', () => {
    expect(describeError({ message: 'permission denied' })).toBe(
      'permission denied',
    )
  })

  test('extracts an error field when message is absent', () => {
    expect(describeError({ error: 'rust panic: index out of bounds' })).toBe(
      'rust panic: index out of bounds',
    )
  })

  test('JSON-stringifies objects when no known message field exists', () => {
    expect(describeError({ kind: 'NotFound', path: '/tmp/x' })).toBe(
      '{"kind":"NotFound","path":"/tmp/x"}',
    )
  })

  test('handles cyclic objects by falling back through to a type label', () => {
    const cyclic: Record<string, unknown> = {}
    cyclic.self = cyclic
    const out = describeError(cyclic, 'load failed')
    // describeObject returns null on JSON failure, falls through to
    // String(object) which is "[object Object]" — that case is caught
    // and replaced with the context-aware type label.
    expect(out).toBe('load failed: <object>')
  })

  test('describes null and undefined explicitly', () => {
    expect(describeError(null)).toBe('null')
    expect(describeError(undefined)).toBe('undefined')
    expect(describeError(null, 'export')).toBe('export: null')
  })

  test('describes numbers and booleans verbatim', () => {
    expect(describeError(42)).toBe('42')
    expect(describeError(false)).toBe('false')
  })

  test('clips extremely long strings so banners stay readable', () => {
    const long = 'x'.repeat(5_000)
    const out = describeError(long)
    expect(out.length).toBeLessThanOrEqual(2_000)
    expect(out.endsWith('…')).toBe(true)
  })

  test('returns the message even when context is provided', () => {
    expect(describeError(new Error('boom'), 'while exporting')).toBe('boom')
  })
})
