/**
 * Tests for the non-blocking bottom-bar progress strip.
 *
 * Why this file exists:
 * - BackgroundProgress is the alternative surface for long-running but
 *   user-deferrable shell actions (manual backup, etc.). It has its own
 *   determinate / indeterminate behaviour and its own truncation rules
 *   for the inline detail + log lines, so the contract is worth pinning
 *   independently of the BusyOverlay it sometimes replaces.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import type { BusyOverlayState } from '@/app/shell-data-context'
import { BackgroundProgress } from './background-progress'

function baseState(
  overrides: Partial<BusyOverlayState> = {},
): BusyOverlayState {
  return {
    label: 'Running a manual backup',
    background: true,
    ...overrides,
  }
}

describe('BackgroundProgress', () => {
  test('renders the label, indeterminate bar, and no percentage when progressValue is null', () => {
    const { container } = render(
      <BackgroundProgress
        state={baseState({
          progressLabel: 'Waiting for backup progress',
          progressValue: null,
        })}
      />,
    )
    expect(screen.getByText('Running a manual backup')).toBeVisible()
    expect(screen.getByText('Waiting for backup progress')).toBeVisible()
    // No percentage cell when the value is not numeric.
    expect(container.textContent).not.toMatch(/%/)
  })

  test('renders a determinate bar + percentage when progressValue is numeric', () => {
    render(
      <BackgroundProgress
        state={baseState({
          progressLabel: '50 / 100',
          progressValue: 50,
        })}
      />,
    )
    expect(screen.getByText('50%')).toBeVisible()
    expect(screen.getByText('50 / 100')).toBeVisible()
  })

  test('clamps progress to the [0, 100] window so a runaway worker cannot stretch the bar', () => {
    render(<BackgroundProgress state={baseState({ progressValue: 1234 })} />)
    expect(screen.getByText('100%')).toBeVisible()
  })

  test('treats NaN progressValue as indeterminate', () => {
    const { container } = render(
      <BackgroundProgress state={baseState({ progressValue: Number.NaN })} />,
    )
    expect(container.textContent).not.toMatch(/%/)
  })

  test('falls back to fallbackLabel when state.label is empty', () => {
    render(
      <BackgroundProgress
        state={baseState({ label: '' })}
        fallbackLabel="Working…"
      />,
    )
    expect(screen.getByText('Working…')).toBeVisible()
  })

  test('renders the detail line and the most recent log line when both exist', () => {
    render(
      <BackgroundProgress
        state={baseState({
          detail: 'Chrome / Default',
          logLines: ['imported 10', 'imported 50'],
        })}
      />,
    )
    expect(screen.getByText('Chrome / Default')).toBeVisible()
    expect(screen.getByText('imported 50')).toBeVisible()
    // The earlier log lines are dropped — the strip only shows the most
    // recent one so the chrome stays one line tall.
    expect(screen.queryByText('imported 10')).toBeNull()
  })

  test('omits the duplicate log row when the detail already says the same thing', () => {
    render(
      <BackgroundProgress
        state={baseState({
          detail: 'Chrome / Default',
          logLines: ['Chrome / Default'],
        })}
      />,
    )
    const matches = screen.getAllByText('Chrome / Default')
    expect(matches).toHaveLength(1)
  })

  test('does not render the detail row at all when neither detail nor log is provided', () => {
    const { container } = render(<BackgroundProgress state={baseState()} />)
    // The strip is exactly two rows: header + progress bar. The optional
    // detail/log row is dropped entirely so the chrome stays compact.
    // Match on the items-baseline detail container — header rows use
    // items-center, so this query is precise to the detail/log row.
    expect(container.querySelectorAll('.items-baseline').length).toBe(0)
  })
})
