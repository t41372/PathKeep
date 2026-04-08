import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { BusyOverlay } from './busy-overlay'
import { EmptyState } from './empty-state'
import { ErrorState } from './error-state'
import { LoadingState } from './loading-state'
import { PermissionGate } from './permission-gate'
import { DashboardSkeleton, TableSkeleton } from './skeleton'
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
        <BusyOverlay label="Applying native schedule changes" />
        <LoadingState label="Rebuilding the semantic index" />
        <ErrorState
          description="The app should pause here and show rollback instructions."
          title="Schedule preview unavailable"
        />
      </>,
    )

    const statuses = screen.getAllByRole('status')

    expect(statuses[0]).toHaveTextContent('Applying native schedule changes')
    expect(statuses[1]).toHaveTextContent('Rebuilding the semantic index')
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Schedule preview unavailable',
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
    render(
      <>
        <DashboardSkeleton />
        <TableSkeleton rows={3} />
      </>,
    )

    expect(screen.getByLabelText('Loading dashboard')).toHaveAttribute(
      'aria-busy',
      'true',
    )
    expect(screen.getByLabelText('Loading table')).toHaveAttribute(
      'aria-busy',
      'true',
    )
    expect(document.querySelectorAll('.skeleton--table-row')).toHaveLength(3)
  })
})
