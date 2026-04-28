/**
 * @file entity-hero.test.tsx
 * @description Guards the promoted Intelligence entity hero chrome.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Verify the shared hero renders navigation, title, subtitle, and optional actions.
 * - Verify routes can omit action chrome without leaving empty wrappers.
 *
 * ## Not responsible for
 * - Testing route-specific target URLs or entity read models.
 *
 * ## Dependencies
 * - Uses MemoryRouter because the hero owns a first-party back link.
 *
 * ## Performance notes
 * - Static render assertions only.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { InsightEntityHero } from './entity-hero'

function renderHero(actions?: React.ReactNode) {
  return render(
    <MemoryRouter>
      <InsightEntityHero
        actions={actions}
        backHref="/intelligence"
        backLabel="Back"
        eyebrow="Entity"
        subtitle="Evidence backed"
        title="sqlite.org"
      />
    </MemoryRouter>,
  )
}

describe('InsightEntityHero', () => {
  test('renders hero actions when supplied', () => {
    renderHero(<button type="button">Open</button>)

    expect(screen.getByRole('link', { name: '← Back' })).toHaveAttribute(
      'href',
      '/intelligence',
    )
    expect(screen.getByRole('heading', { name: 'sqlite.org' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Open' })).toBeVisible()
  })

  test('omits the action wrapper when no actions are supplied', () => {
    const { container } = renderHero()

    expect(screen.getByText('Evidence backed')).toBeVisible()
    expect(container.querySelector('.day-insights__actions')).toBeNull()
  })
})
