import { render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { beforeEach, describe, expect, test } from 'vitest'
import { ShellDataProvider } from '../../app/shell-data'
import { backendTestHarness } from '../../lib/backend'
import { Sidebar } from './index'

describe('Sidebar', () => {
  beforeEach(() => {
    backendTestHarness.reset()
  })

  test('renders the product name, sections, and archive status', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <ShellDataProvider>
        <RouterProvider router={router} />
      </ShellDataProvider>,
    )

    expect(screen.getByText('PATHKEEP')).toBeVisible()
    expect(screen.getByText('CORE')).toBeVisible()
    expect(screen.getByRole('link', { name: 'Dashboard' })).toHaveClass(
      'nav-item',
    )
    expect(screen.getByRole('link', { name: 'Dashboard' })).toHaveClass(
      'nav-item--active',
    )
    expect(await screen.findByText('Archive not initialized')).toBeVisible()
    expect(screen.getByText('Encrypted archive')).toBeVisible()
    expect(screen.getByText('0 B')).toBeVisible()
  })

  test('renders the optional assistant badge', () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <ShellDataProvider>
        <RouterProvider router={router} />
      </ShellDataProvider>,
    )

    expect(screen.getByText('OPT')).toBeVisible()
  })

  test('keeps the root link inactive when another route is selected', () => {
    const router = createMemoryRouter(
      [
        {
          path: '*',
          element: <Sidebar collapsed={false} onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/explorer'] },
    )

    render(
      <ShellDataProvider>
        <RouterProvider router={router} />
      </ShellDataProvider>,
    )

    expect(screen.getByRole('link', { name: 'Dashboard' })).not.toHaveClass(
      'nav-item--active',
    )
    expect(screen.getByRole('link', { name: 'Explorer' })).toHaveClass(
      'nav-item--active',
    )
  })

  test('keeps navigation accessible when the sidebar is collapsed', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar collapsed onToggle={() => {}} />,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(
      <ShellDataProvider>
        <RouterProvider router={router} />
      </ShellDataProvider>,
    )

    expect(screen.getByRole('link', { name: 'Dashboard' })).toBeVisible()
    expect(await screen.findByLabelText('Expand navigation')).toBeVisible()
    expect(screen.getByText('PATHKEEP')).toHaveClass('logo-name')
    expect(screen.getByText('Dashboard')).toHaveAttribute('aria-hidden', 'true')
  })
})
