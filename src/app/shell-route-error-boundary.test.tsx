/**
 * Protects the shared shell route error boundary from regressing back to React Router's raw crash screen.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { ShellRouteErrorBoundary } from './shell-route-error-boundary'
import { createNamespaceTranslator, I18nProvider } from '../lib/i18n'

const commonT = createNamespaceTranslator('en', 'common')
const jobsT = createNamespaceTranslator('en', 'jobs')

function BrokenRoute(): never {
  throw new Error('route exploded')
}

describe('ShellRouteErrorBoundary', () => {
  test('renders a product error state and lets users recover into Jobs', async () => {
    const user = userEvent.setup()
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <BrokenRoute />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
        {
          path: '/jobs',
          element: <div>jobs page</div>,
        },
      ],
      {
        initialEntries: ['/'],
      },
    )

    render(
      <I18nProvider>
        <RouterProvider router={router} />
      </I18nProvider>,
    )

    expect(
      await screen.findByTestId('shell-route-error-boundary'),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('heading', {
        name: commonT('routeRenderErrorTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.queryByText('Unexpected Application Error!'),
    ).not.toBeInTheDocument()

    await user.click(
      screen.getByRole('link', {
        name: jobsT('openJobs'),
      }),
    )

    expect(await screen.findByText('jobs page')).toBeVisible()
  })
})
