/**
 * @file shell-runtime-status.test.tsx
 * @description Focused coverage for the shell-owned runtime polling hook.
 * @module app/shell-runtime-status
 *
 * ## Responsibilities
 * - Protect the shared runtime refresh owner extracted from `ShellDataProvider`.
 * - Verify locked/archive-neutral behavior without backend runtime reads.
 * - Verify in-flight dedupe and active-vs-idle polling cadence.
 *
 * ## Not responsible for
 * - Re-testing Jobs, Sidebar, or Settings runtime panels.
 * - Re-testing queue action mutations such as retry, cancel, or pause.
 * - Re-testing bootstrap, dashboard, backup, or app-lock shell-data suites.
 *
 * ## Dependencies
 * - Depends on the backend client spies and shell-data archive seed helper.
 * - Uses React Testing Library's hook renderer to exercise the shipped hook directly.
 *
 * ## Performance notes
 * - Uses fake timers for polling cadence checks so the suite does not wait for real 3s/15s intervals.
 */

import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../lib/backend-client'
import { createTranslator } from '../lib/i18n'
import type {
  AiQueueStatus,
  AppSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../lib/types'
import { useShellRuntimeStatus } from './shell-runtime-status'
import {
  resetShellDataHarness,
  seedSnapshot,
} from './shell-data-tests/test-helpers'

const t = createTranslator('en')

function idleAiQueue(overrides: Partial<AiQueueStatus> = {}): AiQueueStatus {
  return {
    paused: false,
    concurrency: 1,
    queued: 0,
    running: 0,
    failed: 0,
    recentJobs: [],
    ...overrides,
  }
}

function idleRuntime(
  overrides: Partial<IntelligenceRuntimeSnapshot> = {},
): IntelligenceRuntimeSnapshot {
  return {
    queue: {
      queued: 0,
      running: 0,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      lastActivityAt: null,
    },
    plugins: [],
    modules: [],
    recentJobs: [],
    notes: [],
    ...overrides,
  }
}

async function flushRuntimePromises() {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}

describe('useShellRuntimeStatus', () => {
  beforeEach(() => {
    resetShellDataHarness()
  })

  afterEach(() => {
    if (vi.isFakeTimers()) {
      vi.runOnlyPendingTimers()
      vi.useRealTimers()
    }
  })

  test('keeps locked archives neutral without backend runtime reads', async () => {
    const loadAiQueueStatusSpy = vi.spyOn(backend, 'loadAiQueueStatus')
    const loadRuntimeSpy = vi.spyOn(backend, 'loadIntelligenceRuntime')
    const { result } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot: null,
        refreshKey: 0,
        t,
      }),
    )

    let status = result.current.runtimeStatus
    await act(async () => {
      status = await result.current.refreshRuntimeStatus(null)
    })

    expect(status).toEqual({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    })
    expect(result.current.runtimeStatus).toEqual(status)
    expect(loadAiQueueStatusSpy).not.toHaveBeenCalled()
    expect(loadRuntimeSpy).not.toHaveBeenCalled()
  })

  test('does not poll snapshots that are uninitialized or locked', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    const { snapshot } = await seedSnapshot()
    const uninitializedSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        initialized: false,
      },
    }
    const lockedSnapshot: AppSnapshot = {
      ...snapshot,
      archiveStatus: {
        ...snapshot.archiveStatus,
        unlocked: false,
      },
    }
    const loadAiQueueStatusSpy = vi.spyOn(backend, 'loadAiQueueStatus')
    const loadRuntimeSpy = vi.spyOn(backend, 'loadIntelligenceRuntime')
    const { result, rerender, unmount } = renderHook(
      ({ nextSnapshot }: { nextSnapshot: AppSnapshot }) =>
        useShellRuntimeStatus({
          snapshot: nextSnapshot,
          refreshKey: 0,
          t,
        }),
      {
        initialProps: { nextSnapshot: uninitializedSnapshot },
      },
    )

    await act(flushRuntimePromises)
    expect(result.current.runtimeStatus).toEqual({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    })
    expect(vi.getTimerCount()).toBe(0)

    rerender({ nextSnapshot: lockedSnapshot })
    await act(flushRuntimePromises)

    expect(result.current.runtimeStatus).toEqual({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    })
    expect(loadAiQueueStatusSpy).not.toHaveBeenCalled()
    expect(loadRuntimeSpy).not.toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
    unmount()
  })

  test('deduplicates in-flight refreshes for the same archive scope', async () => {
    const { snapshot } = await seedSnapshot()
    const aiQueue = idleAiQueue({ queued: 2, running: 1 })
    const runtime = idleRuntime({
      queue: {
        queued: 1,
        running: 0,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        lastActivityAt: '2026-04-22T18:00:00Z',
      },
    })
    const loadAiQueueStatusSpy = vi
      .spyOn(backend, 'loadAiQueueStatus')
      .mockResolvedValue(aiQueue)
    const loadRuntimeSpy = vi
      .spyOn(backend, 'loadIntelligenceRuntime')
      .mockResolvedValue(runtime)
    const { result } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot: null,
        refreshKey: 0,
        t,
      }),
    )

    let firstStatus = result.current.runtimeStatus
    let secondStatus = result.current.runtimeStatus
    await act(async () => {
      ;[firstStatus, secondStatus] = await Promise.all([
        result.current.refreshRuntimeStatus(snapshot),
        result.current.refreshRuntimeStatus(snapshot),
      ])
    })

    expect(firstStatus).toEqual(secondStatus)
    expect(firstStatus).toMatchObject({
      aiQueue,
      intelligence: runtime,
      loading: false,
      error: null,
    })
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(1)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(1)
  })

  test('keeps same-scope data while loading and clears it for a new scope', async () => {
    vi.useFakeTimers()
    const { snapshot } = await seedSnapshot()
    const sameScopeQueue = deferred<AiQueueStatus>()
    const sameScopeRuntime = deferred<IntelligenceRuntimeSnapshot>()
    const nextScopeQueue = deferred<AiQueueStatus>()
    const nextScopeRuntime = deferred<IntelligenceRuntimeSnapshot>()
    const initialQueue = idleAiQueue({ running: 1 })
    const initialRuntime = idleRuntime({
      queue: {
        queued: 0,
        running: 1,
        succeeded: 0,
        failed: 0,
        cancelled: 0,
        lastActivityAt: '2026-04-22T18:00:00Z',
      },
    })
    vi.spyOn(backend, 'loadAiQueueStatus')
      .mockResolvedValueOnce(initialQueue)
      .mockReturnValueOnce(sameScopeQueue.promise)
      .mockReturnValueOnce(nextScopeQueue.promise)
    vi.spyOn(backend, 'loadIntelligenceRuntime')
      .mockResolvedValueOnce(initialRuntime)
      .mockReturnValueOnce(sameScopeRuntime.promise)
      .mockReturnValueOnce(nextScopeRuntime.promise)
    const nextSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        selectedProfileIds: ['safari:Work'],
      },
    }
    const { result, unmount } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot,
        refreshKey: 0,
        t,
      }),
    )

    await act(flushRuntimePromises)
    expect(result.current.runtimeStatus.aiQueue).toBe(initialQueue)

    let sameScopeRefresh: Promise<unknown> = Promise.resolve()
    act(() => {
      sameScopeRefresh = result.current.refreshRuntimeStatus(snapshot)
    })
    expect(result.current.runtimeStatus).toMatchObject({
      aiQueue: initialQueue,
      intelligence: initialRuntime,
      loading: true,
      error: null,
    })

    await act(async () => {
      sameScopeQueue.resolve(idleAiQueue())
      sameScopeRuntime.resolve(idleRuntime())
      await sameScopeRefresh
    })

    let nextScopeRefresh: Promise<unknown> = Promise.resolve()
    act(() => {
      nextScopeRefresh = result.current.refreshRuntimeStatus(nextSnapshot)
    })
    expect(result.current.runtimeStatus).toMatchObject({
      aiQueue: null,
      intelligence: null,
      loading: true,
      error: null,
    })

    await act(async () => {
      nextScopeQueue.resolve(idleAiQueue())
      nextScopeRuntime.resolve(idleRuntime())
      await nextScopeRefresh
    })
    unmount()
  })

  test('marks manual refreshes as loading until the runtime reads settle', async () => {
    vi.useFakeTimers()
    const { snapshot } = await seedSnapshot()
    const aiQueue = deferred<AiQueueStatus>()
    const runtime = deferred<IntelligenceRuntimeSnapshot>()
    vi.spyOn(backend, 'loadAiQueueStatus').mockReturnValue(aiQueue.promise)
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockReturnValue(
      runtime.promise,
    )
    const { result, unmount } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot,
        refreshKey: 0,
        t,
      }),
    )

    let pendingRefresh: Promise<unknown> = Promise.resolve()
    await act(async () => {
      await Promise.resolve()
      pendingRefresh = result.current.refreshRuntimeStatus(snapshot)
      await Promise.resolve()
    })

    expect(result.current.runtimeStatus).toMatchObject({
      loading: true,
      error: null,
    })
    expect(backend.loadAiQueueStatus).toHaveBeenCalledTimes(1)
    expect(backend.loadIntelligenceRuntime).toHaveBeenCalledTimes(1)

    await act(async () => {
      aiQueue.resolve(idleAiQueue())
      runtime.resolve(idleRuntime())
      await pendingRefresh
    })

    expect(result.current.runtimeStatus).toMatchObject({
      loading: false,
      error: null,
    })
    unmount()
  })

  test('resets runtime status and clears the in-flight dedupe scope on demand', async () => {
    const { snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue(idleAiQueue())
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      idleRuntime(),
    )
    const { result } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot,
        refreshKey: 0,
        t,
      }),
    )

    await act(async () => {
      await result.current.refreshRuntimeStatus(snapshot)
    })
    expect(result.current.runtimeStatus.aiQueue).not.toBeNull()

    let resetStatus = result.current.runtimeStatus
    act(() => {
      resetStatus = result.current.resetRuntimeStatus()
    })

    expect(resetStatus).toEqual({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    })
    expect(result.current.runtimeStatus).toEqual(resetStatus)

    await act(async () => {
      await result.current.refreshRuntimeStatus(snapshot)
    })
    expect(backend.loadAiQueueStatus).toHaveBeenCalledTimes(2)
  })

  test('publishes current runtime read failures for an available archive', async () => {
    vi.useFakeTimers()
    const { snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'loadAiQueueStatus').mockRejectedValue('offline')
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      idleRuntime(),
    )
    const { result, unmount } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot,
        refreshKey: 0,
        t,
      }),
    )

    await act(flushRuntimePromises)

    expect(result.current.runtimeStatus).toEqual({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: 'offline',
    })
    unmount()
  })

  test('uses the latest hook snapshot when refresh is called without an explicit snapshot', async () => {
    const { snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue(idleAiQueue())
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      idleRuntime(),
    )
    const { result, rerender } = renderHook(
      ({ nextSnapshot }: { nextSnapshot: AppSnapshot | null }) =>
        useShellRuntimeStatus({
          snapshot: nextSnapshot,
          refreshKey: 0,
          t,
        }),
      {
        initialProps: { nextSnapshot: null as AppSnapshot | null },
      },
    )

    rerender({ nextSnapshot: snapshot })

    await act(async () => {
      await result.current.refreshRuntimeStatus()
    })

    expect(backend.loadAiQueueStatus).toHaveBeenCalledTimes(1)
    expect(result.current.runtimeStatus.aiQueue).not.toBeNull()
  })

  test('falls back to a translated runtime error when backend reads reject', async () => {
    const { snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'loadAiQueueStatus').mockRejectedValue('offline')
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      idleRuntime(),
    )
    const { result } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot: null,
        refreshKey: 0,
        t,
      }),
    )

    let status = result.current.runtimeStatus
    await act(async () => {
      status = await result.current.refreshRuntimeStatus(snapshot)
    })

    expect(status).toEqual({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: 'offline',
    })
  })

  test('surfaces Error runtime failures and preserves newer scoped refreshes', async () => {
    const { snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'loadAiQueueStatus')
      .mockRejectedValueOnce(new Error('runtime unavailable'))
      .mockResolvedValue(idleAiQueue())
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      idleRuntime(),
    )
    const { result } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot: null,
        refreshKey: 0,
        t,
      }),
    )

    await act(async () => {
      await expect(
        result.current.refreshRuntimeStatus(snapshot),
      ).resolves.toEqual({
        aiQueue: null,
        intelligence: null,
        loading: false,
        error: 'runtime unavailable',
      })
    })

    const firstQueue = deferred<AiQueueStatus>()
    const firstRuntime = deferred<IntelligenceRuntimeSnapshot>()
    const secondQueue = deferred<AiQueueStatus>()
    const secondRuntime = deferred<IntelligenceRuntimeSnapshot>()
    vi.spyOn(backend, 'loadAiQueueStatus')
      .mockReturnValueOnce(firstQueue.promise)
      .mockReturnValueOnce(secondQueue.promise)
    vi.spyOn(backend, 'loadIntelligenceRuntime')
      .mockReturnValueOnce(firstRuntime.promise)
      .mockReturnValueOnce(secondRuntime.promise)
    const nextSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        selectedProfileIds: ['safari:Work'],
      },
    }

    let first: Promise<unknown> = Promise.resolve()
    let second: Promise<unknown> = Promise.resolve()
    act(() => {
      first = result.current.refreshRuntimeStatus(snapshot)
      second = result.current.refreshRuntimeStatus(nextSnapshot)
    })
    await act(async () => {
      firstQueue.resolve(idleAiQueue({ queued: 1 }))
      firstRuntime.resolve(idleRuntime())
      await first
    })
    const aiCallsBeforeDuplicate = vi.mocked(backend.loadAiQueueStatus).mock
      .calls.length
    const runtimeCallsBeforeDuplicate = vi.mocked(
      backend.loadIntelligenceRuntime,
    ).mock.calls.length
    let duplicateSecond: Promise<unknown> = Promise.resolve()
    act(() => {
      duplicateSecond = result.current.refreshRuntimeStatus(nextSnapshot)
    })
    expect(backend.loadAiQueueStatus).toHaveBeenCalledTimes(
      aiCallsBeforeDuplicate,
    )
    expect(backend.loadIntelligenceRuntime).toHaveBeenCalledTimes(
      runtimeCallsBeforeDuplicate,
    )
    await act(async () => {
      secondQueue.resolve(idleAiQueue())
      secondRuntime.resolve(idleRuntime())
      await Promise.all([second, duplicateSecond])
    })

    await act(async () => {
      await result.current.refreshRuntimeStatus(nextSnapshot)
    })
    expect(backend.loadAiQueueStatus).toHaveBeenCalledTimes(
      aiCallsBeforeDuplicate + 1,
    )
  })

  test('ignores stale successful refreshes after a newer scoped refresh starts', async () => {
    vi.useFakeTimers()
    const { snapshot } = await seedSnapshot()
    const staleQueue = deferred<AiQueueStatus>()
    const staleRuntime = deferred<IntelligenceRuntimeSnapshot>()
    const currentQueue = deferred<AiQueueStatus>()
    const currentRuntime = deferred<IntelligenceRuntimeSnapshot>()
    vi.spyOn(backend, 'loadAiQueueStatus')
      .mockReturnValueOnce(staleQueue.promise)
      .mockReturnValueOnce(currentQueue.promise)
    vi.spyOn(backend, 'loadIntelligenceRuntime')
      .mockReturnValueOnce(staleRuntime.promise)
      .mockReturnValueOnce(currentRuntime.promise)
    const nextSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        selectedProfileIds: ['safari:Work'],
      },
    }
    const { result, unmount } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot,
        refreshKey: 0,
        t,
      }),
    )

    await act(async () => {
      await Promise.resolve()
    })
    let currentRefresh: Promise<unknown> = Promise.resolve()
    act(() => {
      currentRefresh = result.current.refreshRuntimeStatus(nextSnapshot)
    })

    await act(async () => {
      staleQueue.resolve(idleAiQueue({ queued: 9 }))
      staleRuntime.resolve(idleRuntime())
      await flushRuntimePromises()
    })
    expect(result.current.runtimeStatus).toMatchObject({
      aiQueue: null,
      intelligence: null,
      loading: true,
      error: null,
    })

    await act(async () => {
      currentQueue.resolve(idleAiQueue())
      currentRuntime.resolve(idleRuntime())
      await currentRefresh
    })
    expect(result.current.runtimeStatus.aiQueue).not.toBeNull()
    unmount()
  })

  test('ignores stale runtime errors after a newer scoped refresh starts', async () => {
    vi.useFakeTimers()
    const { snapshot } = await seedSnapshot()
    const staleQueue = deferred<AiQueueStatus>()
    const staleRuntime = deferred<IntelligenceRuntimeSnapshot>()
    const currentQueue = deferred<AiQueueStatus>()
    const currentRuntime = deferred<IntelligenceRuntimeSnapshot>()
    vi.spyOn(backend, 'loadAiQueueStatus')
      .mockReturnValueOnce(staleQueue.promise)
      .mockReturnValueOnce(currentQueue.promise)
    vi.spyOn(backend, 'loadIntelligenceRuntime')
      .mockReturnValueOnce(staleRuntime.promise)
      .mockReturnValueOnce(currentRuntime.promise)
    const nextSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        selectedProfileIds: ['safari:Work'],
      },
    }
    const { result, unmount } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot,
        refreshKey: 0,
        t,
      }),
    )

    await act(async () => {
      await Promise.resolve()
    })
    let currentRefresh: Promise<unknown> = Promise.resolve()
    act(() => {
      currentRefresh = result.current.refreshRuntimeStatus(nextSnapshot)
    })

    await act(async () => {
      staleQueue.reject(new Error('stale failure'))
      staleRuntime.resolve(idleRuntime())
      await flushRuntimePromises()
    })
    expect(result.current.runtimeStatus).toMatchObject({
      aiQueue: null,
      intelligence: null,
      loading: true,
      error: null,
    })

    await act(async () => {
      currentQueue.resolve(idleAiQueue())
      currentRuntime.resolve(idleRuntime())
      await currentRefresh
    })
    expect(result.current.runtimeStatus.error).toBeNull()
    unmount()
  })

  test('reruns runtime polling when the shell refresh key changes', async () => {
    vi.useFakeTimers()
    const { snapshot } = await seedSnapshot()
    const loadAiQueueStatusSpy = vi
      .spyOn(backend, 'loadAiQueueStatus')
      .mockResolvedValue(idleAiQueue())
    const loadRuntimeSpy = vi
      .spyOn(backend, 'loadIntelligenceRuntime')
      .mockResolvedValue(idleRuntime())
    const { rerender, unmount } = renderHook(
      ({ refreshKey }: { refreshKey: number }) =>
        useShellRuntimeStatus({
          snapshot,
          refreshKey,
          t,
        }),
      {
        initialProps: { refreshKey: 0 },
      },
    )

    await act(flushRuntimePromises)
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(1)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(1)

    rerender({ refreshKey: 1 })
    await act(flushRuntimePromises)

    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(2)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(2)
    unmount()
  })

  test('polls active work quickly and backs off once queues are idle', async () => {
    const { snapshot } = await seedSnapshot()
    const loadAiQueueStatusSpy = vi
      .spyOn(backend, 'loadAiQueueStatus')
      .mockResolvedValueOnce(idleAiQueue({ queued: 1 }))
      .mockResolvedValue(idleAiQueue())
    const loadRuntimeSpy = vi
      .spyOn(backend, 'loadIntelligenceRuntime')
      .mockResolvedValueOnce(
        idleRuntime({
          queue: {
            queued: 1,
            running: 0,
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            lastActivityAt: '2026-04-22T18:00:00Z',
          },
        }),
      )
      .mockResolvedValue(idleRuntime())

    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    const { unmount } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot,
        refreshKey: 0,
        t,
      }),
    )

    await act(flushRuntimePromises)
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(1)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(1)

    await act(async () => {
      vi.advanceTimersByTime(3000)
      await flushRuntimePromises()
    })
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(2)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(2)

    await act(async () => {
      vi.advanceTimersByTime(14_999)
      await flushRuntimePromises()
    })
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(2)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(2)

    await act(async () => {
      vi.advanceTimersByTime(1)
      await flushRuntimePromises()
    })
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(3)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(3)

    unmount()

    await act(async () => {
      vi.advanceTimersByTime(15_000)
      await flushRuntimePromises()
    })
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(3)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(3)
  })

  test('ignores runtime refreshes that resolve after unmount', async () => {
    const { snapshot } = await seedSnapshot()
    const aiQueue = deferred<AiQueueStatus>()
    const runtime = deferred<IntelligenceRuntimeSnapshot>()
    vi.spyOn(backend, 'loadAiQueueStatus').mockReturnValue(aiQueue.promise)
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockReturnValue(
      runtime.promise,
    )

    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
    const { unmount } = renderHook(() =>
      useShellRuntimeStatus({
        snapshot,
        refreshKey: 0,
        t,
      }),
    )

    unmount()

    await act(async () => {
      aiQueue.resolve(idleAiQueue())
      runtime.resolve(idleRuntime())
      await Promise.all([aiQueue.promise, runtime.promise])
      await flushRuntimePromises()
    })
    expect(vi.getTimerCount()).toBe(0)
  })

  test('resets immediately when the archive becomes locked or unavailable', async () => {
    const { snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'loadAiQueueStatus').mockResolvedValue(
      idleAiQueue({ running: 1 }),
    )
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      idleRuntime({
        queue: {
          queued: 0,
          running: 1,
          succeeded: 0,
          failed: 0,
          cancelled: 0,
          lastActivityAt: null,
        },
      }),
    )
    const { result, rerender } = renderHook(
      ({ nextSnapshot }: { nextSnapshot: AppSnapshot | null }) =>
        useShellRuntimeStatus({
          snapshot: nextSnapshot,
          refreshKey: 0,
          t,
        }),
      {
        initialProps: { nextSnapshot: snapshot as AppSnapshot | null },
      },
    )

    await act(async () => {
      await result.current.refreshRuntimeStatus(snapshot)
    })
    expect(result.current.runtimeStatus.intelligence?.queue.running).toBe(1)

    rerender({ nextSnapshot: null })

    expect(result.current.runtimeStatus).toEqual({
      aiQueue: null,
      intelligence: null,
      loading: false,
      error: null,
    })
  })

  test('clears in-flight dedupe when the archive becomes unavailable', async () => {
    vi.useFakeTimers()
    const { snapshot } = await seedSnapshot()
    const firstQueue = deferred<AiQueueStatus>()
    const firstRuntime = deferred<IntelligenceRuntimeSnapshot>()
    const secondQueue = deferred<AiQueueStatus>()
    const secondRuntime = deferred<IntelligenceRuntimeSnapshot>()
    const loadAiQueueStatusSpy = vi
      .spyOn(backend, 'loadAiQueueStatus')
      .mockReturnValueOnce(firstQueue.promise)
      .mockReturnValueOnce(secondQueue.promise)
    const loadRuntimeSpy = vi
      .spyOn(backend, 'loadIntelligenceRuntime')
      .mockReturnValueOnce(firstRuntime.promise)
      .mockReturnValueOnce(secondRuntime.promise)
    const { result, rerender, unmount } = renderHook(
      ({ nextSnapshot }: { nextSnapshot: AppSnapshot | null }) =>
        useShellRuntimeStatus({
          snapshot: nextSnapshot,
          refreshKey: 0,
          t,
        }),
      {
        initialProps: { nextSnapshot: snapshot as AppSnapshot | null },
      },
    )

    await act(async () => {
      await Promise.resolve()
    })
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(1)

    rerender({ nextSnapshot: null })
    await act(flushRuntimePromises)

    let nextRefresh: Promise<unknown> = Promise.resolve()
    act(() => {
      nextRefresh = result.current.refreshRuntimeStatus(snapshot)
    })
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(2)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(2)

    await act(async () => {
      firstQueue.resolve(idleAiQueue({ queued: 1 }))
      firstRuntime.resolve(idleRuntime())
      secondQueue.resolve(idleAiQueue())
      secondRuntime.resolve(idleRuntime())
      await nextRefresh
    })
    unmount()
  })
})

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  let reject: (error: unknown) => void = () => {}
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })

  return { promise, reject, resolve }
}
