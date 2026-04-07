import { beforeEach, describe, expect, test } from 'vitest'
import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter } from 'react-router-dom'
import App from './index'
import { createDesktopRouter } from './router-factory'
import {
  appRoutes,
  onboardingScreen,
  readRouteHandle,
  sidebarSections,
} from './router'
import { backend, backendTestHarness } from '../lib/backend'
import type { AppConfig } from '../lib/types'

const initializedConfig: AppConfig = {
  initialized: false,
  archiveMode: 'Encrypted',
  preferredLanguage: 'system',
  dueAfterHours: 72,
  scheduleCheckIntervalHours: 6,
  checkpointDays: 90,
  captureFavicons: true,
  selectedProfileIds: ['chrome:Default'],
  gitEnabled: true,
  rememberDatabaseKeyInKeyring: false,
  appAutostart: false,
  remoteBackup: {
    enabled: false,
    bucket: '',
    region: 'us-east-1',
    endpoint: null,
    prefix: 'pathkeep',
    pathStyle: true,
    uploadAfterBackup: false,
    credentialsSaved: false,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    lastError: null,
  },
  ai: {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt:
      'You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

async function seedArchiveRun() {
  await backend.initializeArchive(initializedConfig, 'vault-passphrase')
  await backend.runBackupNow(false)
}

describe('App shell', () => {
  beforeEach(() => {
    backendTestHarness.reset()
  })

  test('renders the dashboard zero state and routes into onboarding', async () => {
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('dashboard-page')).toBeInTheDocument()
    expect(
      screen.getByText('The first archive run still needs review'),
    ).toBeVisible()

    await user.click(screen.getByRole('link', { name: 'Review onboarding' }))

    expect(await screen.findByTestId('onboarding-page')).toBeInTheDocument()
    expect(screen.getByText('Profiles are the backup boundary')).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'Initialize + run first backup' }),
    ).toBeVisible()
  })

  test('initializes the archive from onboarding and returns to a populated dashboard', async () => {
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/onboarding'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('onboarding-page')).toBeInTheDocument()

    await user.type(
      screen.getByLabelText('MASTER PASSWORD'),
      'vault-passphrase',
    )
    await user.type(
      screen.getByLabelText('CONFIRM PASSWORD'),
      'vault-passphrase',
    )
    await user.click(
      screen.getByRole('button', { name: 'Initialize + run first backup' }),
    )

    expect(await screen.findByTestId('dashboard-page')).toBeInTheDocument()
    expect(await screen.findByText('RECENT RUNS')).toBeVisible()
    expect(screen.getByRole('link', { name: 'Detail' })).toBeVisible()
  })

  test('renders explorer filters, detail, export, and audit run detail from live shell data', async () => {
    await seedArchiveRun()
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/explorer?q=sqlite'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()
    const explorerPage = screen.getByTestId('explorer-page')
    const explorerMatches = await within(explorerPage).findAllByText(
      'SQLite inspection in browser developer tools',
    )
    expect(explorerMatches).toHaveLength(2)
    expect(screen.getByText('Canonical visit evidence')).toBeVisible()

    await user.click(screen.getByRole('button', { name: 'jsonl' }))

    expect(await screen.findByText(/pathkeep-export-/)).toBeVisible()

    await user.click(screen.getByRole('link', { name: 'Audit Ledger' }))

    expect(await screen.findByTestId('audit-page')).toBeInTheDocument()
    expect(await screen.findByText('RUN LEDGER')).toBeVisible()
    expect(screen.getByText('ARTIFACTS')).toBeVisible()
  })

  test.each([
    ['/schedule', 'SCHEDULE PREVIEW'],
    ['/security', 'ARCHIVE MODE'],
  ])(
    'renders route %s with live data-backed content',
    async (entry, sentinel) => {
      await seedArchiveRun()
      const router = createMemoryRouter(appRoutes, {
        initialEntries: [entry],
      })

      render(<App router={router} />)

      expect(await screen.findByText(sentinel)).toBeVisible()
    },
  )

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
})
