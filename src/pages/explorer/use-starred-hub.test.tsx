/**
 * Tests for the Starred hub read-model hook.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import type { StarListItem } from '../../lib/backend-client'

const listStars = vi.fn()

vi.mock('../../lib/backend-client', () => ({
  backend: {
    listStars: (...args: unknown[]) => listStars(...args) as unknown,
  },
}))

import { useStarredHub } from './use-starred-hub'

function item(overrides: Partial<StarListItem> = {}): StarListItem {
  return {
    entityKind: 'url',
    entityKey: 'https://a.test/',
    starredAt: '2026-04-01T00:00:00Z',
    domain: 'a.test',
    title: 'A',
    visitCount: 1,
    ...overrides,
  }
}

beforeEach(() => {
  listStars.mockReset().mockResolvedValue([item()])
})

afterEach(() => vi.restoreAllMocks())

describe('useStarredHub', () => {
  test('does not fetch when disabled and reports empty + not-loading', () => {
    const { result } = renderHook(() => useStarredHub(false))
    expect(listStars).not.toHaveBeenCalled()
    expect(result.current.items).toEqual([])
    expect(result.current.loading).toBe(false)
  })

  test('fetches on enable, showing loading until the snapshot resolves', async () => {
    const { result } = renderHook(() => useStarredHub(true))
    expect(result.current.loading).toBe(true)
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.items).toHaveLength(1)
    expect(listStars).toHaveBeenCalledWith(null, 'recently_starred')
  })

  test('re-fetches with the new sort when setSort changes', async () => {
    const { result } = renderHook(() => useStarredHub(true))
    await waitFor(() => expect(result.current.loading).toBe(false))
    act(() => result.current.setSort('most_revisited'))
    await waitFor(() =>
      expect(listStars).toHaveBeenCalledWith(null, 'most_revisited'),
    )
  })

  test('reload triggers a fresh fetch', async () => {
    const { result } = renderHook(() => useStarredHub(true))
    await waitFor(() => expect(result.current.loading).toBe(false))
    act(() => result.current.reload())
    await waitFor(() => expect(listStars).toHaveBeenCalledTimes(2))
  })

  test('records lastError and clears items when the fetch rejects', async () => {
    listStars.mockReset().mockRejectedValue(new Error('boom'))
    const { result } = renderHook(() => useStarredHub(true))
    await waitFor(() => expect(result.current.lastError).toContain('boom'))
    expect(result.current.items).toEqual([])
    expect(result.current.loading).toBe(false)
  })

  test('ignores a resolved fetch after unmount (cancelled guard)', async () => {
    let resolveFetch: (rows: StarListItem[]) => void = () => {}
    listStars.mockReset().mockImplementation(
      () =>
        new Promise<StarListItem[]>((resolve) => {
          resolveFetch = resolve
        }),
    )
    const { unmount } = renderHook(() => useStarredHub(true))
    unmount()
    // Resolving after unmount must not throw (the cancelled guard short-circuits
    // setSnapshot). No assertion beyond "does not throw".
    resolveFetch([item()])
    await Promise.resolve()
  })

  test('ignores a rejected fetch after unmount (cancelled catch guard)', async () => {
    let rejectFetch: (error: Error) => void = () => {}
    listStars.mockReset().mockImplementation(
      () =>
        new Promise<StarListItem[]>((_resolve, reject) => {
          rejectFetch = reject
        }),
    )
    const { unmount } = renderHook(() => useStarredHub(true))
    unmount()
    rejectFetch(new Error('late'))
    await Promise.resolve()
  })
})
