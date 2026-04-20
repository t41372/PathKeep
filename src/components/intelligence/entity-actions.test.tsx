/**
 * This test file protects the shared intelligence route-action contract.
 *
 * Why this file exists:
 * - Route-first entity CTAs are reused by Intelligence and Explorer, so one
 *   broken href grammar can strand multiple desktop flows at once.
 * - The desktop shell runs through HashRouter, which means internal actions
 *   must render `#/...` links instead of bare pathnames.
 *
 * Main declarations:
 * - `CurrentPath`
 *
 * Source-of-truth notes:
 * - Shared entity CTA routing must stay aligned with `docs/design/screens-and-nav.md`
 *   and the route-first entity contract in `docs/features/intelligence.md`.
 */

import userEvent from '@testing-library/user-event'
import { render, screen } from '@testing-library/react'
import {
  createMemoryRouter,
  createHashRouter,
  RouterProvider,
  useLocation,
} from 'react-router-dom'
import { afterEach, describe, expect, test } from 'vitest'
import { InsightEntityActions } from './entity-actions'

function CurrentPath() {
  const location = useLocation()
  return <p>{`${location.pathname}${location.search}`}</p>
}

describe('InsightEntityActions', () => {
  afterEach(() => {
    window.location.hash = '#/'
  })

  test('routes internal actions through HashRouter href grammar', async () => {
    const user = userEvent.setup()
    window.location.hash = '#/'
    const router = createHashRouter([
      {
        path: '/',
        element: (
          <>
            <InsightEntityActions
              items={[
                {
                  href: '/explorer?domain=google.com&start=2026-03-20&end=2026-04-20',
                  label: 'Open evidence',
                },
              ]}
            />
            <CurrentPath />
          </>
        ),
      },
      {
        path: '/explorer',
        element: <CurrentPath />,
      },
    ])

    render(<RouterProvider router={router} />)

    const link = screen.getByRole('link', { name: 'Open evidence' })
    expect(link).toHaveAttribute(
      'href',
      '#/explorer?domain=google.com&start=2026-03-20&end=2026-04-20',
    )

    await user.click(link)

    expect(
      await screen.findByText(
        '/explorer?domain=google.com&start=2026-03-20&end=2026-04-20',
      ),
    ).toBeVisible()

    router.dispose()
  })

  test('keeps external links as plain anchors', () => {
    const router = createMemoryRouter([
      {
        path: '/',
        element: (
          <InsightEntityActions
            items={[
              {
                href: 'https://example.com/report',
                label: 'Open report',
                style: 'text',
              },
            ]}
          />
        ),
      },
    ])

    render(<RouterProvider router={router} />)

    expect(screen.getByRole('link', { name: 'Open report' })).toHaveAttribute(
      'href',
      'https://example.com/report',
    )

    router.dispose()
  })

  test('falls back to anchors for internal hrefs outside router context', () => {
    render(
      <InsightEntityActions
        items={[
          {
            href: '/explorer?domain=google.com',
            label: 'Open local evidence',
          },
        ]}
      />,
    )

    expect(
      screen.getByRole('link', { name: 'Open local evidence' }),
    ).toHaveAttribute('href', '/explorer?domain=google.com')
  })
})
