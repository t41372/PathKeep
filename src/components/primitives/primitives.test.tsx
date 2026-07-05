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
import { describe, expect, test } from 'vitest'
import { BusyOverlay } from './busy-overlay'
import { EmptyState } from './empty-state'
import { ErrorState } from './error-state'
import { LoadingState } from './loading-state'
import { PermissionGate } from './permission-gate'
import { DashboardSkeleton, Skeleton, SkeletonExplorer } from './skeleton'
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

  test('renders the base Skeleton as an aria-hidden shimmer box carrying caller-supplied sizing', () => {
    const { container } = render(<Skeleton className="h-[42px] w-full" />)

    const box = container.firstElementChild as HTMLElement
    expect(box).toHaveAttribute('aria-hidden', 'true')
    // The one canonical keyframe (reused, never re-declared) at the
    // ux-principles ~1.5s ease-in-out cadence, animating opacity only.
    expect(box.className).toContain(
      'animate-[pk-skeleton-pulse_1.5s_ease-in-out_infinite]',
    )
    expect(box.className).toContain('bg-border-light/60')
    expect(box.className).toContain('h-[42px]')
    expect(box.className).toContain('w-full')
  })

  test('DashboardSkeleton mirrors the real Dashboard page wrapper + grid structure exactly', () => {
    render(<DashboardSkeleton label="Loading dashboard" />)

    const dashboard = screen.getByLabelText('Loading dashboard')
    expect(dashboard).toHaveAttribute('aria-busy', 'true')
    // Same outer wrapper DashboardPage renders once ready — this is the
    // exact defect fix: no more legacy `.page-shell`/`.dashboard-grid` v0.2
    // classes here.
    expect(dashboard.className).toBe(
      'mx-auto flex w-full max-w-[1080px] flex-col pt-7',
    )

    // Hero band: same border/grid classes as the real HeroBand header.
    const hero = dashboard.firstElementChild as HTMLElement
    expect(hero.tagName).toBe('HEADER')
    expect(hero.className).toContain('grid-cols-1')
    expect(hero.className).toContain('lg:grid-cols-[1fr_auto]')
    expect(hero.className).toContain('border-b')

    // On This Day + This Week: the real `grid grid-cols-1 gap-4 mb-4
    // lg:grid-cols-2` row, with both cards present.
    const topGrid = hero.nextElementSibling as HTMLElement
    expect(topGrid.className).toContain('grid-cols-1')
    expect(topGrid.className).toContain('lg:grid-cols-2')
    expect(topGrid.className).toContain('gap-4')
    expect(topGrid.children).toHaveLength(2)
    expect(
      screen.getByTestId('dashboard-on-this-day-skeleton'),
    ).toBeInTheDocument()
    expect(
      screen.getByTestId('dashboard-this-week-skeleton'),
    ).toBeInTheDocument()

    // Year heatmap card, full width, between the two grids.
    expect(
      screen.getByTestId('dashboard-year-heatmap-skeleton'),
    ).toBeInTheDocument()

    // Active Threads + Archive card: the real `grid grid-cols-1 gap-4 mb-4
    // lg:grid-cols-3` row, with Active Threads spanning 2 columns.
    const bottomGrid = topGrid.nextElementSibling
      ?.nextElementSibling as HTMLElement
    expect(bottomGrid.className).toContain('grid-cols-1')
    expect(bottomGrid.className).toContain('lg:grid-cols-3')
    expect(bottomGrid.children).toHaveLength(2)
    const activeThreadsWrapper = bottomGrid.children[0] as HTMLElement
    expect(activeThreadsWrapper.className).toContain('lg:col-span-2')
    expect(
      screen.getByTestId('dashboard-active-threads-skeleton'),
    ).toBeInTheDocument()
    expect(
      screen.getByTestId('dashboard-archive-card-skeleton'),
    ).toBeInTheDocument()

    // Every shimmer placeholder is decorative — screen readers get the
    // single aria-busy/aria-label region, not dozens of empty boxes.
    const shimmerBoxes = dashboard.querySelectorAll('[aria-hidden="true"]')
    expect(shimmerBoxes.length).toBeGreaterThan(10)
  })

  test('SkeletonExplorer mirrors the real Explorer page-shell + Browse contact-sheet shapes', () => {
    render(<SkeletonExplorer label="Loading explorer" />)

    const explorer = screen.getByLabelText('Loading explorer')
    expect(explorer).toHaveAttribute('aria-busy', 'true')
    expect(explorer.tagName).toBe('SECTION')
    // Same outer wrapper the ready ExplorerPage renders.
    expect(explorer.className).toContain('page-shell')
    expect(explorer.className).toContain('explorer-page')

    // Single wrapper mirroring `PaperContactSheet`'s own root (`relative
    // flex w-full flex-col`) — the `.page-shell` grid's one child, so no
    // grid `gap` is injected between the toolbar/day-header/rows below.
    const contactSheetRoot = explorer.firstElementChild as HTMLElement
    expect(contactSheetRoot.className).toContain('flex')
    expect(contactSheetRoot.className).toContain('flex-col')

    // Sticky toolbar wrapper (mirrors PaperContactSheet's `-mx-7 px-7`
    // full-bleed sticky container).
    const toolbar = contactSheetRoot.firstElementChild as HTMLElement
    expect(toolbar.className).toContain('-mx-7')
    expect(toolbar.className).toContain('px-7')

    // Day-nav + view-toggle row (mirrors the contact-sheet's `h-[44px]`
    // nav/toggle row that sits above the filter strip).
    const navRow = toolbar.firstElementChild as HTMLElement
    expect(navRow.className).toContain('h-[44px]')
    expect(navRow.className).toContain('justify-between')

    // Filter-strip-shaped row (mirrors the real PaperFilterStrip wrapper in
    // explorer/index.tsx).
    const filterRow = navRow.nextElementSibling as HTMLElement
    const filterChipRow = filterRow.firstElementChild as HTMLElement
    expect(filterChipRow.className).toContain('flex-wrap')
    expect(filterChipRow.className).toContain('gap-x-3')

    // Sticky day-header-shaped row (mirrors PaperDayHeader's classes,
    // including its `-mx-7 px-7` full-bleed wrapper).
    const dayHeader = toolbar.nextElementSibling as HTMLElement
    expect(dayHeader.className).toContain('border-b-[2px]')
    expect(dayHeader.className).toContain('items-baseline')
    expect(dayHeader.className).toContain('-mx-7')

    // Contact-sheet rows (mirror PaperListRow's grid template exactly).
    const rowsContainer = dayHeader.nextElementSibling as HTMLElement
    const rows = Array.from(rowsContainer.children) as HTMLElement[]
    expect(rows).toHaveLength(7)
    rows.forEach((row) => {
      expect(row.className).toContain('grid-cols-[26px_1fr_auto]')
      expect(row.className).toContain('border-b')
    })

    // Decorative shimmer boxes stay out of the accessibility tree.
    const shimmerBoxes = explorer.querySelectorAll('[aria-hidden="true"]')
    expect(shimmerBoxes.length).toBeGreaterThan(10)
  })
})
