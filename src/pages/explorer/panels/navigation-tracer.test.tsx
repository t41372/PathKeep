/**
 * @file navigation-tracer.test.tsx
 * @description Focused coverage for Explorer's on-demand navigation path panel.
 * @module pages/explorer/panels
 *
 * ## Responsibilities
 * - Verify navigation path load, failure, click, and keyboard selection behavior.
 * - Protect the "how did I get here" panel without mounting Explorer.
 *
 * ## Not responsible for
 * - Re-testing backend path reconstruction.
 * - Re-testing Explorer result row composition.
 *
 * ## Dependencies
 * - Mocks the Core Intelligence navigation-path API.
 *
 * ## Performance notes
 * - Keeps path-tracer coverage bounded to a small panel render.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import * as api from '../../../lib/core-intelligence/api'
import type { NavigationPathStep } from '../../../lib/core-intelligence/types'
import { NavigationTracer } from './navigation-tracer'

const intelligenceT = (key: string) => key

describe('NavigationTracer', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('loads and activates navigation path steps', async () => {
    const user = userEvent.setup()
    const onSelectVisitUrl = vi.fn()
    vi.spyOn(api, 'getNavigationPath').mockResolvedValue({
      targetVisitId: 2,
      steps: [
        stepFixture(1, 0, 'https://example.com/start', 'Start page'),
        stepFixture(
          2,
          1,
          'https://example.com/current?token=secret',
          'Current page',
        ),
      ],
    })

    render(
      <NavigationTracer
        intelligenceT={intelligenceT}
        onSelectVisitUrl={onSelectVisitUrl}
        visitId={2}
      />,
    )

    await user.click(screen.getByRole('button', { name: 'tracerTitle' }))
    expect(await screen.findByText('Start page')).toBeVisible()
    expect(screen.getByText('Current page')).toBeVisible()
    expect(screen.getByText('← tracerHere')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'Start page' }))
    expect(onSelectVisitUrl).toHaveBeenCalledWith('https://example.com/start')

    screen.getByRole('button', { name: 'Current page' }).focus()
    await user.keyboard('{ArrowDown}')
    await user.keyboard('{Enter}')
    await user.keyboard(' ')
    expect(onSelectVisitUrl).toHaveBeenCalledWith(
      'https://example.com/current?token=secret',
    )
    expect(onSelectVisitUrl).toHaveBeenCalledTimes(3)

    await user.click(screen.getByRole('button', { name: 'tracerTitle' }))
    await user.click(screen.getByRole('button', { name: 'tracerTitle' }))
    expect(api.getNavigationPath).toHaveBeenCalledTimes(1)
  })

  test('surfaces load failures and empty paths', async () => {
    const user = userEvent.setup()
    vi.spyOn(api, 'getNavigationPath')
      .mockRejectedValueOnce('bridge offline')
      .mockResolvedValueOnce({ targetVisitId: 2, steps: [] })

    const { unmount } = render(
      <NavigationTracer intelligenceT={intelligenceT} visitId={1} />,
    )

    await user.click(screen.getByRole('button', { name: 'tracerTitle' }))
    expect(await screen.findByText('bridge offline')).toBeVisible()

    unmount()
    render(<NavigationTracer intelligenceT={intelligenceT} visitId={2} />)
    await user.click(screen.getByRole('button', { name: 'tracerTitle' }))
    await waitFor(() => expect(api.getNavigationPath).toHaveBeenCalledWith(2))
    expect(await screen.findByText('tracerEmpty')).toBeVisible()
  })

  test('surfaces Error load failures', async () => {
    const user = userEvent.setup()
    vi.spyOn(api, 'getNavigationPath').mockRejectedValue(
      new Error('path unavailable'),
    )

    render(<NavigationTracer intelligenceT={intelligenceT} visitId={3} />)

    await user.click(screen.getByRole('button', { name: 'tracerTitle' }))
    expect(await screen.findByText('path unavailable')).toBeVisible()
  })
})

function stepFixture(
  visitId: number,
  depth: number,
  url: string,
  title: string,
): NavigationPathStep {
  return {
    visitId,
    depth,
    url,
    title,
    visitTimeMs: Date.parse('2026-04-25T12:00:00.000Z'),
  }
}
