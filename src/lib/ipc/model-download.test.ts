/**
 * @file model-download.test.ts
 * @description Focused coverage for the in-app model download subscription + in-flight latch.
 * @module lib/ipc
 *
 * ## Responsibilities
 * - Verify the helper subscribes to the exact desktop progress channel and forwards payloads
 *   (ignoring empty ones), and degrades to a no-op unsubscribe when the bridge is unavailable.
 * - Verify the process-global in-flight latch flips with started/settled and reads back true only
 *   while a download is in flight.
 * - Drive the REAL `useModelDownloadProgress` hook through the mocked Tauri event channel and assert
 *   its full phase/byte state machine: byte accumulation across files, `done` → ready immediately,
 *   `modelDownloaded` → ready override, cancel → idle (not failed), and subscription cleanup.
 *
 * ## Not responsible for
 * - Re-testing the static download panel UI (covered in ai-providers-section.test).
 * - Re-testing the Tauri event implementation itself.
 *
 * ## Dependencies
 * - Mocks `@tauri-apps/api/event` at the module boundary.
 * - Renders the hook with `@testing-library/react`'s `renderHook` for the state-machine coverage.
 */

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { act, renderHook, waitFor } from '@testing-library/react'
import {
  MODEL_DOWNLOAD_PROGRESS_EVENT,
  isModelDownloadInFlight,
  markModelDownloadSettled,
  markModelDownloadStarted,
  subscribeToModelDownloadProgress,
  useModelDownloadProgress,
} from './model-download'
import type { ModelDownloadProgressEvent } from '../types'

const listen = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen,
}))

describe('subscribeToModelDownloadProgress', () => {
  afterEach(() => {
    listen.mockReset()
  })

  test('subscribes to the desktop progress channel and forwards non-empty payloads', async () => {
    const unsubscribe = vi.fn()
    const received: ModelDownloadProgressEvent[] = []
    listen.mockImplementation((_event, handler) => {
      handler({ payload: { kind: 'fileStarted', file: 'a', totalBytes: 0 } })
      // An empty payload must be ignored, not forwarded.
      handler({ payload: null })
      handler({ payload: { kind: 'done' } })
      return Promise.resolve(unsubscribe)
    })

    const result = await subscribeToModelDownloadProgress((event) => {
      received.push(event)
    })

    expect(listen).toHaveBeenCalledWith(
      MODEL_DOWNLOAD_PROGRESS_EVENT,
      expect.any(Function),
    )
    expect(received).toEqual([
      { kind: 'fileStarted', file: 'a', totalBytes: 0 },
      { kind: 'done' },
    ])
    expect(result).toBe(unsubscribe)
  })

  test('returns a noop unsubscribe when the desktop event bridge is unavailable', async () => {
    const listener = vi.fn()
    listen.mockRejectedValueOnce(new Error('event bridge unavailable'))

    const result = await subscribeToModelDownloadProgress(listener)

    expect(result()).toBeUndefined()
    expect(listener).not.toHaveBeenCalled()
  })
})

describe('model download in-flight latch', () => {
  beforeEach(() => {
    // The latch is module-scoped (it survives a remount by design); normalize it before each test.
    markModelDownloadSettled()
  })

  test('reads false initially, true after started, false after settled', () => {
    expect(isModelDownloadInFlight()).toBe(false)
    markModelDownloadStarted()
    expect(isModelDownloadInFlight()).toBe(true)
    markModelDownloadSettled()
    expect(isModelDownloadInFlight()).toBe(false)
  })
})

describe('useModelDownloadProgress', () => {
  // Capture the raw channel handler that the REAL `subscribeToModelDownloadProgress` registers with
  // the mocked Tauri `listen`, so each test can drive the hook through fileStarted / fileProgress /
  // fileFinished / done / error exactly as the desktop bridge would.
  let channelHandler:
    | ((event: { payload: ModelDownloadProgressEvent | null }) => void)
    | null = null
  const unlisten = vi.fn()

  beforeEach(() => {
    channelHandler = null
    unlisten.mockClear()
    markModelDownloadSettled() // normalize the module-scoped latch between tests
    listen.mockImplementation((_event, handler) => {
      channelHandler = handler as typeof channelHandler
      return Promise.resolve(unlisten)
    })
  })

  afterEach(() => {
    listen.mockReset()
    markModelDownloadSettled()
  })

  // Flush the async subscription chain (dynamic import → listen → assign unsubscribe) so the captured
  // handler is registered and `unsub` is wired up before the test drives events / unmounts.
  async function flushSubscription() {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0))
    })
  }

  // Render the real hook and wait until its channel subscription is live.
  async function renderProgress(
    modelDownloaded: boolean,
    cancelledRef?: { current: boolean },
  ) {
    const view = renderHook(() =>
      useModelDownloadProgress(modelDownloaded, cancelledRef),
    )
    await flushSubscription()
    return view
  }

  // Push a raw progress event through the captured channel handler (wrapped like the desktop bridge).
  function emit(event: ModelDownloadProgressEvent) {
    act(() => {
      channelHandler?.({ payload: event })
    })
  }

  test('returns ready immediately and never subscribes when the model is already downloaded', () => {
    const { result } = renderHook(() => useModelDownloadProgress(true))

    expect(result.current.phase).toBe('ready')
    // Authoritative override: with weights present, no channel subscription is opened.
    expect(listen).not.toHaveBeenCalled()
  })

  test('initializes in the downloading phase when a download is already in flight (remount)', async () => {
    markModelDownloadStarted()

    const { result } = await renderProgress(false)

    expect(result.current.phase).toBe('downloading')
  })

  test('accumulates bytes across files and flips to ready immediately on done', async () => {
    const { result } = await renderProgress(false)
    // Idle until the first event arrives.
    expect(result.current.phase).toBe('idle')

    // File 1 starts with a known total; the basename is derived from the path.
    emit({
      kind: 'fileStarted',
      file: 'cache/model.safetensors',
      totalBytes: 100,
    })
    expect(result.current.phase).toBe('downloading')
    expect(result.current.currentFile).toBe('model.safetensors')
    expect(result.current.totalBytes).toBe(100)
    expect(result.current.downloadedBytes).toBe(0)
    expect(isModelDownloadInFlight()).toBe(true)

    // Progress within file 1 (totalBytes refines the per-file total).
    emit({
      kind: 'fileProgress',
      file: 'cache/model.safetensors',
      downloadedBytes: 60,
      totalBytes: 100,
    })
    expect(result.current.downloadedBytes).toBe(60)
    expect(result.current.totalBytes).toBe(100)

    // File 1 finishes — its full (mapped) total folds into the finished running total.
    emit({ kind: 'fileFinished', file: 'cache/model.safetensors' })
    expect(result.current.downloadedBytes).toBe(100)
    expect(result.current.currentFile).toBeNull()

    // File 2 starts; the known total now spans both files.
    emit({ kind: 'fileStarted', file: 'cache/tokenizer.json', totalBytes: 40 })
    expect(result.current.totalBytes).toBe(140)
    expect(result.current.downloadedBytes).toBe(100)

    // Progress within file 2 adds onto the finished total.
    emit({
      kind: 'fileProgress',
      file: 'cache/tokenizer.json',
      downloadedBytes: 25,
      totalBytes: 40,
    })
    expect(result.current.downloadedBytes).toBe(125)

    // done → ready immediately (no snapshot re-poll), current file cleared, latch settled.
    emit({ kind: 'done' })
    expect(result.current.phase).toBe('ready')
    expect(result.current.currentFile).toBeNull()
    expect(isModelDownloadInFlight()).toBe(false)
  })

  test('keeps the total unknown without a Content-Length and folds an unmapped finish into current bytes', async () => {
    const { result } = await renderProgress(false)

    // fileStarted with an unknown total → totalBytes stays 0 (indeterminate).
    emit({ kind: 'fileStarted', file: 'blob', totalBytes: 0 })
    expect(result.current.totalBytes).toBe(0)
    expect(result.current.currentFile).toBe('blob')

    // fileProgress with totalBytes 0 must NOT fabricate a per-file total; only downloaded advances.
    emit({
      kind: 'fileProgress',
      file: 'blob',
      downloadedBytes: 30,
      totalBytes: 0,
    })
    expect(result.current.downloadedBytes).toBe(30)
    expect(result.current.totalBytes).toBe(0)

    // A finish for a file with no mapped total folds in the last-seen current-file bytes.
    emit({ kind: 'fileFinished', file: 'unmapped' })
    expect(result.current.downloadedBytes).toBe(30)
    expect(result.current.currentFile).toBeNull()
  })

  test('maps a non-cancel terminal error to the failed phase with the message (no cancelledRef)', async () => {
    const { result } = await renderProgress(false)

    emit({ kind: 'fileStarted', file: 'a', totalBytes: 0 })
    emit({ kind: 'error', message: 'disk full' })

    expect(result.current.phase).toBe('failed')
    expect(result.current.error).toBe('disk full')
    expect(result.current.currentFile).toBeNull()
    expect(isModelDownloadInFlight()).toBe(false)
  })

  test('maps a cancel-triggered terminal error to idle (not failed) and records no error', async () => {
    const cancelledRef = { current: false }
    const { result } = await renderProgress(false, cancelledRef)

    emit({ kind: 'fileStarted', file: 'a', totalBytes: 0 })
    expect(result.current.phase).toBe('downloading')

    // The caller sets this true just before firing the cancel IPC.
    cancelledRef.current = true
    emit({ kind: 'error', message: 'cancelled' })

    expect(result.current.phase).toBe('idle')
    expect(result.current.error).toBeNull()
    expect(isModelDownloadInFlight()).toBe(false)
  })

  test('a restartNonce bump clears a prior failure and resets to downloading at once', async () => {
    // The retry path (M1): a mid-download failure leaves phase 'failed' + bytes accumulated. The
    // caller bumps restartNonce on the Retry click, which must reset to 'downloading' + clear the
    // error/bytes IMMEDIATELY (before the next fileStarted), so the panel never flashes a stale
    // "Download failed" during the connection-setup window.
    const { result, rerender } = renderHook(
      ({ nonce }: { nonce: number }) =>
        useModelDownloadProgress(false, undefined, nonce),
      { initialProps: { nonce: 0 } },
    )
    await flushSubscription()

    emit({
      kind: 'fileStarted',
      file: 'cache/model.safetensors',
      totalBytes: 100,
    })
    emit({
      kind: 'fileProgress',
      file: 'cache/model.safetensors',
      downloadedBytes: 60,
      totalBytes: 100,
    })
    emit({ kind: 'error', message: 'network down' })
    expect(result.current.phase).toBe('failed')
    expect(result.current.error).toBe('network down')
    expect(result.current.downloadedBytes).toBe(60)

    // User clicks Retry → nonce bumps → the reset effect fires.
    act(() => rerender({ nonce: 1 }))
    expect(result.current.phase).toBe('downloading')
    expect(result.current.error).toBeNull()
    expect(result.current.downloadedBytes).toBe(0)
    expect(result.current.totalBytes).toBe(0)
    expect(result.current.currentFile).toBeNull()
  })

  test('unsubscribes from the channel on unmount (no listener leak)', async () => {
    const { unmount } = await renderProgress(false)
    expect(unlisten).not.toHaveBeenCalled()

    unmount()
    expect(unlisten).toHaveBeenCalledTimes(1)
  })

  test('unsubscribes at once if it unmounts before the subscription resolves', async () => {
    // Race path: the effect cleanup runs before the subscribe promise resolves, so the resolved
    // unsubscribe must fire immediately rather than leak.
    let resolveListen: ((unsub: () => void) => void) | null = null
    listen.mockImplementation((_event, handler) => {
      channelHandler = handler as typeof channelHandler
      return new Promise<() => void>((resolve) => {
        resolveListen = resolve
      })
    })

    const { unmount } = renderHook(() => useModelDownloadProgress(false))
    // Let the dynamic import resolve so `listen` is invoked, but DON'T resolve its promise yet.
    await waitFor(() => expect(channelHandler).not.toBeNull())

    unmount()

    const lateUnsub = vi.fn()
    await act(async () => {
      resolveListen?.(lateUnsub)
      await Promise.resolve()
    })

    expect(lateUnsub).toHaveBeenCalledTimes(1)
  })

  test('ignores channel events delivered after unmount (no post-teardown state update)', async () => {
    const { unmount } = await renderProgress(false)

    unmount()

    // The captured handler may still fire after teardown; the active guard must swallow it.
    expect(() => emit({ kind: 'done' })).not.toThrow()
  })
})
