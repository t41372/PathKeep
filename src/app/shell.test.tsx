import { render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { beforeEach, describe, expect, test } from 'vitest'
import { AppShell } from './shell'
import { ShellDataProvider } from './shell-data'
import { I18nProvider } from '../lib/i18n'
import { backendTestHarness } from '../lib/backend'

describe('AppShell', () => {
  beforeEach(() => {
    backendTestHarness.reset()
  })

  test('falls back to the dashboard metadata when no route handle is present', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <AppShell />,
        },
      ],
      {
        initialEntries: ['/'],
      },
    )

    render(
      <I18nProvider>
        <ShellDataProvider>
          <RouterProvider router={router} />
        </ShellDataProvider>
      </I18nProvider>,
    )

    expect(
      await screen.findByRole('heading', { name: 'Dashboard' }),
    ).toBeVisible()
    expect(
      screen.getByRole('button', { name: /Initialize first/ }),
    ).toBeVisible()
  })
})
