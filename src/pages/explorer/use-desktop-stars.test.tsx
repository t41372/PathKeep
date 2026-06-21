/**
 * Tests for the optimistic, batched desktop stars hook.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

const getStarStatus = vi.fn()
const setStar = vi.fn()
const unsetStar = vi.fn()

vi.mock('../../lib/backend-client', () => ({
  backend: {
    getStarStatus: (...args: unknown[]) => getStarStatus(...args) as unknown,
    setStar: (...args: unknown[]) => setStar(...args) as unknown,
    unsetStar: (...args: unknown[]) => unsetStar(...args) as unknown,
  },
}))

import { useDesktopStars } from './use-desktop-stars'

beforeEach(() => {
  getStarStatus.mockReset().mockResolvedValue({})
  setStar.mockReset().mockResolvedValue(undefined)
  unsetStar.mockReset().mockResolvedValue(undefined)
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('useDesktopStars', () => {
  test('isStarred defaults to false and ignores empty keys', () => {
    const { result } = renderHook(() => useDesktopStars())
    expect(result.current.isStarred('url', 'https://a.test/')).toBe(false)
    expect(result.current.isStarred('url', null)).toBe(false)
  })

  test('hydrate batches one call for unknown keys and reflects the result', async () => {
    getStarStatus.mockResolvedValue({ 'https://a.test/': true })
    const { result } = renderHook(() => useDesktopStars())
    act(() => {
      result.current.hydrate('url', ['https://a.test/', 'https://b.test/'])
    })
    await waitFor(() =>
      expect(result.current.isStarred('url', 'https://a.test/')).toBe(true),
    )
    expect(result.current.isStarred('url', 'https://b.test/')).toBe(false)
    expect(getStarStatus).toHaveBeenCalledWith({
      entityKind: 'url',
      entityKeys: ['https://a.test/', 'https://b.test/'],
    })
  })

  test('hydrate skips already-known keys (no second request) and empty input', async () => {
    getStarStatus.mockResolvedValue({ 'https://a.test/': true })
    const { result } = renderHook(() => useDesktopStars())
    act(() => result.current.hydrate('url', ['https://a.test/']))
    await waitFor(() => expect(getStarStatus).toHaveBeenCalledTimes(1))
    act(() => result.current.hydrate('url', ['https://a.test/']))
    act(() => result.current.hydrate('url', []))
    act(() => result.current.hydrate('url', ['']))
    expect(getStarStatus).toHaveBeenCalledTimes(1)
  })

  test('hydrate records lastError and allows retry when the batch fails', async () => {
    getStarStatus.mockRejectedValueOnce(new Error('boom'))
    const { result } = renderHook(() => useDesktopStars())
    act(() => result.current.hydrate('url', ['https://a.test/']))
    await waitFor(() => expect(result.current.lastError).toContain('hydrate'))
    // Failed keys are forgotten, so a later hydrate retries them.
    getStarStatus.mockResolvedValue({ 'https://a.test/': true })
    act(() => result.current.hydrate('url', ['https://a.test/']))
    await waitFor(() =>
      expect(result.current.isStarred('url', 'https://a.test/')).toBe(true),
    )
  })

  test('toggle optimistically stars and writes through with set_star', async () => {
    const { result } = renderHook(() => useDesktopStars())
    act(() => result.current.toggle('url', 'https://a.test/'))
    expect(result.current.isStarred('url', 'https://a.test/')).toBe(true)
    await waitFor(() =>
      expect(setStar).toHaveBeenCalledWith({
        entityKind: 'url',
        entityKey: 'https://a.test/',
      }),
    )
  })

  test('toggle from starred → unstarred writes unset_star', async () => {
    getStarStatus.mockResolvedValue({ 'https://a.test/': true })
    const { result } = renderHook(() => useDesktopStars())
    act(() => result.current.hydrate('url', ['https://a.test/']))
    await waitFor(() =>
      expect(result.current.isStarred('url', 'https://a.test/')).toBe(true),
    )
    act(() => result.current.toggle('url', 'https://a.test/'))
    expect(result.current.isStarred('url', 'https://a.test/')).toBe(false)
    await waitFor(() =>
      expect(unsetStar).toHaveBeenCalledWith({
        entityKind: 'url',
        entityKey: 'https://a.test/',
      }),
    )
  })

  test('toggle rolls back the optimistic flip and records lastError on failure', async () => {
    setStar.mockRejectedValueOnce(new Error('nope'))
    const { result } = renderHook(() => useDesktopStars())
    act(() => result.current.toggle('url', 'https://a.test/'))
    expect(result.current.isStarred('url', 'https://a.test/')).toBe(true)
    await waitFor(() =>
      expect(result.current.isStarred('url', 'https://a.test/')).toBe(false),
    )
    expect(result.current.lastError).toContain('toggle')
  })

  test('toggle ignores an empty key', () => {
    const { result } = renderHook(() => useDesktopStars())
    act(() => result.current.toggle('url', ''))
    expect(setStar).not.toHaveBeenCalled()
  })

  test('a failing UNSET rolls back to starred (true), not to false', async () => {
    // Regression: rollback must restore the PREVIOUS value, not blindly set
    // false. Hydrate true → unset rejects → the row must read true again.
    getStarStatus.mockResolvedValue({ 'https://a.test/': true })
    unsetStar.mockRejectedValueOnce(new Error('locked'))
    const { result } = renderHook(() => useDesktopStars())
    act(() => result.current.hydrate('url', ['https://a.test/']))
    await waitFor(() =>
      expect(result.current.isStarred('url', 'https://a.test/')).toBe(true),
    )
    // Optimistic un-star flips to false…
    act(() => result.current.toggle('url', 'https://a.test/'))
    expect(result.current.isStarred('url', 'https://a.test/')).toBe(false)
    // …and the rejection rolls it back to its previous TRUE, not to false.
    await waitFor(() =>
      expect(result.current.isStarred('url', 'https://a.test/')).toBe(true),
    )
    expect(result.current.lastError).toContain('toggle')
  })

  test('toggle marks the key known so a later hydrate skips it (no clobber)', async () => {
    // After an optimistic toggle the key is "known"; a subsequent hydrate for
    // the same key must NOT fetch (and therefore can't clobber the optimistic
    // value with a stale read).
    const { result } = renderHook(() => useDesktopStars())
    act(() => result.current.toggle('url', 'https://a.test/'))
    expect(result.current.isStarred('url', 'https://a.test/')).toBe(true)
    await waitFor(() => expect(setStar).toHaveBeenCalledTimes(1))
    // The hydrate finds the key already known and issues no request.
    act(() => result.current.hydrate('url', ['https://a.test/']))
    expect(getStarStatus).not.toHaveBeenCalled()
    expect(result.current.isStarred('url', 'https://a.test/')).toBe(true)
  })
})
