/**
 * This test file protects the shared Explorer detail rail action wiring.
 *
 * Why this file exists:
 * - Detail rail actions connect selected visits to external navigation and path tracing callbacks.
 * - The panel is reused across Explorer views, so one lost callback would drift multiple workflows.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep assertions on selected-visit visibility and callback URLs rather than route-specific parent layout.
 */

import userEvent from '@testing-library/user-event'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { ExplorerDetailPanel } from './detail-panel'

const navigationPathMock = vi.hoisted(() => ({
  getNavigationPath: vi.fn(),
}))

vi.mock('../../../lib/core-intelligence/api', () => navigationPathMock)

const t = (key: string) => key

describe('ExplorerDetailPanel', () => {
  test('forwards selected visit and navigation tracer URLs to the visit handler', async () => {
    const user = userEvent.setup()
    const handleVisit = vi.fn().mockResolvedValue(undefined)
    navigationPathMock.getNavigationPath.mockResolvedValue({
      steps: [
        {
          visitId: 10,
          depth: 0,
          url: 'https://example.com/start',
          title: 'Start page',
          visitedAt: '2026-04-25T09:00:00.000Z',
        },
        {
          visitId: 11,
          depth: 1,
          url: 'https://example.com/result',
          title: 'Result page',
          visitedAt: '2026-04-25T09:02:00.000Z',
        },
      ],
    })

    const { rerender } = render(
      <MemoryRouter>
        <ExplorerDetailPanel
          commonT={t}
          explorerT={t}
          handleVisit={handleVisit}
          intelligenceT={t}
          language="en"
          selectedVisit={{
            domain: 'example.com',
            profileId: 'chrome:Default',
            title: 'Example result',
            transition: 'link',
            url: 'https://example.com/result',
            visitId: 11,
            visitedAt: '2026-04-25T09:02:00.000Z',
          }}
        />
      </MemoryRouter>,
    )

    await user.click(screen.getByRole('button', { name: 'visitRecord' }))
    await user.click(screen.getByRole('button', { name: /tracerTitle/ }))
    await screen.findByText('Start page')
    await user.click(screen.getByText('Start page'))

    expect(handleVisit).toHaveBeenNthCalledWith(1, 'https://example.com/result')
    expect(handleVisit).toHaveBeenNthCalledWith(2, 'https://example.com/start')

    rerender(
      <MemoryRouter>
        <ExplorerDetailPanel
          commonT={t}
          explorerT={t}
          handleVisit={handleVisit}
          intelligenceT={t}
          language="en"
          selectedVisit={{
            domain: 'example.com',
            profileId: 'chrome:Default',
            title: 'Invalid date result',
            transition: 'link',
            url: 'https://example.com/invalid-date',
            visitId: 12,
            visitedAt: 'not-a-date',
          }}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText('not-a-date')).toBeVisible()
  })
})
