/**
 * @file compare-set-page-list.test.tsx
 * @description Guards the shared compare-set page-list renderer.
 * @module components/intelligence
 *
 * ## Responsibilities
 * - Verify div and list variants preserve route links and landing badges.
 * - Verify optional max-item truncation and title fallback behavior.
 *
 * ## Not responsible for
 * - Re-testing promoted compare-set route data loading.
 *
 * ## Dependencies
 * - Uses MemoryRouter because compare-set page rows render route links.
 *
 * ## Performance notes
 * - Fixture rows are tiny and bounded.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import type { CompareSetPage } from '../../lib/core-intelligence'
import { CompareSetPageList } from './compare-set-page-list'

const pages: CompareSetPage[] = [
  {
    canonicalUrl: 'https://example.com/a',
    url: 'https://example.com/a',
    title: 'Example A',
    registrableDomain: 'example.com',
    visitCount: 3,
    isLanding: true,
  },
  {
    canonicalUrl: 'https://docs.example.com/b',
    url: 'https://docs.example.com/b',
    title: null,
    registrableDomain: 'docs.example.com',
    visitCount: 1,
    isLanding: false,
  },
]

function renderList(ui: React.ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>)
}

describe('CompareSetPageList', () => {
  test('renders div rows with landing badge and title fallback', () => {
    renderList(
      <CompareSetPageList
        getHref={(page) =>
          `/intelligence/refind/${encodeURIComponent(page.url)}`
        }
        keyPrefix="compare"
        landingLabel="Landing"
        pages={pages}
      />,
    )

    expect(screen.getByRole('link', { name: 'example.com' })).toHaveAttribute(
      'href',
      '/intelligence/refind/https%3A%2F%2Fexample.com%2Fa',
    )
    expect(screen.getByText('Landing')).toBeVisible()
    expect(screen.getByText('https://docs.example.com/b')).toHaveAttribute(
      'title',
      'https://docs.example.com/b',
    )
  })

  test('renders list rows and respects max item limits', () => {
    const { rerender } = renderList(
      <CompareSetPageList
        as="ul"
        getHref={(page) =>
          `/intelligence/refind/${encodeURIComponent(page.url)}`
        }
        keyPrefix="compare"
        landingLabel="Landing"
        maxItems={1}
        pages={pages}
      />,
    )

    expect(screen.getAllByRole('listitem')).toHaveLength(1)
    expect(
      screen.queryByText('https://docs.example.com/b'),
    ).not.toBeInTheDocument()

    rerender(
      <MemoryRouter>
        <CompareSetPageList
          as="ul"
          getHref={(page) =>
            `/intelligence/refind/${encodeURIComponent(page.url)}`
          }
          keyPrefix="compare"
          landingLabel="Landing"
          pages={pages}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText('https://docs.example.com/b')).toHaveAttribute(
      'title',
      'https://docs.example.com/b',
    )
  })
})
