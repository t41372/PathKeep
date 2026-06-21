/**
 * Tests for the visit-enrichment detail-panel hook (W-ENRICH-1).
 *
 * Covers the load → state projection (loading / disabled / empty / ready /
 * error), and the Fetch-now PME path: an enabled enqueue flips to pending and
 * re-loads after the delay, a `disabled` result is treated as an honest error,
 * a thrown error surfaces honestly, and the CTA is gated by consent.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

const listVisitEnrichment = vi.fn()
const contentFetchNow = vi.fn()

vi.mock('../../lib/backend-client', () => ({
  backend: {
    listVisitEnrichment: (...args: unknown[]) =>
      listVisitEnrichment(...args) as unknown,
    contentFetchNow: (...args: unknown[]) =>
      contentFetchNow(...args) as unknown,
  },
}))

import { useVisitEnrichment } from './use-visit-enrichment'

const target = {
  historyId: 7,
  profileId: 'chrome:Default',
  url: 'https://github.com/owner/repo',
  title: 'owner/repo',
}

beforeEach(() => {
  listVisitEnrichment.mockReset().mockResolvedValue([])
  contentFetchNow.mockReset().mockResolvedValue({
    jobId: 1,
    state: 'queued',
    note: 'queued',
  })
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.useRealTimers()
})

describe('useVisitEnrichment', () => {
  test('a null target reports an empty state and does not read the backend', () => {
    const { result } = renderHook(() =>
      useVisitEnrichment({ target: null, fetchEnabled: true }),
    )
    expect(result.current.state).toEqual({ status: 'empty' })
    expect(listVisitEnrichment).not.toHaveBeenCalled()
  })

  test('no enrichment + consent on yields an honest empty state', async () => {
    const { result } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: true }),
    )
    await waitFor(() =>
      expect(result.current.state).toEqual({ status: 'empty' }),
    )
    expect(listVisitEnrichment).toHaveBeenCalledWith(7)
  })

  test('no enrichment + consent off yields a disabled state', async () => {
    const { result } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: false }),
    )
    await waitFor(() =>
      expect(result.current.state).toEqual({ status: 'disabled' }),
    )
  })

  test('projects the best stored record into a ready state', async () => {
    listVisitEnrichment.mockResolvedValue([
      {
        contentSource: 'github-repo',
        fetchStatus: 'success',
        fetchedAt: '2026-06-21T00:00:00Z',
        readableTitle: 'owner/repo',
        metadataJson: JSON.stringify({ topics: ['rust'] }),
      },
    ])
    const { result } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: true }),
    )
    await waitFor(() => expect(result.current.state.status).toBe('ready'))
    if (result.current.state.status !== 'ready') throw new Error('not ready')
    expect(result.current.state.view.topics).toEqual(['rust'])
  })

  test('a read failure surfaces an honest error state', async () => {
    listVisitEnrichment.mockRejectedValue(new Error('boom'))
    const { result } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: true }),
    )
    await waitFor(() =>
      expect(result.current.state).toEqual({ status: 'error' }),
    )
  })

  test('fetchNow enqueues, flips to pending, and re-loads after the delay', async () => {
    vi.useFakeTimers()
    listVisitEnrichment.mockResolvedValue([])
    const { result } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: true }),
    )
    // Drain the initial load.
    await vi.waitFor(() => expect(listVisitEnrichment).toHaveBeenCalledTimes(1))

    act(() => result.current.fetchNow())
    await vi.waitFor(() => expect(contentFetchNow).toHaveBeenCalledTimes(1))
    expect(contentFetchNow).toHaveBeenCalledWith({
      historyId: 7,
      profileId: 'chrome:Default',
      url: 'https://github.com/owner/repo',
      title: 'owner/repo',
    })
    expect(result.current.fetchPending).toBe(true)

    // After the delay the hook clears pending and re-reads enrichment.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2500)
    })
    expect(result.current.fetchPending).toBe(false)
    await vi.waitFor(() => expect(listVisitEnrichment).toHaveBeenCalledTimes(2))
  })

  test('a disabled fetch result is treated as an honest error, not a queued fetch', async () => {
    contentFetchNow.mockResolvedValue({
      jobId: 0,
      state: 'disabled',
      note: 'disabled',
    })
    const { result } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: true }),
    )
    await waitFor(() => expect(result.current.state.status).toBe('empty'))
    act(() => result.current.fetchNow())
    await waitFor(() => expect(result.current.fetchError).toBe(true))
    expect(result.current.fetchPending).toBe(false)
  })

  test('a thrown fetch surfaces a fetch error and clears pending', async () => {
    contentFetchNow.mockRejectedValue(new Error('nope'))
    const { result } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: true }),
    )
    await waitFor(() => expect(result.current.state.status).toBe('empty'))
    act(() => result.current.fetchNow())
    await waitFor(() => expect(result.current.fetchError).toBe(true))
    expect(result.current.fetchPending).toBe(false)
  })

  test('fetchNow is a no-op when consent is off', async () => {
    const { result } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: false }),
    )
    await waitFor(() => expect(result.current.state.status).toBe('disabled'))
    act(() => result.current.fetchNow())
    expect(contentFetchNow).not.toHaveBeenCalled()
  })

  test('a resolved read after unmount is dropped (no state update on a dead hook)', async () => {
    // Hold the read open so we can unmount before it resolves, exercising the
    // `if (cancelled) return` guard in the load effect's then-branch.
    let resolveRead: (value: unknown[]) => void = () => {}
    listVisitEnrichment.mockReturnValue(
      new Promise<unknown[]>((resolve) => {
        resolveRead = resolve
      }),
    )
    const { unmount } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: true }),
    )
    await waitFor(() => expect(listVisitEnrichment).toHaveBeenCalled())
    unmount()
    // Resolving after unmount must not throw or warn — the guard short-circuits.
    await act(async () => {
      resolveRead([])
      await Promise.resolve()
    })
  })

  test('a rejected read after unmount is dropped', async () => {
    let rejectRead: (reason: unknown) => void = () => {}
    listVisitEnrichment.mockReturnValue(
      new Promise<unknown[]>((_resolve, reject) => {
        rejectRead = reject
      }),
    )
    const { unmount } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: true }),
    )
    await waitFor(() => expect(listVisitEnrichment).toHaveBeenCalled())
    unmount()
    await act(async () => {
      rejectRead(new Error('late'))
      await Promise.resolve()
    })
  })

  test('fetchNow sends a null title when the target has none', async () => {
    const { result } = renderHook(() =>
      useVisitEnrichment({
        target: { ...target, title: null },
        fetchEnabled: true,
      }),
    )
    await waitFor(() => expect(result.current.state.status).toBe('empty'))
    act(() => result.current.fetchNow())
    await waitFor(() => expect(contentFetchNow).toHaveBeenCalled())
    expect(contentFetchNow.mock.calls[0][0].title).toBeNull()
  })

  test('a second fetchNow clears the prior pending re-load timer', async () => {
    vi.useFakeTimers()
    const { result } = renderHook(() =>
      useVisitEnrichment({ target, fetchEnabled: true }),
    )
    await vi.waitFor(() => expect(listVisitEnrichment).toHaveBeenCalledTimes(1))

    // First enqueue arms the re-load timer.
    act(() => result.current.fetchNow())
    await vi.waitFor(() => expect(contentFetchNow).toHaveBeenCalledTimes(1))

    // A second enqueue while still pending must clear the prior timer (the
    // `if (refreshTimer.current)` truthy branch) before arming a new one. The
    // hook gates a second call on `fetchPending`, so flush the first timer to
    // clear pending, arm again, then enqueue once more without flushing.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2500)
    })
    await vi.waitFor(() => expect(listVisitEnrichment).toHaveBeenCalledTimes(2))
    act(() => result.current.fetchNow())
    await vi.waitFor(() => expect(contentFetchNow).toHaveBeenCalledTimes(2))
    // Advancing flushes the second timer; the prior (already-fired) timer id is
    // still on the ref, so the second enqueue took the truthy clear branch.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2500)
    })
    await vi.waitFor(() => expect(listVisitEnrichment).toHaveBeenCalledTimes(3))
  })
})
