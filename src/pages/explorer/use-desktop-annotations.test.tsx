import { describe, expect, test, vi, beforeEach } from 'vitest'
import { act, renderHook, waitFor } from '@testing-library/react'

const getUrlAnnotation = vi.fn()
const setUrlNotes = vi.fn()
const replaceUrlTags = vi.fn()

vi.mock('../../lib/backend-client', () => ({
  backend: {
    getUrlAnnotation: (...args: unknown[]) =>
      getUrlAnnotation(...args) as unknown,
    setUrlNotes: (...args: unknown[]) => setUrlNotes(...args) as unknown,
    replaceUrlTags: (...args: unknown[]) => replaceUrlTags(...args) as unknown,
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

  test('hydration treats missing backend tags as an empty tag list', async () => {
    getUrlAnnotation.mockResolvedValue({
      url: 'https://example.com/no-tags',
      notes: 'remembered',
      tags: undefined,
      updatedAt: '2026-05-18T00:00:00Z',
      createdAt: '2026-05-17T00:00:00Z',
      sourceProfile: null,
    })
    const { result, rerender } = renderHook(() => useDesktopAnnotations())

    expect(result.current.tagsFor('https://example.com/no-tags')).toEqual([])
    await waitFor(() => {
      rerender()
      expect(result.current.notesFor('https://example.com/no-tags')).toBe(
        'remembered',
      )
    })

    expect(result.current.tagsFor('https://example.com/no-tags')).toEqual([])
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

  test('local edits survive a late hydration response (race guard)', async () => {
    // Set up a deferred hydration so we can interleave a local edit
    // before the backend GET resolves.
    let resolveHydrate:
      | ((value: { notes: string; tags: string[] } | null) => void)
      | null = null
    getUrlAnnotation.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveHydrate = resolve
      }),
    )
    setUrlNotes.mockResolvedValue({
      url: 'https://example.com/race',
      notes: 'my-edit',
      tags: [],
      updatedAt: '',
      createdAt: '',
    })
    const { result } = renderHook(() => useDesktopAnnotations())

    // Touch notesFor to trigger hydration kickoff.
    expect(result.current.notesFor('https://example.com/race')).toBe('')

    // User types before the GET returns.
    act(() => {
      result.current.updateNotes('https://example.com/race', 'my-edit')
    })
    expect(result.current.notesFor('https://example.com/race')).toBe('my-edit')

    // Backend GET finally returns a stale value. The hook must NOT
    // overwrite the user's optimistic edit.
    act(() => {
      resolveHydrate?.({ notes: 'STALE backend value', tags: [] })
    })
    await waitFor(() => {
      expect(getUrlAnnotation).toHaveBeenCalled()
    })
    expect(result.current.notesFor('https://example.com/race')).toBe('my-edit')
  })

  test('setUrlNotes failure surfaces via lastError', async () => {
    getUrlAnnotation.mockResolvedValue(null)
    setUrlNotes.mockRejectedValueOnce(new Error('archive locked'))
    const { result } = renderHook(() => useDesktopAnnotations())
    act(() => {
      result.current.updateNotes('https://example.com/locked', 'edit')
    })
    await waitFor(() => {
      expect(result.current.lastError).not.toBeNull()
    })
    expect(result.current.lastError).toContain('notes')
    expect(result.current.lastError).toContain('archive locked')
  })

  test('replaceUrlTags failure surfaces via lastError with "tags" scope', async () => {
    getUrlAnnotation.mockResolvedValue(null)
    replaceUrlTags.mockRejectedValueOnce(new Error('write blocked'))
    const { result } = renderHook(() => useDesktopAnnotations())
    act(() => {
      result.current.updateTags('https://example.com/locked', ['x'])
    })
    await waitFor(() => {
      expect(result.current.lastError).not.toBeNull()
    })
    expect(result.current.lastError).toContain('tags')
    expect(result.current.lastError).toContain('write blocked')
  })
})
