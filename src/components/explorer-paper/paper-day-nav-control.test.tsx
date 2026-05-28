/**
 * Tests for PaperDayNavControl — the prev / pill / next / today control.
 *
 * The control routes three different user intents (prev day, next day, jump
 * to today) and exposes the calendar-toggle handshake. Each branch needs to
 * stay distinguishable from the others so the surrounding Browse view can
 * route key presses (←/→/T/G) without ambiguity.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperDayNavControl,
  type PaperDayNavControlCopy,
} from './paper-day-nav-control'

const COPY: PaperDayNavControlCopy = {
  prev: 'Previous day',
  next: 'Next day',
  today: 'Today',
  openCalendar: 'Open calendar',
}

function baseProps(): Parameters<typeof PaperDayNavControl>[0] {
  return {
    dow: 'FRI',
    monthDay: 'May 16',
    year: '2026',
    densityTier: 3,
    countLabel: '1,234p',
    relativeAgo: 'yesterday',
    calOpen: false,
    isToday: false,
    onPrev: vi.fn(),
    onNext: vi.fn(),
    onToday: vi.fn(),
    onToggleCal: vi.fn(),
    copy: COPY,
    testId: 'daynav',
  }
}

describe('PaperDayNavControl', () => {
  test('renders dow, month/day, year, count, and relative label', () => {
    render(<PaperDayNavControl {...baseProps()} />)

    expect(screen.getByText('FRI')).toBeVisible()
    expect(screen.getByText('May 16')).toBeVisible()
    expect(screen.getByText('2026')).toBeVisible()
    expect(screen.getByText('1,234p')).toBeVisible()
    expect(screen.getByText('yesterday')).toBeVisible()
  })

  test('wires prev / next / today buttons to their handlers', () => {
    const onPrev = vi.fn()
    const onNext = vi.fn()
    const onToday = vi.fn()
    render(
      <PaperDayNavControl
        {...baseProps()}
        onPrev={onPrev}
        onNext={onNext}
        onToday={onToday}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Previous day' }))
    fireEvent.click(screen.getByRole('button', { name: 'Next day' }))
    fireEvent.click(screen.getByRole('button', { name: 'Today' }))

    expect(onPrev).toHaveBeenCalledTimes(1)
    expect(onNext).toHaveBeenCalledTimes(1)
    expect(onToday).toHaveBeenCalledTimes(1)
  })

  test('pill click toggles the calendar and aria-expanded reflects calOpen', () => {
    const onToggleCal = vi.fn()
    const { rerender } = render(
      <PaperDayNavControl {...baseProps()} onToggleCal={onToggleCal} />,
    )

    const pill = screen.getByRole('button', { name: 'Open calendar' })
    expect(pill.getAttribute('aria-expanded')).toBe('false')
    fireEvent.click(pill)
    expect(onToggleCal).toHaveBeenCalledTimes(1)

    rerender(
      <PaperDayNavControl {...baseProps()} onToggleCal={onToggleCal} calOpen />,
    )
    expect(
      screen
        .getByRole('button', { name: 'Open calendar' })
        .getAttribute('aria-expanded'),
    ).toBe('true')
    expect(screen.getByTestId('daynav').dataset.calOpen).toBe('true')
  })

  test('honours prevDisabled and nextDisabled', () => {
    const onPrev = vi.fn()
    const onNext = vi.fn()
    render(
      <PaperDayNavControl
        {...baseProps()}
        prevDisabled
        nextDisabled
        onPrev={onPrev}
        onNext={onNext}
      />,
    )

    const prev = screen.getByRole<HTMLButtonElement>('button', {
      name: 'Previous day',
    })
    const next = screen.getByRole<HTMLButtonElement>('button', {
      name: 'Next day',
    })
    expect(prev.disabled).toBe(true)
    expect(next.disabled).toBe(true)
    fireEvent.click(prev)
    fireEvent.click(next)
    expect(onPrev).not.toHaveBeenCalled()
    expect(onNext).not.toHaveBeenCalled()
  })

  test('renders the calendar slot when supplied', () => {
    render(
      <PaperDayNavControl
        {...baseProps()}
        calOpen
        calendarSlot={<div data-testid="cal-mounted">cal</div>}
      />,
    )

    expect(screen.getByTestId('cal-mounted')).toBeVisible()
  })

  test('marks the Today button as current when isToday is set', () => {
    render(<PaperDayNavControl {...baseProps()} isToday />)
    const todayButton = screen.getByRole('button', { name: 'Today' })
    // The active variant adds border-accent — assert via className substring.
    expect(todayButton.className).toContain('border-accent')
  })

  test('density swatch reflects the requested tier via data-tier', () => {
    const { container, rerender } = render(
      <PaperDayNavControl {...baseProps()} densityTier={0} />,
    )
    expect(container.querySelector('[data-tier="t0"]')).not.toBeNull()

    rerender(<PaperDayNavControl {...baseProps()} densityTier={4} />)
    expect(container.querySelector('[data-tier="t4"]')).not.toBeNull()
  })

  test('every density tier renders without throwing', () => {
    for (const tier of [0, 1, 2, 3, 4] as const) {
      const { container, unmount } = render(
        <PaperDayNavControl {...baseProps()} densityTier={tier} />,
      )
      expect(container.querySelector(`[data-tier="t${tier}"]`)).not.toBeNull()
      unmount()
    }
  })
})
