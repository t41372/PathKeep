/**
 * This test file protects the reusable QueryFamilyCard interaction contract.
 *
 * Why this file exists:
 * - Query-family summaries appear in multiple Intelligence surfaces and should share one expansion/link behavior.
 * - The expand button deliberately stops link navigation, so it needs direct regression coverage.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Keep assertions focused on shared card behavior rather than route-specific parent copy.
 */

import userEvent from '@testing-library/user-event'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import type { QueryFamily } from '../../lib/core-intelligence'
import { I18nProvider } from '../../lib/i18n'
import { QueryFamilyCard } from './query-family-card'

const family: QueryFamily = {
  familyId: 'family:research',
  anchorQuery: 'pathkeep',
  memberCount: 5,
  searchEngine: 'Google',
  queries: [
    'pathkeep',
    'pathkeep tauri',
    'pathkeep sqlite',
    'pathkeep rust',
    'pathkeep tests',
  ],
  firstSeenAt: '2026-04-01T10:00:00.000Z',
  lastSeenAt: '2026-04-05T10:00:00.000Z',
}

function renderCard(card: React.ReactElement) {
  render(
    <I18nProvider>
      <MemoryRouter>{card}</MemoryRouter>
    </I18nProvider>,
  )
}

describe('QueryFamilyCard', () => {
  test('expands hidden query members without triggering card navigation', async () => {
    const user = userEvent.setup()
    renderCard(
      <QueryFamilyCard
        family={family}
        href="/intelligence/query-family/family%3Aresearch"
        linkMode="card"
        memberCountLabel="queries"
        moreLabel={(hiddenCount) => `Show ${hiddenCount} more`}
      />,
    )

    expect(screen.queryByText('"pathkeep rust"')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Show 2 more' }))

    expect(screen.getByText('"pathkeep rust"')).toBeVisible()
    expect(screen.getByText('"pathkeep tests"')).toBeVisible()
    expect(screen.getByRole('link', { name: /pathkeep/i })).toHaveAttribute(
      'href',
      '/intelligence/query-family/family%3Aresearch',
    )
  })

  test('can hide members, dates, and anchor links for compact consumers', () => {
    renderCard(
      <QueryFamilyCard
        family={family}
        href="/intelligence/query-family/family%3Aresearch"
        linkMode="none"
        memberCountLabel="queries"
        showAnchor={false}
        showDates={false}
        showMembers={false}
      />,
    )

    expect(screen.getByText('Google')).toBeVisible()
    expect(screen.getByText('5 queries')).toBeVisible()
    expect(screen.queryByText('"pathkeep"')).not.toBeInTheDocument()
    expect(screen.queryByRole('link')).not.toBeInTheDocument()
  })

  test('renders anchor-mode cards with raw date fallbacks and route footers', () => {
    renderCard(
      <QueryFamilyCard
        family={{
          ...family,
          firstSeenAt: 'not-a-date',
          lastSeenAt: 'still-not-a-date',
        }}
        footer={<span>Route footer</span>}
        href="/intelligence/query-family/family%3Aresearch"
        linkMode="anchor"
        memberCountLabel="queries"
      />,
    )

    expect(screen.getByRole('link', { name: '"pathkeep"' })).toHaveAttribute(
      'href',
      '/intelligence/query-family/family%3Aresearch',
    )
    expect(screen.getByText('not-a-date - still-not-a-date')).toBeVisible()
    expect(screen.getByText('Route footer')).toBeVisible()
  })
})
