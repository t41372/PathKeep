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
import type { AiQueueStatus, IntelligenceRuntimeSnapshot } from '../lib/types'
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
    expect(result.current.runtimeStatus).toMatchObject(firstStatus)
    expect(loadAiQueueStatusSpy).toHaveBeenCalledTimes(1)
    expect(loadRuntimeSpy).toHaveBeenCalledTimes(1)
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
      error: t('common.notAvailable'),
    })
    expect(result.current.runtimeStatus).toEqual(status)
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
})
