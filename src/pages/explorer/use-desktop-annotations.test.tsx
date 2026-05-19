import { describe, expect, test, vi, beforeEach } from 'vitest'
import { act, renderHook, waitFor } from '@testing-library/react'

const getUrlAnnotation = vi.fn()
const setUrlNotes = vi.fn()
const replaceUrlTags = vi.fn()

vi.mock('../../lib/backend-client', () => ({
  backend: {
    getUrlAnnotation: (...args: unknown[]) => getUrlAnnotation(...args),
    setUrlNotes: (...args: unknown[]) => setUrlNotes(...args),
    replaceUrlTags: (...args: unknown[]) => replaceUrlTags(...args),
  },
}))

import { useDesktopAnnotations } from './use-desktop-annotations'

beforeEach(() => {
  getUrlAnnotation.mockReset()
  setUrlNotes.mockReset()
  replaceUrlTags.mockReset()
})

describe('useDesktopAnnotations', () => {
  test('notesFor returns the empty string until backend hydrates', async () => {
    getUrlAnnotation.mockResolvedValue({
      url: 'https://example.com/',
      notes: 'remembered',
      tags: ['rust'],
      updatedAt: '2026-05-18T00:00:00Z',
      createdAt: '2026-05-17T00:00:00Z',
      sourceProfile: null,
    })
    const { result, rerender } = renderHook(() => useDesktopAnnotations())
    // First synchronous read returns empty and triggers hydration.
    expect(result.current.notesFor('https://example.com/')).toBe('')
    await waitFor(() => {
      rerender()
      expect(result.current.notesFor('https://example.com/')).toBe('remembered')
    })
    expect(result.current.tagsFor('https://example.com/')).toEqual(['rust'])
    expect(getUrlAnnotation).toHaveBeenCalledTimes(1)
  })

  test('updateNotes writes through to the backend and updates the cache', () => {
    getUrlAnnotation.mockResolvedValue(null)
    setUrlNotes.mockResolvedValue({
      url: 'https://example.com/x',
      notes: 'why',
      tags: [],
      updatedAt: '',
      createdAt: '',
    })
    const { result } = renderHook(() => useDesktopAnnotations())
    act(() => {
      result.current.updateNotes('https://example.com/x', 'why')
    })
    expect(setUrlNotes).toHaveBeenCalledWith({
      url: 'https://example.com/x',
      notes: 'why',
    })
    expect(result.current.notesFor('https://example.com/x')).toBe('why')
  })

  test('updateTags writes through to the backend and updates the cache', () => {
    getUrlAnnotation.mockResolvedValue(null)
    replaceUrlTags.mockResolvedValue({
      url: 'https://example.com/y',
      notes: '',
      tags: ['design'],
      updatedAt: '',
      createdAt: '',
    })
    const { result } = renderHook(() => useDesktopAnnotations())
    act(() => {
      result.current.updateTags('https://example.com/y', ['design'])
    })
    expect(replaceUrlTags).toHaveBeenCalledWith({
      url: 'https://example.com/y',
      tags: ['design'],
    })
    expect(result.current.tagsFor('https://example.com/y')).toEqual(['design'])
  })

  test('backend hydration failure leaves the cache empty without throwing', async () => {
    getUrlAnnotation.mockRejectedValue(new Error('archive locked'))
    const { result } = renderHook(() => useDesktopAnnotations())
    expect(result.current.notesFor('https://example.com/locked')).toBe('')
    await waitFor(() => {
      expect(getUrlAnnotation).toHaveBeenCalled()
    })
    expect(result.current.notesFor('https://example.com/locked')).toBe('')
  })

  test('null key arguments return empty defaults without hitting backend', () => {
    const { result } = renderHook(() => useDesktopAnnotations())
    expect(result.current.notesFor(null)).toBe('')
    expect(result.current.tagsFor(undefined)).toEqual([])
    expect(getUrlAnnotation).not.toHaveBeenCalled()
  })
})
