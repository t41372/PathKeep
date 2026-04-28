import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import {
  RefindFactorList,
  RefindSummaryCard,
  WorkbenchEntityRow,
  WorkbenchExpandableGroupCard,
  WorkbenchTargetLinksRow,
} from '.'

describe('intelligence workbench primitives', () => {
  test('toggles shared refind factors without changing entity-first links', async () => {
    const user = userEvent.setup()

    render(
      <MemoryRouter>
        <RefindSummaryCard
          actionItems={[
            {
              href: '/intelligence/domain/sqlite.org?range=custom&start=2026-04-01&end=2026-04-01',
              label: 'sqlite.org',
              style: 'text',
            },
            {
              href: '/explorer?q=https%3A%2F%2Fsqlite.org%2Flang.html',
              label: 'Open Explorer',
              style: 'text',
            },
          ]}
          description="Seen on 4 different days and reopened from search 2 times."
          expandLabel="Show factors"
          factorRows={[
            { label: 'Cross-day revisits', valueLabel: '4 ×3', emphasis: 12 },
            {
              label: 'Distinct trail appearances',
              valueLabel: '2 ×3',
              emphasis: 6,
            },
          ]}
          scoreLabel="Score: 5.0"
          title="SQLite language reference"
          titleHref="/intelligence/refind/https%3A%2F%2Fsqlite.org%2Flang.html"
        />
      </MemoryRouter>,
    )

    expect(
      screen.getByRole('link', { name: 'SQLite language reference' }),
    ).toHaveAttribute(
      'href',
      '/intelligence/refind/https%3A%2F%2Fsqlite.org%2Flang.html',
    )
    expect(screen.getByRole('link', { name: 'sqlite.org' })).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-01&end=2026-04-01',
    )

    expect(screen.queryByText('Cross-day revisits')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /Show factors/i }))
    expect(screen.getByText('Cross-day revisits')).toBeVisible()
    expect(screen.getByText('4 ×3')).toBeVisible()
  })

  test('renders refind factors without bars for zero, negative, or missing emphasis', () => {
    render(
      <RefindFactorList
        factors={[
          { label: 'No signal', valueLabel: '0' },
          { label: 'Negative signal', valueLabel: '-1', emphasis: -1 },
          { label: 'Positive signal', valueLabel: '2', emphasis: 2 },
        ]}
      />,
    )

    expect(screen.getByText('No signal')).toBeVisible()
    expect(screen.getByText('Negative signal')).toBeVisible()
    expect(screen.getByText('Positive signal')).toBeVisible()
    expect(document.querySelectorAll('.refind-card__factor-bar')).toHaveLength(
      1,
    )
  })

  test('lets shared selectable rows handle click and keyboard selection', async () => {
    const user = userEvent.setup()
    const onSelect = vi.fn()

    render(
      <WorkbenchEntityRow
        className="session-visit-row"
        content="SQLite language reference"
        contentClassName="session-visit-row__content"
        icon="📄"
        iconClassName="session-visit-row__page-icon"
        meta="09:30"
        metaClassName="session-visit-row__time"
        onSelect={onSelect}
      />,
    )

    const row = screen.getByRole('button')
    await user.click(row)
    await user.keyboard('{Enter}')
    await user.keyboard(' ')
    fireEvent.keyDown(row, { key: 'Escape' })

    expect(onSelect).toHaveBeenCalledTimes(3)
  })

  test('keeps rows without a selection handler inert', () => {
    render(
      <WorkbenchEntityRow
        className="read-only-row"
        content="Read-only evidence"
        contentClassName="read-only-row__content"
        icon="R"
        iconClassName="read-only-row__icon"
        meta="09:30"
        metaClassName="read-only-row__meta"
      />,
    )

    const row = screen.getByText('Read-only evidence').closest('.read-only-row')
    expect(row).toBeInstanceOf(HTMLElement)
    fireEvent.keyDown(row as HTMLElement, { key: 'Enter' })
    expect(screen.getByText('Read-only evidence')).toBeVisible()
  })

  test('delegates expandable group-card toggles without owning route logic', async () => {
    const user = userEvent.setup()
    const onToggle = vi.fn()

    render(
      <WorkbenchExpandableGroupCard
        bodyClassName="session-card__body"
        expanded
        headerClassName="session-card__header"
        headerContent={<span>Session header</span>}
        onToggle={onToggle}
        rootClassName="session-card"
      >
        <div>Session body</div>
      </WorkbenchExpandableGroupCard>,
    )

    expect(screen.getByText('Session body')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Session header' }))
    expect(onToggle).toHaveBeenCalledTimes(1)
  })

  test('renders shared target-link rows with a primary insights link and secondary chips', () => {
    render(
      <MemoryRouter>
        <WorkbenchTargetLinksRow
          label="Open"
          primaryHref="/intelligence/day/2026-04-18?profileId=chrome%3ADefault"
          primaryLabel="Open insights"
          secondaryLinks={[
            {
              href: '/intelligence/domain/sqlite.org?range=custom&start=2026-04-18&end=2026-04-18&profileId=chrome%3ADefault',
              key: 'domain',
              label: 'sqlite.org',
            },
            {
              href: '/intelligence/query-family/family-1?range=custom&start=2026-04-01&end=2026-04-18&profileId=chrome%3ADefault',
              key: 'family',
              label: 'SQLite pragma',
            },
          ]}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText('Open')).toBeVisible()
    expect(screen.getByRole('link', { name: 'Open insights' })).toHaveAttribute(
      'href',
      '/intelligence/day/2026-04-18?profileId=chrome%3ADefault',
    )
    expect(screen.getByRole('link', { name: 'sqlite.org' })).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-18&end=2026-04-18&profileId=chrome%3ADefault',
    )
    expect(screen.getByRole('link', { name: 'SQLite pragma' })).toHaveAttribute(
      'href',
      '/intelligence/query-family/family-1?range=custom&start=2026-04-01&end=2026-04-18&profileId=chrome%3ADefault',
    )
  })
})
