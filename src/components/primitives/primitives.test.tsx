import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'vitest'
import { EmptyState } from './empty-state'
import { ErrorState } from './error-state'
import { LoadingState } from './loading-state'
import { PermissionGate } from './permission-gate'

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
        <LoadingState label="Rebuilding the semantic index" />
        <ErrorState
          description="The app should pause here and show rollback instructions."
          title="Schedule preview unavailable"
        />
      </>,
    )

    expect(screen.getByRole('status')).toHaveTextContent(
      'Rebuilding the semantic index',
    )
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
})
