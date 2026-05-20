/**
 * Coverage test for the dashboard active-threads card.
 *
 * Covers:
 * - archive-not-ready: stays idle (no fetch, renders the honest empty copy).
 * - loading: skeleton list visible.
 * - error: error message visible.
 * - empty result: empty copy visible.
 * - populated: rows render with arrow chains + occurrence labels; click
 *   forwards the flowId to onOpenThread.
 */

import { describe, expect, test, vi } from 'vitest'
import { act, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { I18nProvider } from '@/lib/i18n'
import * as coreIntelligenceApi from '@/lib/core-intelligence/api'
import { ProfileScopeProvider } from '@/lib/profile-scope'
import type { PathFlow } from '@/lib/core-intelligence'
import { DashboardActiveThreads } from './active-threads-card'

function makeFlow(overrides: Partial<PathFlow> = {}): PathFlow {
  return {
    flowId: 'flow-1',
    flowPattern: 'a→b→c',
    stepCount: 3,
    occurrenceCount: 4,
    lastSeenAt: '2026-05-20T00:00:00Z',
    steps: [
      { index: 0, label: 'github.com', registrableDomain: 'github.com' },
      { index: 1, label: 'docs.rs', registrableDomain: 'docs.rs' },
      { index: 2, label: 'sqlite.org', registrableDomain: 'sqlite.org' },
    ],
    ...overrides,
  }
}

function renderCard(
  props: Partial<Parameters<typeof DashboardActiveThreads>[0]> = {},
) {
  return render(
    <ProfileScopeProvider>
      <I18nProvider>
        <DashboardActiveThreads
          archiveReady={true}
          onOpenAll={vi.fn()}
          onOpenThread={vi.fn()}
          {...props}
        />
      </I18nProvider>
    </ProfileScopeProvider>,
  )
}

describe('DashboardActiveThreads', () => {
  test('stays idle (no fetch) when the archive is not ready', () => {
    const spy = vi
      .spyOn(coreIntelligenceApi, 'getPathFlows')
      .mockResolvedValue({
        data: [],
        meta: { state: 'ready' },
      } as unknown as Awaited<
        ReturnType<typeof coreIntelligenceApi.getPathFlows>
      >)
    renderCard({ archiveReady: false })
    expect(spy).not.toHaveBeenCalled()
    expect(
      screen.getByTestId('dashboard-active-threads-empty'),
    ).toBeInTheDocument()
  })

  test('renders loading skeleton while fetching, then renders rows', async () => {
    let resolveFlows: (value: PathFlow[]) => void = () => {}
    const flowsPromise = new Promise<PathFlow[]>((resolve) => {
      resolveFlows = resolve
    })
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockImplementation(
      () =>
        flowsPromise.then((data) => ({
          data,
          meta: { state: 'ready' },
        })) as unknown as ReturnType<typeof coreIntelligenceApi.getPathFlows>,
    )
    renderCard()
    expect(
      screen.getByTestId('dashboard-active-threads-loading'),
    ).toBeInTheDocument()
    await act(async () => {
      resolveFlows([
        makeFlow({ flowId: 'flow-a' }),
        makeFlow({ flowId: 'flow-b' }),
      ])
    })
    expect(
      await screen.findByTestId('dashboard-active-threads-list'),
    ).toBeInTheDocument()
    expect(screen.getAllByText('4 occurrences').length).toBe(2)
  })

  test('renders the error message when the fetch rejects', async () => {
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockRejectedValue(
      new Error('boom'),
    )
    renderCard()
    expect(
      await screen.findByTestId('dashboard-active-threads-error'),
    ).toHaveTextContent('boom')
  })

  test('renders the empty copy when the result has no flows', async () => {
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue({
      data: [],
      meta: { state: 'ready' },
    } as unknown as Awaited<
      ReturnType<typeof coreIntelligenceApi.getPathFlows>
    >)
    renderCard()
    expect(
      await screen.findByTestId('dashboard-active-threads-empty'),
    ).toBeInTheDocument()
  })

  test('forwards a clicked flow id to onOpenThread', async () => {
    const user = userEvent.setup()
    vi.spyOn(coreIntelligenceApi, 'getPathFlows').mockResolvedValue({
      data: [makeFlow({ flowId: 'flow-target' })],
      meta: { state: 'ready' },
    } as unknown as Awaited<
      ReturnType<typeof coreIntelligenceApi.getPathFlows>
    >)
    const onOpenThread = vi.fn()
    renderCard({ onOpenThread })
    await user.click(
      await screen.findByTestId(
        'dashboard-active-threads-row-flow-target',
      ),
    )
    expect(onOpenThread).toHaveBeenCalledWith('flow-target')
  })
})
