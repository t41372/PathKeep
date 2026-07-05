/**
 * Tests for the bounded starred-count hook behind the Starred-hub entry badge.
 *
 * Covers the honest count source (urls + domains), the empty browser-preview
 * path, the reload-on-token-bump path, and the failed-read suppression path.
 */

import { renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

const getStarCounts = vi.fn()
const desktopTransport = { value: true }

vi.mock('../../lib/backend-client', () => ({
  backend: {
    getStarCounts: (...args: unknown[]) => getStarCounts(...args) as unknown,
  },
}))

vi.mock('../../lib/runtime', () => ({
  hasDesktopCommandTransport: () => desktopTransport.value,
}))

import { useStarredCount } from './use-starred-count'

beforeEach(() => {
  desktopTransport.value = true
  getStarCounts.mockReset().mockResolvedValue({ urls: 0, domains: 0 })
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('useStarredCount', () => {
  test('sums urls + domains into the honest total once the read lands', async () => {
    getStarCounts.mockResolvedValue({ urls: 7, domains: 3 })
    const { result } = renderHook(() => useStarredCount())

    await waitFor(() => expect(result.current.loaded).toBe(true))
    expect(result.current.total).toBe(10)
    expect(getStarCounts).toHaveBeenCalledTimes(1)
  })

  test('reports a trustworthy zero in browser-preview without calling the backend', async () => {
    desktopTransport.value = false
    const { result } = renderHook(() => useStarredCount())

    await waitFor(() => expect(result.current.loaded).toBe(true))
    expect(result.current.total).toBe(0)
    expect(getStarCounts).not.toHaveBeenCalled()
  })

  test('re-reads the bounded count when the reload token changes', async () => {
    getStarCounts.mockResolvedValueOnce({ urls: 2, domains: 0 })
    const { result, rerender } = renderHook(
      ({ token }: { token: number }) => useStarredCount(token),
      { initialProps: { token: 0 } },
    )

    await waitFor(() => expect(result.current.total).toBe(2))

    getStarCounts.mockResolvedValueOnce({ urls: 5, domains: 1 })
    rerender({ token: 1 })

    await waitFor(() => expect(result.current.total).toBe(6))
    expect(getStarCounts).toHaveBeenCalledTimes(2)
  })

  test('suppresses the badge (untrustworthy count) when the read fails', async () => {
    getStarCounts.mockRejectedValue(new Error('count failed'))
    const { result } = renderHook(() => useStarredCount())

    await waitFor(() => expect(getStarCounts).toHaveBeenCalledTimes(1))
    expect(result.current.loaded).toBe(false)
    expect(result.current.total).toBe(0)
  })

  test('ignores a resolved count after unmount (cancellation guard, resolve path)', async () => {
    let resolveCount: (value: {
      urls: number
      domains: number
    }) => void = () => {}
    getStarCounts.mockReturnValue(
      new Promise<{ urls: number; domains: number }>((resolve) => {
        resolveCount = resolve
      }),
    )
    const { result, unmount } = renderHook(() => useStarredCount())

    await waitFor(() => expect(getStarCounts).toHaveBeenCalledTimes(1))
    // Unmount mid-flight, THEN resolve — the cancellation guard must drop it.
    unmount()
    resolveCount({ urls: 9, domains: 9 })
    await Promise.resolve()
    // The snapshot the hook returned before unmount stays at the empty default.
    expect(result.current.loaded).toBe(false)
    expect(result.current.total).toBe(0)
  })

  test('ignores a rejected count after unmount (cancellation guard, reject path)', async () => {
    let rejectCount: (reason: unknown) => void = () => {}
    getStarCounts.mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectCount = reject
      }),
    )
    const { unmount } = renderHook(() => useStarredCount())

    await waitFor(() => expect(getStarCounts).toHaveBeenCalledTimes(1))
    unmount()
    // Reject after unmount: the catch's cancellation guard returns early.
    rejectCount(new Error('late failure'))
    await Promise.resolve()
    await Promise.resolve()
  })
})
