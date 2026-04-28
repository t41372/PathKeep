/**
 * @file results-panel.test.tsx
 * @description Focused interaction coverage for Explorer keyword results.
 * @module pages/explorer/panels
 *
 * ## Responsibilities
 * - Verify pagination, row selection, visit actions, export actions, and export artifact support.
 * - Cover the render-only results owner without replaying the full Explorer route harness.
 *
 * ## Not responsible for
 * - Re-testing backend query orchestration, favicon hydration, or URL persistence.
 * - Re-testing grouped session/trail panels.
 *
 * ## Dependencies
 * - Uses MemoryRouter because the detail rail includes internal intelligence links.
 * - Depends on shipped i18n namespaces for Explorer, Intelligence, and Common labels.
 *
 * ## Performance notes
 * - Fixtures stay small; this suite exercises callbacks rather than loading large history sets.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { createNamespaceTranslator } from '../../../lib/i18n'
import type { HistoryQueryResponse } from '../../../lib/types'
import { ExplorerResultsPanel } from './results-panel'

const commonT = createNamespaceTranslator('en', 'common')
const explorerT = createNamespaceTranslator('en', 'explorer')
const intelligenceT = createNamespaceTranslator('en', 'intelligence')

const historyResults: HistoryQueryResponse = {
  total: 120,
  page: 2,
  pageSize: 50,
  pageCount: 3,
  hasPrevious: true,
  hasNext: true,
  nextCursor: null,
  items: [
    {
      id: 7,
      profileId: 'chrome:Default',
      url: 'https://example.com/alpha',
      title: 'Alpha',
      domain: 'example.com',
      visitedAt: '2026-04-17T10:00:00Z',
      visitTime: Date.parse('2026-04-17T10:00:00Z'),
      transition: 805306368,
      favicon: null,
      sourceVisitId: 7,
    },
  ],
}

function renderPanel(
  overrides: Partial<Parameters<typeof ExplorerResultsPanel>[0]> = {},
) {
  const props: Parameters<typeof ExplorerResultsPanel>[0] = {
    actionError: null,
    commonT,
    copyFeedback: null,
    explorerT,
    exportResult: { path: '/tmp/pathkeep/export.jsonl' },
    handleCopyExportPath: vi.fn().mockResolvedValue(undefined),
    handleExport: vi.fn().mockResolvedValue(undefined),
    handleFirstHistoryPage: vi.fn(),
    handleHistoryPageJump: vi.fn(),
    handleLastHistoryPage: vi.fn(),
    handleNextHistoryPage: vi.fn(),
    handleOpenExportPath: vi.fn().mockResolvedValue(undefined),
    handlePreviousHistoryPage: vi.fn(),
    handleVisit: vi.fn().mockResolvedValue(undefined),
    historyBlockedByInvalidRegex: false,
    historyPage: 2,
    historyPageCount: 3,
    historyPageInput: '2',
    historyPageSize: 50,
    intelligenceT,
    language: 'en',
    loading: false,
    onHistoryPageInputChange: vi.fn(),
    onHistoryPageSizeChange: vi.fn(),
    onSelectHistory: vi.fn(),
    results: historyResults,
    selectedEntry: historyResults.items[0] ?? null,
    ...overrides,
  }

  return {
    props,
    view: render(
      <MemoryRouter>
        <ExplorerResultsPanel {...props} />
      </MemoryRouter>,
    ),
  }
}

describe('ExplorerResultsPanel', () => {
  test('wires pagination, selection, visit, export, copy, and open actions', async () => {
    const user = userEvent.setup()
    const { props } = renderPanel()

    await user.click(
      screen.getAllByRole('button', { name: explorerT('firstPage') })[0],
    )
    await user.click(
      screen.getAllByRole('button', { name: explorerT('previousPage') })[0],
    )
    await user.click(
      screen.getAllByRole('button', { name: explorerT('nextPage') })[0],
    )
    await user.click(
      screen.getAllByRole('button', { name: explorerT('lastPage') })[0],
    )

    const pageInput = screen.getAllByRole('spinbutton')[0]
    fireEvent.change(pageInput, { target: { value: '3' } })
    fireEvent.keyDown(pageInput, { key: 'Enter' })
    fireEvent.keyDown(pageInput, { key: 'Escape' })
    await user.click(
      screen.getAllByRole('button', { name: explorerT('jumpToPage') })[0],
    )
    await user.selectOptions(
      screen.getAllByRole('combobox', { name: explorerT('pageSizeLabel') })[0],
      '100',
    )

    const record = screen.getAllByText('Alpha')[0].closest('[role="button"]')
    expect(record).toBeInstanceOf(HTMLElement)
    await user.click(record as HTMLElement)
    ;(record as HTMLElement).focus()
    await user.keyboard('{Enter}')

    await user.click(
      screen.getAllByRole('button', { name: explorerT('visitRecord') })[0],
    )
    await user.click(screen.getByRole('button', { name: 'jsonl' }))
    await user.click(screen.getByRole('button', { name: 'markdown' }))
    await user.click(
      screen.getByRole('button', { name: commonT('openAction') }),
    )
    await user.click(
      screen.getByRole('button', { name: commonT('copyAction') }),
    )

    expect(props.handleFirstHistoryPage).toHaveBeenCalledTimes(1)
    expect(props.handlePreviousHistoryPage).toHaveBeenCalledTimes(1)
    expect(props.handleNextHistoryPage).toHaveBeenCalledTimes(1)
    expect(props.handleLastHistoryPage).toHaveBeenCalledWith(3)
    expect(props.onHistoryPageInputChange).toHaveBeenLastCalledWith('3')
    expect(props.handleHistoryPageJump).toHaveBeenCalledWith(3)
    expect(props.onHistoryPageSizeChange).toHaveBeenCalledWith(100)
    expect(props.onSelectHistory).toHaveBeenCalledWith(7)
    expect(props.handleVisit).toHaveBeenCalledWith('https://example.com/alpha')
    expect(props.handleExport).toHaveBeenCalledWith('jsonl')
    expect(props.handleExport).toHaveBeenCalledWith('markdown')
    expect(props.handleOpenExportPath).toHaveBeenCalledWith(
      '/tmp/pathkeep/export.jsonl',
    )
    expect(props.handleCopyExportPath).toHaveBeenCalledWith(
      '/tmp/pathkeep/export.jsonl',
    )
  })

  test('renders loading and invalid-regex guardrails', () => {
    const loadingView = renderPanel({
      loading: true,
      results: null,
      selectedEntry: null,
    })

    expect(screen.getByTestId('explorer-results-skeleton')).toHaveAttribute(
      'aria-label',
      commonT('loadingExplorerResults'),
    )
    expect(
      screen.getAllByRole('button', { name: explorerT('nextPage') })[0],
    ).toBeDisabled()
    loadingView.view.unmount()

    const { props } = renderPanel({
      actionError: 'export failed',
      exportResult: null,
      historyBlockedByInvalidRegex: true,
    })

    expect(screen.getByRole('alert')).toHaveTextContent('export failed')
    expect(screen.getByRole('button', { name: 'jsonl' })).toBeDisabled()
    expect(props.handleExport).not.toHaveBeenCalled()
  })

  test('falls back to URL text when a history result has no title', () => {
    renderPanel({
      results: {
        ...historyResults,
        items: [
          {
            ...historyResults.items[0],
            title: '',
            url: 'https://example.com/fallback-title',
          },
        ],
      },
      selectedEntry: null,
    })

    expect(screen.getAllByText('example.com/fallback-title')[0]).toBeVisible()
  })
})
