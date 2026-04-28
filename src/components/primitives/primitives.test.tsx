/**
 * This test file protects the shared Primitives component contract.
 *
 * Why this file exists:
 * - Reusable shell components can create subtle regressions everywhere at once, so the tests here act as a front-end safety net.
 * - If the design or accessibility contract changes, these tests should tell the next reader exactly which promise moved.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Shared shell components must stay aligned with `docs/design/screens-and-nav.md`, `docs/design/ux-principles.md`, and `docs/design/design-tokens.md`.
 * - Avoid locking tests to decorative markup when the actual contract is state visibility, routing, or accessible labeling.
 */

import { render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { BusyOverlay } from './busy-overlay'
import { EmptyState } from './empty-state'
import { ErrorState } from './error-state'
import { LoadingState } from './loading-state'
import { PermissionGate } from './permission-gate'
import {
  DashboardSkeleton,
  Skeleton,
  SkeletonExplorer,
  SkeletonExplorerResults,
  SkeletonInsights,
  TableSkeleton,
} from './skeleton'
import { StatusCallout } from './status-callout'

describe('Shell primitives', () => {
  test('renders an empty state with optional action content', () => {
    render(
      <EmptyState
        action={<button type="button">Review sources</button>}
        description="Search, filters, and detail panes land here next."
        eyebrow="EXPLORER"
        title="History Explorer"
      />,
    )

    expect(screen.getByText('EXPLORER')).toBeVisible()
    expect(
      screen.getByRole('heading', { name: 'History Explorer' }),
    ).toBeVisible()
    expect(screen.getByRole('button', { name: 'Review sources' })).toBeVisible()
  })

  test('renders loading and error state affordances', () => {
    render(
      <>
        <BusyOverlay
          label="Applying native schedule changes"
          progressLabel="2 / 4"
          progressValue={50}
        />
        <LoadingState
          label="Rebuilding the semantic index"
          detail="Refreshing derived views without blocking the archive."
          logLines={[
            'queued',
            'opened archive',
            'read batches',
            'wrote facts',
            'done',
          ]}
          progressLabel="1 / 2"
          progressValue={50}
        />
        <ErrorState
          description="The app should pause here and show rollback instructions."
          title="Schedule preview unavailable"
        />
      </>,
    )

    const statuses = screen.getAllByRole('status')

    expect(statuses[0]).toHaveTextContent('Applying native schedule changes')
    expect(statuses[1]).toHaveTextContent('Rebuilding the semantic index')
    expect(screen.getAllByText('50%')).toHaveLength(2)
    expect(screen.queryByText('queued')).not.toBeInTheDocument()
    expect(screen.getByText('opened archive')).toBeVisible()
    expect(screen.getByText('done')).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Schedule preview unavailable',
    )
    expect(screen.queryByText('ATTENTION')).not.toBeInTheDocument()
  })

  test('renders loading state progress when only one progress field is available', () => {
    render(
      <>
        <LoadingState label="Reading rows" progressValue={2} />
        <LoadingState label="Preparing import" progressLabel="File 1 of 2" />
        <LoadingState
          label="Waiting for native event"
          progressValue={Number.NaN}
        />
      </>,
    )

    expect(screen.getByText('2%')).toBeVisible()
    expect(screen.getByText('File 1 of 2')).toBeVisible()
    expect(screen.queryByText('NaN%')).not.toBeInTheDocument()
  })

  test('renders loading state compact, empty-progress, and clamped progress contracts', () => {
    const { container } = render(
      <>
        <LoadingState label="Plain loading" />
        <LoadingState compact label="Compact loading" progressValue={2} />
        <LoadingState label="Label-only progress" progressLabel="File 1 of 2" />
      </>,
    )
    const statuses = screen.getAllByRole('status')

    expect(statuses[0]).toHaveClass('loading-state')
    expect(statuses[0]).toHaveAttribute('class', 'loading-state ')
    expect(
      within(statuses[0]).queryByText('File 1 of 2'),
    ).not.toBeInTheDocument()
    expect(
      statuses[0].querySelector('.loading-state__progress'),
    ).not.toBeInTheDocument()

    expect(statuses[1]).toHaveClass('loading-state', 'loading-state--compact')
    expect(within(statuses[1]).getByText('2%')).toBeVisible()
    expect(
      statuses[1].querySelector('.loading-state__progress-fill'),
    ).toHaveStyle({ width: '4%' })

    expect(within(statuses[2]).getByText('File 1 of 2')).toBeVisible()
    expect(within(statuses[2]).queryByText(/%/)).not.toBeInTheDocument()
    expect(
      statuses[2].querySelector('.loading-state__progress-track'),
    ).not.toBeInTheDocument()
    expect(container.querySelectorAll('.loading-state__progress')).toHaveLength(
      2,
    )
  })

  test('renders permission guidance and optional controls', () => {
    render(
      <PermissionGate
        detail="Full Disk Access is still a manual decision on macOS and should stay inspectable."
        eyebrow="PERMISSIONS"
        title="Browser access needs review"
      >
        <button type="button">Preview native schedule</button>
      </PermissionGate>,
    )

    expect(screen.getByText('PERMISSIONS')).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'Preview native schedule' }),
    ).toBeVisible()
  })

  test('omits optional action regions when no controls are provided', () => {
    const { container } = render(
      <>
        <EmptyState
          description="Explorer scaffolding still needs its timeline and query panes."
          eyebrow="EXPLORER"
          title="History Explorer"
        />
        <PermissionGate
          detail="Permission review can render as pure guidance when no action is currently available."
          eyebrow="PERMISSIONS"
          title="Manual review required"
        />
      </>,
    )

    expect(container.querySelectorAll('.utility-block__actions')).toHaveLength(
      0,
    )
  })

  test('renders reusable severity callouts', () => {
    render(
      <StatusCallout
        tone="blocked"
        eyebrow="TRUST"
        title="Scheduler needs review"
        body="Mismatch and manual-review states should stay visible until the user re-checks the plan."
      />,
    )

    expect(screen.getByText('TRUST')).toBeVisible()
    expect(
      screen.getByRole('heading', { name: 'Scheduler needs review' }),
    ).toBeVisible()
  })

  test('renders dashboard and table skeletons for long-running loads', () => {
    const { container } = render(
      <>
        <Skeleton width="48px" />
        <DashboardSkeleton label="Loading dashboard" />
        <TableSkeleton label="Loading table" rows={3} />
      </>,
    )

    expect(container.querySelector('.skeleton--text')).toHaveStyle({
      height: '12px',
      width: '48px',
    })

    const dashboard = screen.getByLabelText('Loading dashboard')
    expect(dashboard).toHaveAttribute('aria-busy', 'true')
    const dashboardStats = dashboard.querySelectorAll('.skeleton--stat-card')
    expect(dashboardStats).toHaveLength(4)
    const statLines = dashboardStats[0]?.querySelectorAll('.skeleton__line')
    expect(statLines).toHaveLength(3)
    expect(statLines?.[0]).toHaveStyle({ height: '10px', width: '56%' })
    expect(statLines?.[1]).toHaveStyle({
      height: '24px',
      marginTop: '8px',
      width: '42%',
    })
    expect(statLines?.[2]).toHaveStyle({
      height: '10px',
      marginTop: '8px',
      width: '48%',
    })
    expect(
      Array.from(
        dashboard.querySelectorAll('.dashboard-left .skeleton--block'),
      ),
    ).toHaveLength(2)
    expect(
      Array.from(
        dashboard.querySelectorAll<HTMLElement>(
          '.dashboard-left .skeleton--block',
        ),
      ).map((block) => block.style.height),
    ).toEqual(['260px', '182px'])
    expect(
      Array.from(
        dashboard.querySelectorAll<HTMLElement>(
          '.dashboard-right .skeleton--block',
        ),
      ).map((block) => block.style.height),
    ).toEqual(['196px', '196px'])

    const table = screen.getByLabelText('Loading table')
    expect(table).toHaveAttribute('aria-busy', 'true')
    const tableRows = table.querySelectorAll('.skeleton--table-row')
    expect(tableRows).toHaveLength(3)
    expect(
      Array.from(
        tableRows[0]?.querySelectorAll<HTMLElement>('.skeleton__line') ?? [],
      ).map((line) => line.style.width),
    ).toEqual(['16%', '34%', '22%', '10%'])
  })

  test('renders block and text skeletons with explicit dimensions', () => {
    render(
      <>
        <Skeleton height="24px" variant="text" width="64px" />
        <Skeleton height="80px" variant="block" width="120px" />
      </>,
    )

    expect(document.querySelector('.skeleton--text')).toHaveStyle({
      height: '24px',
      width: '64px',
    })
    expect(document.querySelector('.skeleton--block')).toHaveStyle({
      height: '80px',
      width: '120px',
    })
  })

  test('renders block and text skeleton default dimensions', () => {
    render(
      <>
        <Skeleton variant="text" />
        <Skeleton variant="block" />
      </>,
    )

    expect(document.querySelector('.skeleton--text')).toHaveStyle({
      height: '12px',
      width: '100%',
    })
    expect(document.querySelector('.skeleton--block')).toHaveStyle({
      height: '120px',
      width: '100%',
    })
  })

  test('uses stable per-unit keys for repeated skeleton units', () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

    try {
      const { rerender } = render(<Skeleton count={2} variant="text" />)
      rerender(<Skeleton count={2} variant="block" />)

      expect(document.querySelectorAll('.skeleton')).toHaveLength(2)
      expect(errorSpy).not.toHaveBeenCalled()
    } finally {
      errorSpy.mockRestore()
    }
  })

  test('renders route-level skeleton variants for Explorer and Intelligence surfaces', () => {
    render(
      <>
        <SkeletonExplorer label="Loading explorer" />
        <SkeletonExplorerResults label="Loading explorer results" />
        <SkeletonInsights label="Loading intelligence" />
      </>,
    )

    expect(screen.getByLabelText('Loading explorer')).toHaveAttribute(
      'aria-busy',
      'true',
    )
    const explorer = screen.getByLabelText('Loading explorer')
    const explorerHeader =
      explorer.querySelector<HTMLElement>('.skeleton-block')
    expect(explorerHeader).toHaveStyle({
      height: '44px',
      marginBottom: 'var(--space-4)',
    })
    const explorerLayout = explorerHeader?.nextElementSibling as HTMLElement
    expect(explorerLayout).toHaveStyle({
      display: 'flex',
      gap: 'var(--space-4)',
    })
    const [explorerList, explorerDetail] = Array.from(
      explorerLayout.children,
    ) as HTMLElement[]
    expect(explorerList).toHaveStyle({ flex: '1' })
    expect(explorerDetail).toHaveStyle({ width: '320px' })
    expect(explorerList?.querySelector('.skeleton--block')).toHaveStyle({
      height: '32px',
      width: '100%',
    })
    expect(explorerDetail?.querySelector('.skeleton--block')).toHaveStyle({
      height: '200px',
      width: '100%',
    })
    const explorerRows =
      explorerList?.querySelectorAll<HTMLElement>('.skeleton-block')
    expect(explorerRows).toHaveLength(6)
    explorerRows?.forEach((row) => {
      expect(row).toHaveStyle({
        height: '48px',
        marginBottom: 'var(--space-2)',
      })
    })

    const explorerResults = screen.getByLabelText('Loading explorer results')
    expect(explorerResults).toHaveAttribute('aria-busy', 'true')
    expect(screen.getByTestId('explorer-results-skeleton')).toBeVisible()
    expect(explorerResults).toHaveClass('explorer-grid--skeleton')
    expect(
      explorerResults.querySelector('.record-group-header .skeleton-block'),
    ).toHaveStyle({ height: '18px', width: '42%' })
    const resultRowsPanel = explorerResults.querySelector<HTMLElement>(
      '.record-group .panel-body',
    )
    expect(resultRowsPanel).toHaveStyle({ padding: '0' })
    const resultRows =
      resultRowsPanel?.querySelectorAll<HTMLElement>('.skeleton-block')
    expect(resultRows).toHaveLength(6)
    resultRows?.forEach((row) => {
      expect(row).toHaveStyle({
        height: '70px',
        marginBottom: 'var(--space-2)',
      })
    })
    expect(
      explorerResults.querySelector('.panel-header .skeleton-block'),
    ).toHaveStyle({ height: '18px', width: '36%' })
    expect(
      Array.from(
        explorerResults.querySelectorAll<HTMLElement>(
          '.intelligence-stack .skeleton--block',
        ),
      ).map((block) => block.style.height),
    ).toEqual(['120px', '80px', '160px'])

    const intelligence = screen.getByLabelText('Loading intelligence')
    expect(intelligence).toHaveAttribute('aria-busy', 'true')
    expect(
      intelligence.querySelectorAll('.stats-row .skeleton--stat-card'),
    ).toHaveLength(4)
    const intelligenceLayout = intelligence.children[1] as HTMLElement
    expect(intelligenceLayout).toHaveStyle({
      display: 'flex',
      gap: 'var(--space-4)',
      marginTop: 'var(--space-4)',
    })
    const intelligenceColumns = Array.from(
      intelligenceLayout.children,
    ) as HTMLElement[]
    expect(intelligenceColumns).toHaveLength(2)
    intelligenceColumns.forEach((column) => {
      expect(column).toHaveStyle({ flex: '1' })
      expect(column.querySelector('.skeleton--block')).toHaveStyle({
        height: '220px',
        width: '100%',
      })
    })
    const intelligenceFooter = intelligence.children[2] as HTMLElement
    expect(intelligenceFooter).toHaveStyle({ marginTop: 'var(--space-4)' })
    expect(intelligenceFooter.querySelector('.skeleton--block')).toHaveStyle({
      height: '160px',
      width: '100%',
    })
  })
})
