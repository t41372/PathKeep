import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import App from './index'
import { createDesktopRouter } from './router-factory'
import {
  appRoutes,
  onboardingScreen,
  readRouteHandle,
  sidebarSections,
} from './router'

describe('App shell', () => {
  test('renders the dashboard shell and navigates across core pages', async () => {
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/'],
    })

    render(<App router={router} />)

    expect(screen.getByTestId('app-shell')).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
    expect(screen.getByText('Archive healthy')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Backup Now' })).toBeVisible()
    expect(screen.getByText('RECENT RUNS')).toBeVisible()
    expect(screen.getByRole('link', { name: 'Dashboard' })).toHaveAttribute(
      'aria-current',
      'page',
    )

    await user.click(screen.getByRole('link', { name: 'Explorer' }))

    expect(
      screen.getByRole('heading', { level: 1, name: 'History Explorer' }),
    ).toBeVisible()
    expect(
      screen.getByText('Browse, search & filter your archive'),
    ).toBeVisible()
    expect(
      screen.getByText(/Time-travel and full-text search land here next\./),
    ).toBeVisible()
    expect(screen.getByRole('link', { name: 'Dashboard' })).not.toHaveAttribute(
      'aria-current',
    )

    await user.click(screen.getByRole('link', { name: /AI Assistant/ }))

    expect(screen.getByRole('heading', { name: 'AI Assistant' })).toBeVisible()
    expect(
      screen.getByText('Ask questions about your browsing history'),
    ).toBeVisible()
    expect(screen.getByText('AI stays optional')).toBeVisible()
  })

  test('renders the onboarding shell and routes back to the dashboard preview', async () => {
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/onboarding'],
    })

    render(<App router={router} />)

    expect(screen.getByTestId('onboarding-shell')).toBeInTheDocument()
    expect(
      screen.getByRole('heading', { name: 'Onboarding / Setup' }),
    ).toBeVisible()
    expect(screen.getByText('Preview native schedule')).toBeVisible()

    await user.click(screen.getByRole('link', { name: 'Skip onboarding' }))

    expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
  })

  test('keeps sidebar information architecture grouped by section', () => {
    expect(sidebarSections).toEqual([
      {
        label: 'CORE',
        items: [
          expect.objectContaining({
            id: 'dashboard',
            label: 'Dashboard',
            subtitle: 'Archive overview & system status',
            icon: '⌂',
            href: '/',
          }),
          expect.objectContaining({
            id: 'explorer',
            label: 'Explorer',
            subtitle: 'Browse, search & filter your archive',
            icon: '◎',
            href: '/explorer',
          }),
          expect.objectContaining({
            id: 'insights',
            label: 'Insights',
            subtitle: 'Topics, threads & browsing patterns',
            icon: '◈',
            href: '/insights',
          }),
          expect.objectContaining({
            id: 'assistant',
            label: 'AI Assistant',
            subtitle: 'Ask questions about your browsing history',
            icon: '▷',
            href: '/assistant',
            badge: 'OPT',
          }),
        ],
      },
      {
        label: 'OPERATIONS',
        items: [
          expect.objectContaining({
            id: 'import',
            label: 'Import',
            subtitle: 'Google Takeout & browser direct import',
            icon: '↓',
            href: '/import',
          }),
          expect.objectContaining({
            id: 'audit',
            label: 'Audit Ledger',
            subtitle: 'Manifest chain, run history & integrity',
            icon: '⊞',
            href: '/audit',
          }),
          expect.objectContaining({
            id: 'schedule',
            label: 'Schedule',
            subtitle: 'Backup schedule & install artifacts',
            icon: '⏀',
            href: '/schedule',
          }),
        ],
      },
      {
        label: 'SYSTEM',
        items: [
          expect.objectContaining({
            id: 'security',
            label: 'Security',
            subtitle: 'Encryption, keyring & password management',
            icon: '⊘',
            href: '/security',
          }),
          expect.objectContaining({
            id: 'settings',
            label: 'Settings',
            subtitle: 'Profiles, AI provider & general config',
            icon: '⚙',
            href: '/settings',
          }),
        ],
      },
    ])
    expect(onboardingScreen).toEqual(
      expect.objectContaining({
        label: 'Onboarding',
        title: 'Onboarding / Setup',
        subtitle: 'Preview, manual guidance, and first-run archive decisions',
        icon: '◌',
        href: '/onboarding',
      }),
    )
    expect(appRoutes[0]).toEqual(expect.objectContaining({ path: '/' }))
  })

  test.each([
    ['/insights', 'Insights', 'Review insight cards'],
    ['/import', 'Import', 'Inspect Takeout contents'],
    ['/audit', 'Audit Ledger', 'M1 engine'],
    ['/schedule', 'Schedule', 'Preview native schedule'],
    ['/security', 'Security', 'Review keyring preview'],
    [
      '/settings',
      'Settings',
      'Settings modules are being split out of the legacy context.',
    ],
  ])('renders shell route %s', (entry, title, sentinel) => {
    const router = createMemoryRouter(appRoutes, {
      initialEntries: [entry],
    })

    render(<App router={router} />)

    expect(screen.getByRole('heading', { level: 1, name: title })).toBeVisible()
    expect(
      screen.getByText((content) => content.includes(sentinel)),
    ).toBeVisible()
  })

  test('creates a desktop router and validates route handles', () => {
    const router = createDesktopRouter()

    expect(readRouteHandle(null)).toBeNull()
    expect(readRouteHandle({})).toBeNull()
    expect(readRouteHandle({ screen: null })).toBeNull()
    expect(readRouteHandle({ screen: 'dashboard' })).toBeNull()
    expect(readRouteHandle({ screen: onboardingScreen })).toEqual({
      screen: onboardingScreen,
    })
    expect(router.state.location.pathname).toBe('/')

    router.dispose()
  })

  test('redirects unknown routes back to the dashboard shell', () => {
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/missing'],
    })

    render(<App router={router} />)

    expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
  })
})
