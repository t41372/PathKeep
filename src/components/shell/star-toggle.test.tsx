/**
 * Tests for the shared StarToggle affordance.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { StarToggle } from './star-toggle'

function renderToggle(
  overrides: Partial<React.ComponentProps<typeof StarToggle>> = {},
) {
  const onToggle = overrides.onToggle ?? vi.fn()
  render(
    <StarToggle
      starLabel="Star"
      unstarLabel="Unstar"
      testId="toggle"
      {...overrides}
      starred={overrides.starred ?? false}
      onToggle={onToggle}
    />,
  )
  return { onToggle }
}

describe('StarToggle', () => {
  test('shows the star label and aria-pressed=false when not starred', () => {
    renderToggle({ starred: false })
    const button = screen.getByTestId('toggle')
    expect(button).toHaveAttribute('aria-pressed', 'false')
    expect(button).toHaveAttribute('aria-label', 'Star')
    expect(button).toHaveAttribute('data-starred', 'false')
  })

  test('shows the unstar label and aria-pressed=true when starred', () => {
    renderToggle({ starred: true })
    const button = screen.getByTestId('toggle')
    expect(button).toHaveAttribute('aria-pressed', 'true')
    expect(button).toHaveAttribute('aria-label', 'Unstar')
    expect(button).toHaveAttribute('data-starred', 'true')
  })

  test('calls onToggle on click and stops propagation', () => {
    const rowClick = vi.fn()
    const onToggle = vi.fn()
    render(
      <div onClick={rowClick}>
        <StarToggle
          starred={false}
          onToggle={onToggle}
          starLabel="Star"
          unstarLabel="Unstar"
          testId="toggle"
        />
      </div>,
    )
    fireEvent.click(screen.getByTestId('toggle'))
    expect(onToggle).toHaveBeenCalledTimes(1)
    expect(rowClick).not.toHaveBeenCalled()
  })

  test('toggles on the S key (lower and upper case) and ignores others', () => {
    const { onToggle } = renderToggle()
    const button = screen.getByTestId('toggle')
    fireEvent.keyDown(button, { key: 's' })
    fireEvent.keyDown(button, { key: 'S' })
    fireEvent.keyDown(button, { key: 'Enter' })
    expect(onToggle).toHaveBeenCalledTimes(2)
  })

  test('renders at the always-visible opacity when alwaysVisible is set', () => {
    renderToggle({ starred: false, alwaysVisible: true })
    const button = screen.getByTestId('toggle')
    expect(button.className).toContain('opacity-100')
  })

  test('hover-reveals (opacity-0 baseline) when not starred and not alwaysVisible', () => {
    renderToggle({ starred: false, alwaysVisible: false })
    const button = screen.getByTestId('toggle')
    expect(button.className).toContain('opacity-0')
  })

  test('honours a custom size on the inline svg', () => {
    renderToggle({ size: 22 })
    const svg = screen.getByTestId('toggle').querySelector('svg')
    expect(svg).toHaveAttribute('width', '22')
  })

  test('does not set a redundant role on the native button', () => {
    renderToggle()
    expect(screen.getByTestId('toggle')).not.toHaveAttribute('role')
  })

  test('exposes a polite live region announcing the current state', () => {
    renderToggle({
      starred: true,
      statusLabel: { starred: 'Starred', unstarred: 'Unstarred' },
    })
    const status = screen.getByTestId('toggle-status')
    expect(status).toHaveAttribute('aria-live', 'polite')
    expect(status).toHaveTextContent('Starred')
    expect(status).toHaveClass('sr-only')
  })

  test('falls back to the action labels when no statusLabel is given', () => {
    renderToggle({ starred: false })
    expect(screen.getByTestId('toggle-status')).toHaveTextContent('Star')
  })
})
