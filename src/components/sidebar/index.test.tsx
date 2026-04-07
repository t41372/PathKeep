import { render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { Sidebar } from './index'

describe('Sidebar', () => {
  test('renders the product name, sections, and archive status', () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar />,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(<RouterProvider router={router} />)

    expect(screen.getByText('PATHKEEP')).toBeVisible()
    expect(screen.getByText('CORE')).toBeVisible()
    expect(screen.getByRole('link', { name: 'Dashboard' })).toHaveClass(
      'nav-item',
    )
    expect(screen.getByRole('link', { name: 'Dashboard' })).toHaveClass(
      'nav-item--active',
    )
    expect(screen.getByText('Archive healthy')).toBeVisible()
  })

  test('renders the optional assistant badge', () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Sidebar />,
        },
      ],
      { initialEntries: ['/'] },
    )

    render(<RouterProvider router={router} />)

    expect(screen.getByText('OPT')).toBeVisible()
  })

  test('keeps the root link inactive when another route is selected', () => {
    const router = createMemoryRouter(
      [
        {
          path: '*',
          element: <Sidebar />,
        },
      ],
      { initialEntries: ['/explorer'] },
    )

    render(<RouterProvider router={router} />)

    expect(screen.getByRole('link', { name: 'Dashboard' })).not.toHaveClass(
      'nav-item--active',
    )
    expect(screen.getByRole('link', { name: 'Explorer' })).toHaveClass(
      'nav-item--active',
    )
  })
})
