import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'

import { createNamespaceTranslator } from '../../../lib/i18n'
import type { HistoryQueryResponse } from '../../../lib/types'
import * as api from '../../../lib/core-intelligence/api'
import { ExplorerDetailPanel } from './detail-panel'
import { NavigationTracer } from './navigation-tracer'
import { ExplorerResultsPanel } from './results-panel'
import { SessionGroupPanel } from './session-group'
import { TrailGroupPanel } from './trail-group'

const commonT = createNamespaceTranslator('en', 'common')
const explorerT = createNamespaceTranslator('en', 'explorer')
const intelligenceT = createNamespaceTranslator('en', 'intelligence')

describe('Explorer privacy redaction', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('redacts callback URLs in the results list and the detail rail', () => {
    const callbackUrl =
      'http://localhost:1455/success?id_token=secret-token&email=test@example.com'
    const results: HistoryQueryResponse = {
      total: 1,
      items: [
        {
          id: 1,
          profileId: 'chrome:Default',
          url: callbackUrl,
          title: `Sign into Codex ${callbackUrl}`,
          domain: 'localhost',
          favicon: null,
          visitedAt: '2026-04-18T12:00:00Z',
          visitTime: Date.parse('2026-04-18T12:00:00Z'),
          durationMs: null,
          transition: null,
          sourceVisitId: 0,
          appId: null,
        },
      ],
      page: 0,
      pageSize: 50,
      pageCount: 1,
      hasPrevious: false,
      hasNext: false,
      nextCursor: null,
    }
    const item = results.items[0]

    render(
      <ExplorerResultsPanel
        actionError={null}
        commonT={commonT}
        copiedExportPath={null}
        explorerT={explorerT}
        exportResult={null}
        handleCopyExportPath={vi.fn(async () => {})}
        handleExport={vi.fn(async () => {})}
        handleFirstHistoryPage={vi.fn()}
        handleHistoryPageJump={vi.fn()}
        handleLastHistoryPage={vi.fn()}
        handleNextHistoryPage={vi.fn()}
        handleOpenExportPath={vi.fn(async () => {})}
        handlePreviousHistoryPage={vi.fn()}
        handleVisit={vi.fn(async () => {})}
        historyBlockedByInvalidRegex={false}
        historyPage={1}
        historyPageCount={1}
        historyPageInput="1"
        intelligenceT={intelligenceT}
        language="en"
        onHistoryPageInputChange={vi.fn()}
        onSelectHistory={vi.fn()}
        results={results}
        selectedEntry={item}
      />,
    )

    expect(screen.getAllByText('localhost/success').length).toBeGreaterThan(0)
    expect(screen.queryByText(/id_token=/i)).not.toBeInTheDocument()
    expect(screen.queryByText(/test@example\.com/i)).not.toBeInTheDocument()
  })

  test('redacts callback-derived titles and visits inside session groups', async () => {
    vi.spyOn(api, 'getSessions').mockResolvedValue({
      sessions: [
        {
          sessionId: 'session-1',
          firstVisitMs: Date.parse('2026-04-18T12:00:00Z'),
          lastVisitMs: Date.parse('2026-04-18T12:15:00Z'),
          visitCount: 2,
          searchCount: 1,
          domainCount: 1,
          isDeepDive: false,
          autoTitle:
            'https://example.com/callback?code=secret&token=secret&email=test@example.com',
        },
      ],
      total: 1,
      page: 0,
      pageSize: 20,
    })
    vi.spyOn(api, 'getSessionDetail').mockResolvedValue({
      session: {
        sessionId: 'session-1',
        firstVisitMs: Date.parse('2026-04-18T12:00:00Z'),
        lastVisitMs: Date.parse('2026-04-18T12:15:00Z'),
        visitCount: 2,
        searchCount: 1,
        domainCount: 1,
        isDeepDive: false,
        autoTitle: 'Session',
      },
      visits: [
        {
          visitId: 1,
          url: 'https://example.com/callback?code=secret',
          title: null,
          registrableDomain: 'example.com',
          visitTimeMs: Date.parse('2026-04-18T12:05:00Z'),
          isSearchEvent: true,
          searchQuery:
            'https://example.com/callback?code=secret&token=secret&email=test@example.com',
          searchEngine: 'Search',
          trailId: null,
          transitionType: 'LINK',
        },
      ],
      trails: [],
    })

    const user = userEvent.setup()
    render(
      <SessionGroupPanel
        dateRange={{ start: '2026-04-01', end: '2026-04-18' }}
        explorerT={explorerT}
        intelligenceT={intelligenceT}
        language="en"
      />,
    )

    expect(await screen.findByText('example.com/callback')).toBeVisible()
    await user.click(screen.getByText('example.com/callback'))

    expect(
      await screen.findByText(/Search: "example\.com\/callback"/),
    ).toBeVisible()
    expect(screen.queryByText(/token=secret/i)).not.toBeInTheDocument()
    expect(screen.queryByText(/test@example\.com/i)).not.toBeInTheDocument()
  })

  test('redacts callback-derived queries and members inside search trails', async () => {
    vi.spyOn(api, 'getSearchTrails').mockResolvedValue({
      trails: [
        {
          trailId: 'trail-1',
          sessionId: 'session-1',
          initialQuery:
            'http://localhost:1455/success?id_token=secret-token&email=test@example.com',
          searchEngine: 'Google',
          reformulationCount: 1,
          visitCount: 2,
          landingUrl: 'https://example.com/article',
          landingDomain: 'example.com',
          firstVisitMs: Date.parse('2026-04-18T12:00:00Z'),
          lastVisitMs: Date.parse('2026-04-18T12:10:00Z'),
          maxDepth: 1,
          queries: [
            'http://localhost:1455/success?id_token=secret-token',
            'test@example.com follow up',
          ],
        },
      ],
      total: 1,
      page: 0,
      pageSize: 20,
    })
    vi.spyOn(api, 'getTrailDetail').mockResolvedValue({
      trail: {
        trailId: 'trail-1',
        sessionId: 'session-1',
        initialQuery: 'ignored',
        searchEngine: 'Google',
        reformulationCount: 1,
        visitCount: 2,
        landingUrl: null,
        landingDomain: null,
        firstVisitMs: Date.parse('2026-04-18T12:00:00Z'),
        lastVisitMs: Date.parse('2026-04-18T12:10:00Z'),
        maxDepth: 1,
        queries: [],
      },
      members: [
        {
          trailId: 'trail-1',
          visitId: 11,
          ordinal: 1,
          role: 'search_event',
          url: 'https://example.com/search',
          title: null,
          visitTimeMs: Date.parse('2026-04-18T12:02:00Z'),
          searchQuery:
            'http://localhost:1455/success?id_token=secret-token&email=test@example.com',
        },
      ],
    })

    const user = userEvent.setup()
    render(
      <TrailGroupPanel
        dateRange={{ start: '2026-04-01', end: '2026-04-18' }}
        explorerT={explorerT}
        intelligenceT={intelligenceT}
        language="en"
      />,
    )

    expect(await screen.findByText('"localhost/success"')).toBeVisible()
    await user.click(screen.getByText('"localhost/success"'))

    expect(await screen.findByText('"… follow up"')).toBeVisible()
    expect(screen.queryByText(/id_token=/i)).not.toBeInTheDocument()
    expect(screen.queryByText(/test@example\.com/i)).not.toBeInTheDocument()
  })

  test('redacts callback URLs in the navigation tracer path', async () => {
    vi.spyOn(api, 'getNavigationPath').mockResolvedValue({
      targetVisitId: 9,
      steps: [
        {
          visitId: 9,
          url: 'http://localhost:1455/success?id_token=secret-token',
          title: null,
          visitTimeMs: Date.parse('2026-04-18T12:00:00Z'),
          depth: 0,
        },
      ],
    })

    const user = userEvent.setup()
    render(
      <NavigationTracer
        intelligenceT={intelligenceT}
        visitId={9}
        onSelectVisitUrl={vi.fn()}
      />,
    )

    await user.click(
      screen.getByRole('button', { name: intelligenceT('tracerTitle') }),
    )

    await waitFor(() =>
      expect(screen.getByText('localhost/success')).toBeVisible(),
    )
    expect(screen.queryByText(/id_token=/i)).not.toBeInTheDocument()
  })

  test('redacts callback URLs in the standalone detail rail', () => {
    render(
      <ExplorerDetailPanel
        commonT={commonT}
        explorerT={explorerT}
        handleVisit={vi.fn(async () => {})}
        intelligenceT={intelligenceT}
        language="en"
        selectedVisit={{
          profileId: 'chrome:Default',
          title: null,
          transition: 'LINK',
          url: 'http://localhost:1455/success?id_token=secret-token',
          visitId: 1,
          visitedAt: '2026-04-18T12:00:00Z',
        }}
      />,
    )

    expect(screen.getAllByText('localhost/success').length).toBeGreaterThan(0)
    expect(screen.queryByText(/id_token=/i)).not.toBeInTheDocument()
  })
})
