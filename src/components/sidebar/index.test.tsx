import { render, screen } from '@testing-library/react'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { beforeEach, describe, expect, test } from 'vitest'
import { ShellDataProvider } from '../../app/shell-data'
import { backendTestHarness } from '../../lib/backend'
import { I18nProvider } from '../../lib/i18n'
import { ProfileScopeProvider } from '../../lib/profile-scope'
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
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <RouterProvider router={router} />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nProvider>,
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
    expect(await screen.findByText('Encrypted archive')).toBeVisible()
    expect(screen.getByText('0 B')).toBeVisible()
    expect(screen.getByText('Profile scope: All profiles')).toBeVisible()
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
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <RouterProvider router={router} />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(screen.getByText('Optional')).toBeVisible()
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
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <RouterProvider router={router} />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nProvider>,
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
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <RouterProvider router={router} />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(screen.getByRole('link', { name: 'Dashboard' })).toBeVisible()
    expect(await screen.findByLabelText('Expand navigation')).toBeVisible()
    expect(screen.getByText('PATHKEEP')).toHaveClass('logo-name')
    expect(screen.getByText('Dashboard')).toHaveAttribute('aria-hidden', 'true')
  })
})
