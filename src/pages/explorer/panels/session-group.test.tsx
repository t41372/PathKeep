/**
 * @file session-group.test.tsx
 * @description Focused coverage for Explorer's grouped session panel.
 * @module pages/explorer/panels
 *
 * ## Responsibilities
 * - Verify empty/error states, pagination controls, session expansion, and visit selection.
 * - Protect the grouped Explorer surface from losing session-detail interactions.
 *
 * ## Not responsible for
 * - Re-testing privacy redaction, which has a dedicated cross-panel suite.
 * - Re-testing promoted session route rendering.
 *
 * ## Dependencies
 * - Mocks Core Intelligence session API calls.
 * - Uses MemoryRouter because expanded cards expose route links.
 *
 * ## Performance notes
 * - Small fixtures keep pagination and detail behavior deterministic.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type {
  DateRange,
  SessionSummary,
  SessionVisit,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import { createNamespaceTranslator } from '../../../lib/i18n'
import { SessionGroupPanel } from './session-group'

const dateRange: DateRange = { start: '2026-04-01', end: '2026-04-30' }
const explorerT = createNamespaceTranslator('en', 'explorer')
const intelligenceT = createNamespaceTranslator('en', 'intelligence')

describe('SessionGroupPanel', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('renders the empty state when no sessions are returned', async () => {
    vi.spyOn(api, 'getSessions').mockResolvedValue({
      page: 0,
      pageSize: 20,
      sessions: [],
      total: 0,
    })

    renderPanel()

    expect(
      await screen.findByText('No sessions found in this time range.'),
    ).toBeVisible()
  })

  test('renders the error state when grouped sessions fail to load', async () => {
    vi.spyOn(api, 'getSessions').mockRejectedValue(new Error('sessions failed'))

    renderPanel()

    expect(await screen.findByText('sessions failed')).toBeVisible()
  })

  test('paginates sessions and reuses the same grouped detail behavior', async () => {
    const user = userEvent.setup()
    const selectVisit = vi.fn()
    const getSessions = vi
      .spyOn(api, 'getSessions')
      .mockImplementation((_range, _profileId, options) =>
        Promise.resolve({
          page: options?.page ?? 0,
          pageSize: 20,
          sessions:
            options?.page === 1
              ? [sessionFixture({ sessionId: 'session-2', autoTitle: '' })]
              : [
                  sessionFixture({
                    sessionId: 'session-1',
                    autoTitle: 'Research session',
                    isDeepDive: true,
                    searchCount: 2,
                  }),
                ],
          total: 25,
        }),
      )
    vi.spyOn(api, 'getSessionDetail').mockResolvedValue({
      session: sessionFixture({ sessionId: 'session-1' }),
      trails: [],
      visits: [visitFixture()],
    })

    renderPanel({ onSelectVisit: selectVisit })

    expect(await screen.findByText('Research session')).toBeVisible()
    expect(screen.getByText(/2 searches/)).toBeVisible()
    expect(screen.getByTitle('Deep Dive Session')).toBeVisible()
    expect(
      screen.getByRole('button', { name: explorerT('previousPage') }),
    ).toBeDisabled()

    await user.click(
      screen.getByRole('button', { name: explorerT('nextPage') }),
    )
    await waitFor(() =>
      expect(getSessions).toHaveBeenLastCalledWith(
        dateRange,
        'chrome:Default',
        {
          page: 1,
          pageSize: 20,
        },
      ),
    )
    expect(await screen.findByText('Untitled Session')).toBeVisible()
    expect(
      screen.getByRole('button', { name: explorerT('nextPage') }),
    ).toBeDisabled()

    await user.click(
      screen.getByRole('button', { name: explorerT('previousPage') }),
    )
    expect(await screen.findByText('Research session')).toBeVisible()

    await user.click(screen.getByRole('button', { name: /Research session/ }))
    expect(await screen.findByText('Search: "rust tauri"')).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Open session insights' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/session/session-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault',
    )

    await user.click(screen.getByText('Search: "rust tauri"'))
    expect(selectVisit).toHaveBeenCalledWith({
      domain: 'example.com',
      profileId: 'chrome:Default',
      title: null,
      transition: 'LINK',
      url: 'https://example.com/search?q=rust+tauri',
      visitId: 10,
      visitedAt: '2026-04-25T12:00:00.000Z',
    })
  })

  test('shows a detail fallback when expanded session detail fails', async () => {
    const user = userEvent.setup()
    vi.spyOn(api, 'getSessions').mockResolvedValue({
      page: 0,
      pageSize: 20,
      sessions: [sessionFixture()],
      total: 1,
    })
    vi.spyOn(api, 'getSessionDetail').mockRejectedValue(new Error('offline'))

    renderPanel()

    await user.click(await screen.findByRole('button', { name: /Session/ }))
    expect(
      await screen.findByText('Could not load session details.'),
    ).toBeVisible()
  })

  test('renders non-English time branches and cached detail toggles', async () => {
    const user = userEvent.setup()
    const getSessionDetail = vi
      .spyOn(api, 'getSessionDetail')
      .mockResolvedValue({
        session: sessionFixture({ sessionId: 'session-zh' }),
        trails: [],
        visits: [
          visitFixture({
            searchEngine: null,
            searchQuery: 'local recall',
            visitId: 20,
          }),
          visitFixture({
            isSearchEvent: false,
            searchQuery: null,
            title: 'Readable page title',
            visitId: 21,
          }),
          visitFixture({
            isSearchEvent: false,
            searchQuery: null,
            title: null,
            url: 'https://example.com/no-title',
            visitId: 22,
          }),
        ],
      })
    vi.spyOn(api, 'getSessions').mockResolvedValue({
      page: 0,
      pageSize: 20,
      sessions: [
        sessionFixture({
          sessionId: 'session-zh',
          autoTitle: '中文 session',
        }),
      ],
      total: 1,
    })

    const { unmount } = renderPanel({
      language: 'zh-CN',
      profileId: null,
    })

    await user.click(
      await screen.findByRole('button', { name: /中文 session/ }),
    )
    expect(await screen.findByText('Search: "local recall"')).toBeVisible()
    expect(screen.getByText('Readable page title')).toBeVisible()
    expect(screen.getByText('example.com/no-title')).toBeVisible()
    await user.click(screen.getByText('Readable page title'))
    await user.click(screen.getByRole('button', { name: /中文 session/ }))
    await user.click(screen.getByRole('button', { name: /中文 session/ }))
    expect(getSessionDetail).toHaveBeenCalledTimes(1)

    unmount()
    renderPanel({ language: 'zh-TW' })
    await user.click(
      await screen.findByRole('button', { name: /中文 session/ }),
    )
    expect(await screen.findByText('Search: "local recall"')).toBeVisible()
  })
})

function renderPanel({
  language = 'en',
  onSelectVisit,
  profileId = 'chrome:Default',
}: {
  language?: Parameters<typeof SessionGroupPanel>[0]['language']
  onSelectVisit?: Parameters<typeof SessionGroupPanel>[0]['onSelectVisit']
  profileId?: Parameters<typeof SessionGroupPanel>[0]['profileId']
} = {}) {
  return render(
    <MemoryRouter>
      <SessionGroupPanel
        dateRange={dateRange}
        explorerT={explorerT}
        intelligenceT={intelligenceT}
        language={language}
        onSelectVisit={onSelectVisit}
        profileId={profileId}
      />
    </MemoryRouter>,
  )
}

function sessionFixture(
  overrides: Partial<SessionSummary> = {},
): SessionSummary {
  return {
    autoTitle: 'Session',
    domainCount: 1,
    firstVisitMs: Date.parse('2026-04-25T11:55:00Z'),
    isDeepDive: false,
    lastVisitMs: Date.parse('2026-04-25T12:10:00Z'),
    searchCount: 0,
    sessionId: 'session-1',
    visitCount: 2,
    ...overrides,
  }
}

function visitFixture(overrides: Partial<SessionVisit> = {}): SessionVisit {
  return {
    isSearchEvent: true,
    registrableDomain: 'example.com',
    searchEngine: 'Search',
    searchQuery: 'rust tauri',
    title: null,
    trailId: null,
    transitionType: 'LINK',
    url: 'https://example.com/search?q=rust+tauri',
    visitId: 10,
    visitTimeMs: Date.parse('2026-04-25T12:00:00Z'),
    ...overrides,
  }
}
