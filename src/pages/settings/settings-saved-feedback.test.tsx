/**
 * @file settings-saved-feedback.test.tsx
 * @description Unit coverage for the reusable Settings "Saved" chip + flash hook.
 * @module pages/settings
 */

import { act, render, renderHook, screen } from '@testing-library/react'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { SettingsSavedChip } from './settings-saved-feedback'
import { useSavedFeedback } from './use-saved-feedback'

function Wrapper({ children }: { children: ReactNode }) {
  return <I18nProvider>{children}</I18nProvider>
}

describe('useSavedFeedback', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  test('flashes visible, then hides after the visible window', () => {
    const { result } = renderHook(() => useSavedFeedback())
    expect(result.current.visible).toBe(false)

    act(() => {
      result.current.flash()
    })
    expect(result.current.visible).toBe(true)

    act(() => {
      vi.advanceTimersByTime(1500)
    })
    expect(result.current.visible).toBe(false)
  })

  test('re-flashing restarts the timer instead of stacking pulses', () => {
    const { result } = renderHook(() => useSavedFeedback())

    act(() => {
      result.current.flash()
    })
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    expect(result.current.visible).toBe(true)

    // A second flash before the first window ends restarts the countdown.
    act(() => {
      result.current.flash()
    })
    act(() => {
      vi.advanceTimersByTime(1000)
    })
    // Still visible because the timer was reset (only 1000ms since the re-flash).
    expect(result.current.visible).toBe(true)
    act(() => {
      vi.advanceTimersByTime(500)
    })
    expect(result.current.visible).toBe(false)
  })

  test('clears the pending timer on unmount', () => {
    const { result, unmount } = renderHook(() => useSavedFeedback())
    act(() => {
      result.current.flash()
    })
    // Unmounting mid-pulse must not throw or leave a dangling timer.
    expect(() => unmount()).not.toThrow()
    act(() => {
      vi.advanceTimersByTime(1500)
    })
  })
})

describe('SettingsSavedChip', () => {
  test('announces the localized label while visible', () => {
    render(
      <Wrapper>
        <SettingsSavedChip visible />
      </Wrapper>,
    )
    const chip = screen.getByTestId('settings-saved-chip')
    expect(chip).toHaveTextContent('Saved')
    expect(chip).toHaveAttribute('role', 'status')
    expect(chip).toHaveAttribute('aria-live', 'polite')
    expect(chip).toHaveAttribute('data-visible', 'true')
    // Visible chips are not hidden from assistive tech.
    expect(chip).not.toHaveAttribute('aria-hidden')
  })

  test('renders empty and aria-hidden while not visible', () => {
    render(
      <Wrapper>
        <SettingsSavedChip visible={false} testId="custom-chip" />
      </Wrapper>,
    )
    const chip = screen.getByTestId('custom-chip')
    expect(chip).not.toHaveTextContent('Saved')
    expect(chip).toHaveAttribute('aria-hidden', 'true')
    expect(chip).toHaveAttribute('data-visible', 'false')
  })
})
