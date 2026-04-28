/**
 * Protects the shared shell route error boundary from regressing back to React Router's raw crash screen.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, data, RouterProvider } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { ShellRouteErrorBoundary } from './shell-route-error-boundary'
import { createNamespaceTranslator, I18nProvider } from '../lib/i18n'

const commonT = createNamespaceTranslator('en', 'common')
const jobsT = createNamespaceTranslator('en', 'jobs')

function BrokenRoute(): never {
  throw new Error('route exploded')
}

function BrokenRouteWithoutStack(): never {
  const error = new Error('message only failure')
  error.stack = undefined
  throw error
}

function StringRoute(): never {
  // eslint-disable-next-line @typescript-eslint/only-throw-error -- React Router can surface non-Error thrown values from route modules; the boundary must degrade them safely.
  throw 'plain route failure'
}

function ObjectRoute(): never {
  // eslint-disable-next-line @typescript-eslint/only-throw-error -- React Router can surface non-Error thrown values from route modules; the boundary must degrade them safely.
  throw { reason: 'opaque route failure' }
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
    expect(screen.getByText(commonT('routeRenderErrorEyebrow'))).toBeVisible()
    expect(screen.getByText(commonT('routeRenderErrorBody'))).toBeVisible()
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

  test('shows response body details and recovery actions', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          loader: () => {
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- React Router loaders intentionally throw Response objects for route errors.
            throw new Response('loader body', {
              status: 418,
              statusText: "I'm a teapot",
            })
          },
          element: <div />,
          hydrateFallbackElement: <div />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
        {
          path: '/overview',
          element: <div>overview page</div>,
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

    expect(await screen.findByText('loader body')).toBeInTheDocument()
    const detailsTitle = screen.getByText(commonT('routeRenderErrorDetails'))
    expect(detailsTitle).toBeVisible()
    expect(detailsTitle.closest('summary')).toHaveStyle({ cursor: 'pointer' })
    expect(detailsTitle.closest('details')).toHaveStyle({
      marginTop: 'var(--space-4)',
    })
    expect(
      screen.getByRole('button', {
        name: commonT('routeRenderErrorRetry'),
      }),
    ).toBeVisible()
    const reloadSpy = vi
      .spyOn(window.history, 'go')
      .mockImplementation(() => undefined)
    fireEvent.click(
      screen.getByRole('button', {
        name: commonT('routeRenderErrorRetry'),
      }),
    )
    expect(reloadSpy).toHaveBeenCalledTimes(1)
    expect(reloadSpy).toHaveBeenCalledWith(0)
    reloadSpy.mockRestore()
    expect(
      screen.getByRole('link', {
        name: commonT('routeRenderErrorOverview'),
      }),
    ).toHaveAttribute('href', '/')

    unmountRouter(router)
  })

  test('falls back to response status text and raw string details', async () => {
    const statusRouter = createMemoryRouter(
      [
        {
          path: '/',
          loader: () => {
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- React Router loaders intentionally throw Response objects for route errors.
            throw new Response('   ', {
              status: 409,
              statusText: 'Conflict',
            })
          },
          element: <div />,
          hydrateFallbackElement: <div />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
      ],
      { initialEntries: ['/'] },
    )

    const { unmount } = render(
      <I18nProvider>
        <RouterProvider router={statusRouter} />
      </I18nProvider>,
    )
    expect(await screen.findByText('409 Conflict')).toBeInTheDocument()
    unmount()

    const trimmedRouter = createMemoryRouter(
      [
        {
          path: '/',
          loader: () => {
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- React Router loaders intentionally throw Response objects for route errors.
            throw new Response(null, {
              status: 410,
              statusText: '',
            })
          },
          element: <div />,
          hydrateFallbackElement: <div />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
      ],
      { initialEntries: ['/'] },
    )

    const trimmedView = render(
      <I18nProvider>
        <RouterProvider router={trimmedRouter} />
      </I18nProvider>,
    )
    await screen.findByTestId('shell-route-error-boundary')
    expect(trimmedView.container.querySelector('code')?.textContent).toBe('410')
    trimmedView.unmount()

    const stringRouter = createMemoryRouter(
      [
        {
          path: '/',
          element: <StringRoute />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <I18nProvider>
        <RouterProvider router={stringRouter} />
      </I18nProvider>,
    )
    expect(await screen.findByText('plain route failure')).toBeInTheDocument()
  })

  test('falls back to status text for structured route response bodies', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          loader: () => {
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- React Router loaders intentionally throw structured route data.
            throw data(
              { reason: 'structured failure' },
              {
                status: 500,
                statusText: 'Structured',
              },
            )
          },
          element: <div />,
          hydrateFallbackElement: <div />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <I18nProvider>
        <RouterProvider router={router} />
      </I18nProvider>,
    )

    expect(await screen.findByText('500 Structured')).toBeInTheDocument()
    expect(screen.queryByText('structured failure')).not.toBeInTheDocument()
  })

  test('updates technical details when the matched route error changes', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/first',
          element: <BrokenRoute />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
        {
          path: '/second',
          element: <BrokenRouteWithoutStack />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
      ],
      { initialEntries: ['/first'] },
    )

    render(
      <I18nProvider>
        <RouterProvider router={router} />
      </I18nProvider>,
    )

    expect(await screen.findByText(/route exploded/)).toBeInTheDocument()
    await router.navigate('/second')
    expect(await screen.findByText('message only failure')).toBeInTheDocument()
    expect(screen.queryByText(/route exploded/)).not.toBeInTheDocument()
  })

  test('omits technical details for opaque route errors', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <ObjectRoute />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
      ],
      { initialEntries: ['/'] },
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
      screen.queryByText(commonT('routeRenderErrorDetails')),
    ).not.toBeInTheDocument()
  })

  test('uses Error.message when stack details are unavailable', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <BrokenRouteWithoutStack />,
          ErrorBoundary: ShellRouteErrorBoundary,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <I18nProvider>
        <RouterProvider router={router} />
      </I18nProvider>,
    )

    expect(await screen.findByText('message only failure')).toBeInTheDocument()
  })
})

function unmountRouter(router: ReturnType<typeof createMemoryRouter>) {
  router.dispose()
}
