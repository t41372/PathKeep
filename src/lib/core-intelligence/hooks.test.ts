import { renderHook, waitFor } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { dateRangeForCalendarYear, useAsyncData } from './hooks'

describe('core intelligence hooks', () => {
  test('builds an inclusive calendar-year range', () => {
    expect(dateRangeForCalendarYear(2024)).toEqual({
      start: '2024-01-01',
      end: '2024-12-31',
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
})
