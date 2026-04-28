/**
 * @file timeline-bar.test.tsx
 * @description Render-level coverage for the Explorer timeline strip.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Verify date shortcuts, clear-range controls, and page summary copy stay wired.
 * - Keep Explorer's render-only timeline owner covered without mounting the full route.
 *
 * ## Not responsible for
 * - Fetching Explorer data.
 * - Re-testing URL-state parsing or date-window math.
 *
 * ## Dependencies
 * - Uses the real shared shortcut list from `helpers.ts`.
 *
 * ## Performance notes
 * - Pure render tests keep strict coverage cheap while protecting user-visible controls.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { ExplorerTimelineBar } from './timeline-bar'

describe('ExplorerTimelineBar', () => {
  test('applies shortcuts, clears ranges, and renders page summaries', () => {
    const onApplyDateShortcut = vi.fn()
    const onClearDateRange = vi.fn()

    const { rerender } = render(
      <ExplorerTimelineBar
        activeShortcutKey="week"
        end="2026-04-25"
        explorerT={explorerT}
        onApplyDateShortcut={onApplyDateShortcut}
        onClearDateRange={onClearDateRange}
        start="2026-04-19"
        summary={{
          currentPage: 2,
          loaded: 50,
          pageCount: 4,
          total: 180,
        }}
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'shortcutMonth' }))
    expect(onApplyDateShortcut).toHaveBeenCalledWith(30)
    expect(screen.getByText('pageCountSummary:2/4')).toBeVisible()
    expect(screen.getByText('resultsSummary:50/180')).toBeVisible()
    expect(screen.getByText('2026-04-19 → 2026-04-25')).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: 'clearRange' }))
    expect(onClearDateRange).toHaveBeenCalled()

    rerender(
      <ExplorerTimelineBar
        activeShortcutKey={null}
        end={null}
        explorerT={explorerT}
        onApplyDateShortcut={onApplyDateShortcut}
        onClearDateRange={onClearDateRange}
        start="2026-04-19"
        summary={null}
      />,
    )
    expect(screen.getByText('2026-04-19 → …')).toBeVisible()

    rerender(
      <ExplorerTimelineBar
        activeShortcutKey={null}
        end="2026-04-25"
        explorerT={explorerT}
        onApplyDateShortcut={onApplyDateShortcut}
        onClearDateRange={onClearDateRange}
        start={null}
        summary={null}
      />,
    )
    expect(screen.getByText('… → 2026-04-25')).toBeVisible()
  })

  test('renders waiting copy when no query summary or date range is available', () => {
    render(
      <ExplorerTimelineBar
        activeShortcutKey={null}
        end={null}
        explorerT={explorerT}
        onApplyDateShortcut={vi.fn()}
        onClearDateRange={vi.fn()}
        start={null}
        summary={null}
      />,
    )

    expect(screen.getByText('waitingForQuery')).toBeVisible()
    expect(screen.getByText('allRecordedTime')).toBeVisible()
    expect(
      screen.queryByRole('button', { name: 'clearRange' }),
    ).not.toBeInTheDocument()
  })
})

function explorerT(key: string, vars?: Record<string, string | number>) {
  if (key === 'pageCountSummary') {
    return `pageCountSummary:${vars?.current}/${vars?.total}`
  }
  if (key === 'resultsSummary') {
    return `resultsSummary:${vars?.loaded}/${vars?.total}`
  }
  return key
}
