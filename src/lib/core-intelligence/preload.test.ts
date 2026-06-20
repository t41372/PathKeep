/**
 * @file preload.test.ts
 * @description Tests for prioritized, idle-staggered intelligence overview warming.
 */

import { afterEach, describe, expect, test, vi } from 'vitest'

const { loadPrimaryMock, loadSecondaryMock } = vi.hoisted(() => ({
  loadPrimaryMock: vi.fn(),
  loadSecondaryMock: vi.fn(),
}))

vi.mock('./api', () => ({
  loadIntelligencePrimaryOverview: loadPrimaryMock,
  loadIntelligenceSecondaryOverview: loadSecondaryMock,
}))

vi.mock('./hooks', () => ({
  // Each preset gets a distinct, recognizable range so order/force assertions
  // can tell month/quarter/year/all warms apart.
  dateRangeFromPreset: (preset: string) => ({
    start: preset === 'all' ? '1900-01-01' : `start:${preset}`,
    end: '2026-06-20',
  }),
}))

import {
  preloadAllTimeIntelligenceOverview,
  preloadIntelligenceOverviews,
} from './preload'

/**
 * Idle window whose `requestIdleCallback` fires the callback synchronously, so a
 * single `await flushMicrotasks()` advances exactly one link of the chained warm.
 */
function syncIdleWindow() {
  let handle = 0
  const requestIdleCallback = vi.fn(
    (cb: (deadline: { didTimeout: boolean }) => void) => {
      cb({ didTimeout: false })
      handle += 1
      return handle
    },
  )
  const cancelIdleCallback = vi.fn()
  return {
    target: { requestIdleCallback, cancelIdleCallback } as unknown as Window,
    requestIdleCallback,
    cancelIdleCallback,
  }
}

/**
 * Idle window that stores callbacks instead of running them, so tests can assert
 * how many handles are pending and that cleanup cancels each one.
 */
function deferredIdleWindow() {
  const callbacks: Array<() => void> = []
  let handle = 0
  const requestIdleCallback = vi.fn((cb: () => void) => {
    callbacks.push(cb)
    handle += 1
    return handle
  })
  const cancelIdleCallback = vi.fn()
  return {
    target: { requestIdleCallback, cancelIdleCallback } as unknown as Window,
    requestIdleCallback,
    cancelIdleCallback,
    callbacks,
  }
}

/** Extracts the `start` of each range the primary loader was warmed with, in order. */
function primaryWarmStarts(): string[] {
  return loadPrimaryMock.mock.calls.map(
    (call) => (call[0] as { start: string }).start,
  )
}

async function flushMicrotasks() {
  // Drain enough microtask hops to settle one warm's primary -> secondary ->
  // outer `.then` -> `.catch` -> `.then(scheduleNext)` chain and arm the next
  // idle slot. Over-draining is harmless when fewer hops are pending.
  for (let i = 0; i < 8; i += 1) {
    await Promise.resolve()
  }
}

afterEach(() => {
  vi.clearAllMocks()
})

describe('preloadAllTimeIntelligenceOverview', () => {
  test('returns a no-op when no window is available', () => {
    const cancel = preloadAllTimeIntelligenceOverview(null, null)
    expect(cancel).toBeTypeOf('function')
    cancel()
    expect(loadPrimaryMock).not.toHaveBeenCalled()
  })

  test('warms primary then secondary with the all-time range and force', async () => {
    loadPrimaryMock.mockResolvedValue(undefined)
    loadSecondaryMock.mockResolvedValue(undefined)
    const { target } = syncIdleWindow()

    preloadAllTimeIntelligenceOverview('chrome:Default', target)
    await flushMicrotasks()

    expect(loadPrimaryMock).toHaveBeenCalledWith(
      { start: '1900-01-01', end: '2026-06-20' },
      'chrome:Default',
      { force: true },
    )
    expect(loadSecondaryMock).toHaveBeenCalledWith(
      { start: '1900-01-01', end: '2026-06-20' },
      'chrome:Default',
      { force: true },
    )
  })

  test('swallows loader rejections so warming never throws', async () => {
    loadPrimaryMock.mockRejectedValue(new Error('boom'))
    const { target } = syncIdleWindow()

    expect(() => preloadAllTimeIntelligenceOverview(null, target)).not.toThrow()
    await flushMicrotasks()
    expect(loadSecondaryMock).not.toHaveBeenCalled()
  })

  test('cancels the pending idle warm via the returned cleanup', () => {
    const { target, requestIdleCallback, cancelIdleCallback } =
      deferredIdleWindow()

    const cancel = preloadAllTimeIntelligenceOverview('chrome:Default', target)
    expect(requestIdleCallback).toHaveBeenCalledTimes(1)
    cancel()

    expect(cancelIdleCallback).toHaveBeenCalledWith(1)
  })

  test('defaults to the ambient window when no target is passed', () => {
    loadPrimaryMock.mockResolvedValue(undefined)
    loadSecondaryMock.mockResolvedValue(undefined)

    const cancel = preloadAllTimeIntelligenceOverview('chrome:Default')

    expect(cancel).toBeTypeOf('function')
    cancel()
  })

  test('falls back to setTimeout when idle callbacks are unavailable', () => {
    const setTimeoutSpy = vi.fn(() => 99)
    const clearTimeoutSpy = vi.fn()
    const target = {
      setTimeout: setTimeoutSpy,
      clearTimeout: clearTimeoutSpy,
    } as unknown as Window

    const cancel = preloadAllTimeIntelligenceOverview('chrome:Default', target)
    expect(setTimeoutSpy).toHaveBeenCalledWith(expect.any(Function), 160)
    cancel()
    expect(clearTimeoutSpy).toHaveBeenCalledWith(99)
  })
})

describe('preloadIntelligenceOverviews', () => {
  test('returns a no-op when no window is available', () => {
    const cancel = preloadIntelligenceOverviews('chrome:Default', null)
    expect(cancel).toBeTypeOf('function')
    cancel()
    expect(loadPrimaryMock).not.toHaveBeenCalled()
  })

  test('warms all-time first, then month, quarter, year primary in priority order', async () => {
    loadPrimaryMock.mockResolvedValue(undefined)
    loadSecondaryMock.mockResolvedValue(undefined)
    const { target } = syncIdleWindow()

    preloadIntelligenceOverviews('chrome:Default', target)

    // After the first idle slot + its microtasks, all-time primary + secondary
    // have run and the month slot has fired (sync idle), but later presets wait
    // on their own idle slots reached as each warm settles.
    await flushMicrotasks()
    await flushMicrotasks()
    await flushMicrotasks()

    // Primary warm order: all-time, month, quarter, year.
    expect(primaryWarmStarts()).toEqual([
      '1900-01-01',
      'start:month',
      'start:quarter',
      'start:year',
    ])

    // All-time forces both bands; bounded presets warm primary only, force:false.
    expect(loadPrimaryMock).toHaveBeenNthCalledWith(
      1,
      { start: '1900-01-01', end: '2026-06-20' },
      'chrome:Default',
      { force: true },
    )
    expect(loadPrimaryMock).toHaveBeenNthCalledWith(
      2,
      { start: 'start:month', end: '2026-06-20' },
      'chrome:Default',
      { force: false },
    )
    expect(loadPrimaryMock).toHaveBeenNthCalledWith(
      3,
      { start: 'start:quarter', end: '2026-06-20' },
      'chrome:Default',
      { force: false },
    )
    expect(loadPrimaryMock).toHaveBeenNthCalledWith(
      4,
      { start: 'start:year', end: '2026-06-20' },
      'chrome:Default',
      { force: false },
    )

    // Secondary is only warmed once, for all-time.
    expect(loadSecondaryMock).toHaveBeenCalledTimes(1)
    expect(loadSecondaryMock).toHaveBeenCalledWith(
      { start: '1900-01-01', end: '2026-06-20' },
      'chrome:Default',
      { force: true },
    )
  })

  test('stagger uses one idle slot per warm step (all-time + three presets)', async () => {
    loadPrimaryMock.mockResolvedValue(undefined)
    loadSecondaryMock.mockResolvedValue(undefined)
    const { target, requestIdleCallback } = syncIdleWindow()

    preloadIntelligenceOverviews('archive', target)
    await flushMicrotasks()
    await flushMicrotasks()
    await flushMicrotasks()

    expect(requestIdleCallback).toHaveBeenCalledTimes(4)
  })

  test('continues warming the bounded presets even when all-time rejects', async () => {
    loadPrimaryMock.mockRejectedValueOnce(new Error('all-time boom'))
    loadPrimaryMock.mockResolvedValue(undefined)
    loadSecondaryMock.mockResolvedValue(undefined)
    const { target } = syncIdleWindow()

    expect(() => preloadIntelligenceOverviews('archive', target)).not.toThrow()
    await flushMicrotasks()
    await flushMicrotasks()
    await flushMicrotasks()

    // All-time secondary never ran (primary rejected), but month/quarter/year did.
    expect(loadSecondaryMock).not.toHaveBeenCalled()
    expect(primaryWarmStarts()).toEqual([
      '1900-01-01',
      'start:month',
      'start:quarter',
      'start:year',
    ])
  })

  test('continues the chain when a bounded preset rejects', async () => {
    loadPrimaryMock.mockResolvedValueOnce(undefined) // all-time
    loadSecondaryMock.mockResolvedValue(undefined)
    loadPrimaryMock.mockRejectedValueOnce(new Error('month boom')) // month
    loadPrimaryMock.mockResolvedValue(undefined) // quarter, year
    const { target } = syncIdleWindow()

    preloadIntelligenceOverviews('archive', target)
    await flushMicrotasks()
    await flushMicrotasks()
    await flushMicrotasks()

    expect(primaryWarmStarts()).toEqual([
      '1900-01-01',
      'start:month',
      'start:quarter',
      'start:year',
    ])
  })

  test('cleanup cancels every pending idle handle armed so far', async () => {
    loadPrimaryMock.mockResolvedValue(undefined)
    loadSecondaryMock.mockResolvedValue(undefined)
    const { target, callbacks, cancelIdleCallback } = deferredIdleWindow()

    const cancel = preloadIntelligenceOverviews('archive', target)
    // First idle slot (all-time) armed but not yet run.
    expect(callbacks).toHaveLength(1)

    // Run the all-time slot so the month slot gets armed too.
    callbacks[0]()
    await flushMicrotasks()
    expect(callbacks).toHaveLength(2)

    cancel()
    // Both armed handles (all-time = 1, month = 2) are cancelled.
    expect(cancelIdleCallback).toHaveBeenCalledWith(1)
    expect(cancelIdleCallback).toHaveBeenCalledWith(2)
    expect(cancelIdleCallback).toHaveBeenCalledTimes(2)
  })

  test('cleanup before the chain advances stops further scheduling', async () => {
    loadPrimaryMock.mockResolvedValue(undefined)
    loadSecondaryMock.mockResolvedValue(undefined)
    const { target, callbacks, requestIdleCallback } = deferredIdleWindow()

    const cancel = preloadIntelligenceOverviews('archive', target)
    expect(requestIdleCallback).toHaveBeenCalledTimes(1)

    // Cancel before running the all-time slot, then run it: the cancelled guard
    // must prevent the month slot from being scheduled.
    cancel()
    callbacks[0]()
    await flushMicrotasks()

    expect(requestIdleCallback).toHaveBeenCalledTimes(1)
  })

  test('falls back to setTimeout when idle callbacks are unavailable', async () => {
    loadPrimaryMock.mockResolvedValue(undefined)
    loadSecondaryMock.mockResolvedValue(undefined)
    const timeoutCallbacks: Array<() => void> = []
    let handle = 0
    const setTimeoutSpy = vi.fn((cb: () => void) => {
      timeoutCallbacks.push(cb)
      handle += 1
      return handle
    })
    const clearTimeoutSpy = vi.fn()
    const target = {
      setTimeout: setTimeoutSpy,
      clearTimeout: clearTimeoutSpy,
    } as unknown as Window

    const cancel = preloadIntelligenceOverviews('archive', target)
    expect(setTimeoutSpy).toHaveBeenCalledWith(expect.any(Function), 160)

    // Drive the all-time slot so the next slot is scheduled via setTimeout too.
    timeoutCallbacks[0]()
    await flushMicrotasks()
    expect(setTimeoutSpy).toHaveBeenCalledTimes(2)

    cancel()
    expect(clearTimeoutSpy).toHaveBeenCalledWith(1)
    expect(clearTimeoutSpy).toHaveBeenCalledWith(2)
  })

  test('defaults to the ambient window when no target is passed', () => {
    loadPrimaryMock.mockResolvedValue(undefined)
    loadSecondaryMock.mockResolvedValue(undefined)

    const cancel = preloadIntelligenceOverviews('chrome:Default')

    expect(cancel).toBeTypeOf('function')
    cancel()
  })
})
