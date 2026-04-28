import { act, renderHook, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, test, vi } from 'vitest'
import {
  dateRangeForCalendarYear,
  dateRangeFromPreset,
  useAsyncData,
  useTimeRange,
} from './hooks'

describe('core intelligence hooks', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  test('builds an inclusive calendar-year range', () => {
    expect(dateRangeForCalendarYear(2024)).toEqual({
      start: '2024-01-01',
      end: '2024-12-31',
    })
  })

  test('builds rolling date ranges from every supported preset', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-04-25T12:00:00'))

    expect(dateRangeFromPreset('day')).toEqual({
      start: '2026-04-25',
      end: '2026-04-25',
    })
    expect(dateRangeFromPreset('week')).toEqual({
      start: '2026-04-18',
      end: '2026-04-25',
    })
    expect(dateRangeFromPreset('month')).toEqual({
      start: '2026-03-25',
      end: '2026-04-25',
    })
    expect(dateRangeFromPreset('quarter')).toEqual({
      start: '2026-01-25',
      end: '2026-04-25',
    })
    expect(dateRangeFromPreset('year')).toEqual({
      start: '2025-04-25',
      end: '2026-04-25',
    })
    expect(dateRangeFromPreset('all')).toEqual({
      start: '1900-01-01',
      end: '2026-04-25',
    })
    expect(dateRangeFromPreset('custom')).toEqual({
      start: '2026-03-25',
      end: '2026-04-25',
    })
  })

  test('hydrates immediately from cached data while revalidating in the background', async () => {
    const fetcher = vi.fn().mockResolvedValue({ value: 'fresh' })

    const { result } = renderHook(() =>
      useAsyncData(fetcher, ['scope'], {
        getCached: () => ({ value: 'cached' }),
      }),
    )

    expect(result.current.data).toEqual({ value: 'cached' })
    expect(result.current.loading).toBe(false)

    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(result.current.data).toEqual({ value: 'fresh' }))
    expect(result.current.loading).toBe(false)
  })

  test('keeps stale cached data visible when a background refresh fails', async () => {
    const fetcher = vi.fn().mockRejectedValue(new Error('refresh failed'))

    const { result } = renderHook(() =>
      useAsyncData(fetcher, ['scope'], {
        getCached: () => ({ value: 'cached' }),
      }),
    )

    expect(result.current.data).toEqual({ value: 'cached' })
    expect(result.current.loading).toBe(false)

    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(result.current.loading).toBe(false))
    expect(result.current.data).toEqual({ value: 'cached' })
    expect(result.current.error).toBeNull()
  })

  test('surfaces non-Error async failures when no cached data is visible', async () => {
    const fetcher = vi.fn().mockRejectedValue('bridge offline')

    const { result } = renderHook(() => useAsyncData(fetcher, ['scope']))

    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1))
    await waitFor(() => expect(result.current.loading).toBe(false))

    expect(result.current.data).toBeNull()
    expect(result.current.error).toBe('bridge offline')
  })

  test('ignores stale async results after a newer dependency request starts', async () => {
    const first = deferred<{ value: string }>()
    const second = deferred<{ value: string }>()
    const fetcher = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)

    const { result, rerender } = renderHook(
      ({ scope }: { scope: string }) => useAsyncData(fetcher, [scope]),
      { initialProps: { scope: 'one' } },
    )

    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1))
    rerender({ scope: 'two' })
    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2))

    await act(async () => {
      first.resolve({ value: 'stale' })
      await first.promise
    })
    expect(result.current.data).toBeNull()
    expect(result.current.loading).toBe(true)

    await act(async () => {
      second.resolve({ value: 'fresh' })
      await second.promise
    })
    expect(result.current.data).toEqual({ value: 'fresh' })

    const third = deferred<{ value: string }>()
    const fourth = deferred<{ value: string }>()
    fetcher
      .mockReturnValueOnce(third.promise)
      .mockReturnValueOnce(fourth.promise)

    rerender({ scope: 'three' })
    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(3))
    rerender({ scope: 'four' })
    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(4))

    await act(async () => {
      third.reject(new Error('stale failure'))
      await third.promise.catch(() => undefined)
    })
    expect(result.current.error).toBeNull()
  })

  test('manages preset and explicit custom time ranges', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-04-25T12:00:00'))

    const { result } = renderHook(() => useTimeRange('day'))

    expect(result.current.preset).toBe('day')
    expect(result.current.dateRange).toEqual({
      start: '2026-04-25',
      end: '2026-04-25',
    })

    act(() => {
      result.current.setPreset('week')
    })
    expect(result.current.preset).toBe('week')
    expect(result.current.dateRange).toEqual({
      start: '2026-04-18',
      end: '2026-04-25',
    })

    act(() => {
      result.current.setPreset('custom')
    })
    expect(result.current.preset).toBe('custom')
    expect(result.current.dateRange).toEqual({
      start: '2026-04-18',
      end: '2026-04-25',
    })

    act(() => {
      result.current.setPreset('all')
    })
    expect(result.current.preset).toBe('all')
    expect(result.current.dateRange).toEqual({
      start: '1900-01-01',
      end: '2026-04-25',
    })

    act(() => {
      result.current.setCustomRange({
        start: '2024-01-01',
        end: '2024-02-01',
      })
    })
    expect(result.current.preset).toBe('custom')
    expect(result.current.dateRange).toEqual({
      start: '2024-01-01',
      end: '2024-02-01',
    })
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
