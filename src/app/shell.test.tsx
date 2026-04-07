import { render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { AppShell } from './shell'

describe('AppShell', () => {
  test('falls back to the dashboard metadata when no route handle is present', () => {
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

    render(<RouterProvider router={router} />)

    expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
    expect(screen.getByRole('button', { name: 'Backup Now' })).toBeVisible()
  })
})
