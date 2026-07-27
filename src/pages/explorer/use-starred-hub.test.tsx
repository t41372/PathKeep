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

import { STARRED_HUB_LIMIT, useStarredHub } from './use-starred-hub'

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
    // E2: an explicit limit. The call used to pass none, silently taking the
    // backend's DEFAULT_LIST_LIMIT of 500 and dropping the rest with no signal.
    expect(listStars).toHaveBeenCalledWith(
      null,
      'recently_starred',
      STARRED_HUB_LIMIT,
    )
  })

  test('re-fetches with the new sort when setSort changes', async () => {
    const { result } = renderHook(() => useStarredHub(true))
    await waitFor(() => expect(result.current.loading).toBe(false))
    act(() => result.current.setSort('most_revisited'))
    await waitFor(() =>
      expect(listStars).toHaveBeenCalledWith(
        null,
        'most_revisited',
        STARRED_HUB_LIMIT,
      ),
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

  test('reports truncated when the reply fills the requested limit', async () => {
    // A full page means the backend may be holding more. Surfaces need this
    // flag to say so rather than presenting the prefix as the whole set.
    listStars
      .mockReset()
      .mockResolvedValue(
        Array.from({ length: STARRED_HUB_LIMIT }, () => item()),
      )
    const { result } = renderHook(() => useStarredHub(true))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.items).toHaveLength(STARRED_HUB_LIMIT)
    expect(result.current.truncated).toBe(true)
    expect(result.current.limit).toBe(STARRED_HUB_LIMIT)
  })

  test('reports not-truncated for a partial page', async () => {
    const { result } = renderHook(() => useStarredHub(true))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.truncated).toBe(false)
  })
})
