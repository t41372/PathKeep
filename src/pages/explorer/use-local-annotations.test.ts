/**
 * Tests for useLocalAnnotations.
 *
 * The hook is small but the localStorage round-trip and the missing-key
 * fallbacks deserve direct coverage so the Explorer route doesn't drift
 * when the backend annotations module replaces it.
 */

import { renderHook, act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'
import { useLocalAnnotations } from './use-local-annotations'

const NOTES_KEY = 'pk.notes'
const TAGS_KEY = 'pk.tags'

describe('useLocalAnnotations', () => {
  beforeEach(() => {
    globalThis.localStorage?.clear()
  })

  afterEach(() => {
    globalThis.localStorage?.clear()
  })

  test('returns empty defaults when storage is fresh', () => {
    const { result } = renderHook(() => useLocalAnnotations())
    expect(result.current.notesFor('any-url')).toBe('')
    expect(result.current.tagsFor('any-url')).toEqual([])
  })

  test('writes notes through to localStorage and reads them back on remount', () => {
    const { result, unmount } = renderHook(() => useLocalAnnotations())
    act(() => {
      result.current.updateNotes('https://example.test', 'meaningful note')
    })
    expect(result.current.notesFor('https://example.test')).toBe(
      'meaningful note',
    )
    expect(globalThis.localStorage.getItem(NOTES_KEY)).toContain(
      'meaningful note',
    )

    unmount()
    const remount = renderHook(() => useLocalAnnotations())
    expect(remount.result.current.notesFor('https://example.test')).toBe(
      'meaningful note',
    )
  })

  test('writes tags through to localStorage and reads them back on remount', () => {
    const { result, unmount } = renderHook(() => useLocalAnnotations())
    act(() => {
      result.current.updateTags('https://example.test', ['rust', 'async'])
    })
    expect(result.current.tagsFor('https://example.test')).toEqual([
      'rust',
      'async',
    ])
    expect(globalThis.localStorage.getItem(TAGS_KEY)).toContain('rust')

    unmount()
    const remount = renderHook(() => useLocalAnnotations())
    expect(remount.result.current.tagsFor('https://example.test')).toEqual([
      'rust',
      'async',
    ])
  })

  test('null / undefined key resolves to safe defaults', () => {
    const { result } = renderHook(() => useLocalAnnotations())
    expect(result.current.notesFor(null)).toBe('')
    expect(result.current.notesFor(undefined)).toBe('')
    expect(result.current.tagsFor(null)).toEqual([])
    expect(result.current.tagsFor(undefined)).toEqual([])
  })

  test('corrupt localStorage payload is recovered to empty maps', () => {
    globalThis.localStorage.setItem(NOTES_KEY, '{not-json')
    globalThis.localStorage.setItem(TAGS_KEY, '"not an object"')
    const { result } = renderHook(() => useLocalAnnotations())
    expect(result.current.notesFor('https://example.test')).toBe('')
    expect(result.current.tagsFor('https://example.test')).toEqual([])
  })

  test('separate URLs keep separate annotations', () => {
    const { result } = renderHook(() => useLocalAnnotations())
    act(() => {
      result.current.updateNotes('url-a', 'note A')
      result.current.updateNotes('url-b', 'note B')
      result.current.updateTags('url-a', ['tag-a'])
      result.current.updateTags('url-b', ['tag-b'])
    })
    expect(result.current.notesFor('url-a')).toBe('note A')
    expect(result.current.notesFor('url-b')).toBe('note B')
    expect(result.current.tagsFor('url-a')).toEqual(['tag-a'])
    expect(result.current.tagsFor('url-b')).toEqual(['tag-b'])
  })
})
