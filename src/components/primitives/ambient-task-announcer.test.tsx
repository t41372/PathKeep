/**
 * Tests for the ambient-task announcer — the always-mounted SR live region that speaks the
 * background-work PRESENCE transition.
 *
 * Why this file exists:
 * - AmbientTaskAnnouncer's whole contract is "announce on appear/disappear, never on a progress
 *   tick". That is a subtle transition-vs-value distinction, so it is pinned here independently of
 *   the shell that mounts it.
 */

import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { AmbientTaskAnnouncer } from './ambient-task-announcer'

const labels = {
  startedLabel: 'Background work started',
  endedLabel: 'Background work finished',
}

describe('AmbientTaskAnnouncer', () => {
  test('is silent on mount even when work is already active (no announce on mount)', () => {
    render(<AmbientTaskAnnouncer active={true} {...labels} />)
    const region = screen.getByTestId('ambient-task-announcer')
    expect(region).toHaveAttribute('role', 'status')
    expect(region).toHaveAttribute('aria-live', 'polite')
    expect(region).toHaveClass('sr-only')
    expect(region).toHaveTextContent('')
  })

  test('announces the started label on the false→true transition', () => {
    const { rerender } = render(
      <AmbientTaskAnnouncer active={false} {...labels} />,
    )
    expect(screen.getByTestId('ambient-task-announcer')).toHaveTextContent('')

    rerender(<AmbientTaskAnnouncer active={true} {...labels} />)
    expect(screen.getByTestId('ambient-task-announcer')).toHaveTextContent(
      'Background work started',
    )
  })

  test('does NOT re-announce when active stays true across a progress tick', () => {
    const { rerender } = render(
      <AmbientTaskAnnouncer active={false} {...labels} />,
    )
    rerender(<AmbientTaskAnnouncer active={true} {...labels} />)
    expect(screen.getByTestId('ambient-task-announcer')).toHaveTextContent(
      'Background work started',
    )
    // A rerender that leaves `active` true (a progress tick) must not change the
    // message — proving presence-based, not per-tick, announcements.
    rerender(<AmbientTaskAnnouncer active={true} {...labels} />)
    expect(screen.getByTestId('ambient-task-announcer')).toHaveTextContent(
      'Background work started',
    )
  })

  test('announces the ended label on the true→false transition', () => {
    const { rerender } = render(
      <AmbientTaskAnnouncer active={false} {...labels} />,
    )
    rerender(<AmbientTaskAnnouncer active={true} {...labels} />)
    rerender(<AmbientTaskAnnouncer active={false} {...labels} />)
    expect(screen.getByTestId('ambient-task-announcer')).toHaveTextContent(
      'Background work finished',
    )
  })
})
