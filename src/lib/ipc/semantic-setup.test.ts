/**
 * @file semantic-setup.test.ts
 * @description Focused coverage for the local-semantic-search background orchestration.
 * @module lib/ipc
 *
 * ## Responsibilities
 * - Prove the MANDATORY ordering: the index build only fires AFTER the model-download terminal `done`,
 *   never concurrently (the build's embedding job bails if the model is not yet on disk).
 * - Prove the failure contracts: a terminal model `error` and a download-command rejection both reject
 *   WITHOUT firing the build, so the caller can decide (the shell action swallows).
 * - Prove the subscribe-resolves-after-terminal race is handled (single-settle latch + immediate
 *   unsubscribe) and that a late duplicate terminal event is ignored.
 *
 * ## Not responsible for
 * - Re-testing the download subscription/latch primitives (covered in model-download.test).
 * - Re-testing the Tauri event bridge itself.
 *
 * ## Dependencies
 * - Mocks `@tauri-apps/api/event` (the live channel) and the `../backend-client` command surface.
 */

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import {
  MODEL_DOWNLOAD_IDLE_TIMEOUT_MS,
  runLocalSemanticSetup,
} from './semantic-setup'
import {
  isModelDownloadInFlight,
  markModelDownloadSettled,
} from './model-download'
import type { ModelDownloadProgressEvent } from '../types'

const listen = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen,
}))

const { backendMock } = vi.hoisted(() => ({
  backendMock: {
    downloadStaticEmbeddingModel: vi.fn(),
    buildAiIndex: vi.fn(),
  },
}))

vi.mock('../backend-client', () => ({
  backend: backendMock,
}))

const FULL_REBUILD_REQUEST = {
  fullRebuild: true,
  clearOnly: false,
  scope: 'full',
} as const

describe('runLocalSemanticSetup', () => {
  // The raw channel handler the REAL `subscribeToModelDownloadProgress` registers with the mocked
  // Tauri `listen`, so each test can drive the terminal event exactly as the desktop bridge would.
  let channelHandler:
    | ((event: { payload: ModelDownloadProgressEvent | null }) => void)
    | null = null
  const unlisten = vi.fn()

  beforeEach(() => {
    channelHandler = null
    unlisten.mockClear()
    backendMock.downloadStaticEmbeddingModel.mockReset()
    backendMock.buildAiIndex.mockReset()
    backendMock.downloadStaticEmbeddingModel.mockResolvedValue(undefined)
    backendMock.buildAiIndex.mockResolvedValue(undefined)
    markModelDownloadSettled() // normalize the process-global latch between tests
    listen.mockImplementation((_event, handler) => {
      channelHandler = handler as typeof channelHandler
      return Promise.resolve(unlisten)
    })
  })

  afterEach(() => {
    listen.mockReset()
    markModelDownloadSettled()
  })

  // Let the async subscription chain (dynamic import → listen → fire download command) drain so the
  // captured handler is live before the test emits a terminal event.
  async function flush() {
    await new Promise((resolve) => setTimeout(resolve, 0))
  }

  function emit(event: ModelDownloadProgressEvent) {
    channelHandler?.({ payload: event })
  }

  test('downloads the model, then builds the index ONLY after the terminal done', async () => {
    const promise = runLocalSemanticSetup()
    await flush()

    // The download command was started, the latch is set, and the build has NOT fired yet — the
    // ordering guard holds the build back until the model is on disk.
    expect(backendMock.downloadStaticEmbeddingModel).toHaveBeenCalledTimes(1)
    expect(isModelDownloadInFlight()).toBe(true)
    expect(backendMock.buildAiIndex).not.toHaveBeenCalled()

    emit({ kind: 'done' })
    await promise

    // The build fired AFTER done, with the exact full-rebuild request, and the latch was released.
    expect(backendMock.buildAiIndex).toHaveBeenCalledTimes(1)
    expect(backendMock.buildAiIndex).toHaveBeenCalledWith(FULL_REBUILD_REQUEST)
    expect(isModelDownloadInFlight()).toBe(false)
    expect(unlisten).toHaveBeenCalledTimes(1)
  })

  test('rejects on a terminal model error and never builds the index', async () => {
    const promise = runLocalSemanticSetup()
    await flush()

    emit({ kind: 'error', message: 'disk full' })

    await expect(promise).rejects.toThrow('disk full')
    expect(backendMock.buildAiIndex).not.toHaveBeenCalled()
    expect(isModelDownloadInFlight()).toBe(false)
    expect(unlisten).toHaveBeenCalledTimes(1)
  })

  test('rejects when the download command itself rejects (Error) and never builds', async () => {
    backendMock.downloadStaticEmbeddingModel.mockRejectedValueOnce(
      new Error('spawn failed'),
    )

    await expect(runLocalSemanticSetup()).rejects.toThrow('spawn failed')
    expect(backendMock.buildAiIndex).not.toHaveBeenCalled()
    expect(isModelDownloadInFlight()).toBe(false)
    expect(unlisten).toHaveBeenCalledTimes(1)
  })

  test('wraps a non-Error download-command rejection before rejecting', async () => {
    backendMock.downloadStaticEmbeddingModel.mockRejectedValueOnce(
      'string boom',
    )

    await expect(runLocalSemanticSetup()).rejects.toThrow('string boom')
    expect(backendMock.buildAiIndex).not.toHaveBeenCalled()
  })

  test('handles the subscribe-resolves-after-terminal race and still builds', async () => {
    // The terminal fires SYNCHRONOUSLY while the subscription is still registering, so the setup is
    // already settled by the time the unsubscribe fn arrives.
    listen.mockImplementation((_event, handler) => {
      ;(handler as (event: { payload: ModelDownloadProgressEvent }) => void)({
        payload: { kind: 'done' },
      })
      return Promise.resolve(unlisten)
    })

    await runLocalSemanticSetup()

    // The race skipped starting a download the terminal already ended, unsubscribed immediately, and
    // still ran the build (the model is present).
    expect(backendMock.downloadStaticEmbeddingModel).not.toHaveBeenCalled()
    expect(backendMock.buildAiIndex).toHaveBeenCalledWith(FULL_REBUILD_REQUEST)
    expect(unlisten).toHaveBeenCalledTimes(1)
    expect(isModelDownloadInFlight()).toBe(false)
  })

  test('ignores non-terminal progress events and only builds after the terminal done', async () => {
    const promise = runLocalSemanticSetup()
    await flush()

    // A mid-download progress event is neither done nor error — it must not settle or build.
    emit({
      kind: 'fileProgress',
      file: 'model.safetensors',
      downloadedBytes: 10,
      totalBytes: 100,
    })
    expect(backendMock.buildAiIndex).not.toHaveBeenCalled()

    emit({ kind: 'done' })
    await promise
    expect(backendMock.buildAiIndex).toHaveBeenCalledTimes(1)
  })

  test('ignores a late duplicate terminal event after the first settles', async () => {
    const promise = runLocalSemanticSetup()
    await flush()

    emit({ kind: 'done' })
    // A late duplicate terminal must be a no-op (single-settle latch): no double reject, one build.
    emit({ kind: 'error', message: 'late failure' })

    await promise
    expect(backendMock.buildAiIndex).toHaveBeenCalledTimes(1)
  })

  // Under fake timers, drain the subscription chain (dynamic import → listen → fire download command →
  // arm watchdog). vitest's async timer advance flushes the microtask queue, so advancing by 0 drains
  // the chain WITHOUT firing the 120s watchdog — leaving its fake-timer state set up to advance next.
  async function drainMicrotasks() {
    await vi.advanceTimersByTimeAsync(0)
  }

  test('the idle watchdog rejects a silent channel, clears the latch, unsubscribes, and never builds', async () => {
    // A degraded event bridge / dead download thread delivers NO terminal event while the command
    // still resolves. Without the watchdog the setup would hang forever and strand the process-global
    // in-flight latch (leaving a spurious "downloading" phase in the Settings panel on next mount).
    vi.useFakeTimers()
    try {
      const promise = runLocalSemanticSetup()
      const assertion = expect(promise).rejects.toThrow(/went silent/)
      await drainMicrotasks()

      // The download started + the latch is set, but no channel event ever arrives.
      expect(backendMock.downloadStaticEmbeddingModel).toHaveBeenCalledTimes(1)
      expect(isModelDownloadInFlight()).toBe(true)

      // After a full idle window of complete silence the watchdog trips.
      await vi.advanceTimersByTimeAsync(MODEL_DOWNLOAD_IDLE_TIMEOUT_MS)
      await assertion

      // The single-settle latch cleared the process-global flag, the subscription was released, and
      // the build never fired.
      expect(isModelDownloadInFlight()).toBe(false)
      expect(unlisten).toHaveBeenCalledTimes(1)
      expect(backendMock.buildAiIndex).not.toHaveBeenCalled()
    } finally {
      vi.useRealTimers()
    }
  })

  test('a mid-download progress event resets the idle watchdog so a slow-but-active download is never aborted', async () => {
    vi.useFakeTimers()
    try {
      const promise = runLocalSemanticSetup()
      await drainMicrotasks()

      // Advance almost a full idle window, emit progress (resetting the watchdog), then advance almost
      // a full window again. A duration cap (or a non-reset watchdog) would have tripped by now.
      await vi.advanceTimersByTimeAsync(MODEL_DOWNLOAD_IDLE_TIMEOUT_MS - 1000)
      emit({
        kind: 'fileProgress',
        file: 'model.safetensors',
        downloadedBytes: 10,
        totalBytes: 100,
      })
      await vi.advanceTimersByTimeAsync(MODEL_DOWNLOAD_IDLE_TIMEOUT_MS - 1000)

      // Still active — never tripped, never built.
      expect(backendMock.buildAiIndex).not.toHaveBeenCalled()
      expect(isModelDownloadInFlight()).toBe(true)

      // The real terminal finally arrives → the model is present → the build runs normally.
      emit({ kind: 'done' })
      await promise
      expect(backendMock.buildAiIndex).toHaveBeenCalledTimes(1)
      expect(isModelDownloadInFlight()).toBe(false)
    } finally {
      vi.useRealTimers()
    }
  })
})
