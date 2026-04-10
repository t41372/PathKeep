import { beforeEach, describe, expect, test, vi } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/react'
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
import { createNamespaceTranslator, createTranslator } from '../lib/i18n'
import type { AppConfig } from '../lib/types'

const commonT = createTranslator('en')
const dashboardT = createNamespaceTranslator('en', 'dashboard')
const shellT = createNamespaceTranslator('en', 'shell')
const onboardingT = createNamespaceTranslator('en', 'onboarding')
const assistantT = createNamespaceTranslator('en', 'assistant')
const insightsT = createNamespaceTranslator('en', 'insights')
const scheduleT = createNamespaceTranslator('en', 'schedule')
const securityT = createNamespaceTranslator('en', 'security')
const settingsT = createNamespaceTranslator('en', 'settings')

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
  appLock: {
    enabled: false,
    idleTimeoutMinutes: 5,
    biometricEnabled: false,
    passcodeEnabled: true,
    passcodeConfigured: false,
    recoveryHint: null,
  },
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
  enrichment: {
    plugins: [
      {
        id: 'readable-content-refetch',
        enabled: true,
        version: 'm4-v1',
      },
    ],
  },
  ai: {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
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

async function seedAiProviders() {
  const snapshot = await backend.getAppSnapshot()
  await backend.saveConfig({
    ...snapshot.config,
    ai: {
      ...snapshot.config.ai,
      enabled: true,
      llmProviderId: 'llm-local',
      llmProviders: [
        {
          id: 'llm-local',
          name: 'Local LLM',
          purpose: 'llm',
          requestFormat: 'openai',
          enabled: true,
          baseUrl: 'http://localhost:11434',
          apiKeySaved: false,
          defaultModel: 'qwen3:8b',
          modelCatalog: [],
          temperature: 0.2,
          maxTokens: 1200,
          dimensions: null,
          notes: null,
        },
      ],
      embeddingProviders: [],
    },
  })
}

function expectHtmlElement(node: Element | null): HTMLElement {
  expect(node).toBeInstanceOf(HTMLElement)
  return node as HTMLElement
}

function seedInteractiveSchedule() {
  backendTestHarness.seedSchedule(
    {
      platform: 'macos',
      label: 'dev.codex.pathkeep.backup',
      executablePath: '/Applications/PathKeep.app',
      generatedFiles: [
        {
          relativePath: 'schedule/dev.codex.pathkeep.backup.plist',
          absolutePath:
            '/Users/test/Library/LaunchAgents/dev.codex.pathkeep.backup.plist',
          purpose: 'LaunchAgent plist',
          contents:
            '<?xml version="1.0"?><plist><dict><key>Label</key><string>dev.codex.pathkeep.backup</string></dict></plist>',
        },
      ],
      manualSteps: ['Review the LaunchAgent install.'],
      applyCommands: [['launchctl', 'bootstrap']],
      rollbackCommands: [['launchctl', 'bootout']],
      applySupported: true,
    },
    {
      platform: 'macos',
      label: 'dev.codex.pathkeep.backup',
      dueAfterHours: 72,
      checkIntervalHours: 6,
      applySupported: true,
      installState: 'installed',
      detectedFiles: ['~/Library/LaunchAgents/dev.codex.pathkeep.backup.plist'],
      manualSteps: ['Remove the LaunchAgent if you no longer want automation.'],
      auditPath: null,
      lastSuccessfulBackupAt: null,
      warnings: [],
    },
  )
}

describe('App shell', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
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
      await screen.findByRole('heading', {
        name: dashboardT('zeroStateTitle'),
      }),
    ).toBeVisible()

    await user.click(
      screen.getByRole('link', { name: dashboardT('openOnboardingFlow') }),
    )

    expect(await screen.findByTestId('onboarding-page')).toBeInTheDocument()
    expect(
      await screen.findByRole('button', { name: onboardingT('beginSetup') }),
    ).toBeVisible()
  })

  test('initializes the archive from onboarding and returns to a populated dashboard', async () => {
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/onboarding'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('onboarding-page')).toBeInTheDocument()

    expect(
      await screen.findByRole('button', { name: onboardingT('beginSetup') }),
    ).toBeVisible()
  })

  test('lets the user leave onboarding and resume later', async () => {
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/onboarding'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('onboarding-page')).toBeInTheDocument()
    expect(
      await screen.findByRole('button', { name: onboardingT('beginSetup') }),
    ).toBeVisible()
    expect(screen.getByText(/GPL v3/i)).toBeVisible()
    expect(screen.queryByText(/MIT licensed/i)).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: shellT('exitSetup') }))

    expect(await screen.findByTestId('dashboard-page')).toBeInTheDocument()
  })

  test('routes locked sessions to the lock screen and restores the requested route after unlock', async () => {
    const user = userEvent.setup()

    await seedArchiveRun()
    await backend.setAppLockPasscode({
      passcode: '2468',
      recoveryHint: 'digits only',
    })
    const snapshot = await backend.getAppSnapshot()
    await backend.saveConfig({
      ...snapshot.config,
      appLock: {
        ...snapshot.config.appLock,
        enabled: true,
        passcodeConfigured: true,
        recoveryHint: 'digits only',
      },
    })
    await backend.lockAppSession('startup')

    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/explorer?mode=keyword'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('lock-page')).toBeInTheDocument()
    expect(screen.queryByTestId('app-shell')).not.toBeInTheDocument()

    await user.type(screen.getByLabelText(shellT('lockPasscodeLabel')), '2468')
    await user.click(screen.getByRole('button', { name: shellT('unlockApp') }))

    expect(await screen.findByTestId('app-shell')).toBeInTheDocument()
    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()
  })

  test('supports explicit page jumps in explorer results and preserves the shell scroll position', async () => {
    const user = userEvent.setup()
    const baseTime = Date.now()

    await backend.initializeArchive(initializedConfig, 'vault-passphrase')
    backendTestHarness.mutateState((state) => {
      state.history.items = Array.from({ length: 375 }, (_, index) => ({
        id: index + 1,
        profileId: 'chrome:Default',
        url: `https://example.com/sqlite/${index + 1}`,
        title: `SQLite note ${index + 1}`,
        domain: 'example.com',
        visitedAt: new Date(baseTime - index * 60_000).toISOString(),
        visitTime: baseTime - index * 60_000,
        durationMs: 5_000,
        transition: 805306368,
        sourceVisitId: index + 1,
        appId: null,
      }))
    })

    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/explorer?q=sqlite&page=3'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()
    await waitFor(() =>
      expect(router.state.location.search).toContain('page=3'),
    )
    await waitFor(() =>
      expect(
        screen.getByRole('spinbutton', {
          name: 'Page number',
        }),
      ).toHaveValue(3),
    )
    await waitFor(() =>
      expect(document.querySelectorAll('.record-item')).toHaveLength(50),
    )

    const scrollContainer = document.querySelector('.workspace-scroll')
    expect(scrollContainer).toBeInstanceOf(HTMLElement)
    expectHtmlElement(scrollContainer).scrollTop = 240

    await user.click(screen.getByRole('button', { name: 'Next page' }))
    await waitFor(() =>
      expect(router.state.location.search).toContain('page=4'),
    )
    await waitFor(() =>
      expect(
        screen.getByRole('spinbutton', {
          name: 'Page number',
        }),
      ).toHaveValue(4),
    )
    expect(expectHtmlElement(scrollContainer).scrollTop).toBe(240)

    const pageInput = screen.getByRole('spinbutton', {
      name: 'Page number',
    })
    await user.clear(pageInput)
    await user.type(pageInput, '8')
    await user.click(screen.getByRole('button', { name: 'Go' }))

    await waitFor(() =>
      expect(router.state.location.search).toContain('page=8'),
    )
    await waitFor(() =>
      expect(
        screen.getByRole('spinbutton', {
          name: 'Page number',
        }),
      ).toHaveValue(8),
    )
    await waitFor(() =>
      expect(document.querySelectorAll('.record-item')).toHaveLength(25),
    )
    expect(screen.getByRole('button', { name: 'Last page' })).toBeDisabled()
  })

  test('submitting the topbar search navigates into explorer without crashing', async () => {
    const user = userEvent.setup()

    await seedArchiveRun()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('app-shell')).toBeInTheDocument()

    await user.type(
      screen.getByRole('searchbox', { name: 'Search history' }),
      'sqlite',
    )
    await user.keyboard('{Enter}')

    await waitFor(() =>
      expect(router.state.location.pathname).toBe('/explorer'),
    )
    await waitFor(() =>
      expect(router.state.location.search).toContain('q=sqlite'),
    )
    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()
    await waitFor(() =>
      expect(document.querySelectorAll('.record-item').length).toBeGreaterThan(
        0,
      ),
    )
  })

  test('switches archive mode from the onboarding security step', async () => {
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/onboarding'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('onboarding-page')).toBeInTheDocument()
    await user.click(
      await screen.findByRole('button', { name: onboardingT('beginSetup') }),
    )
    await user.click(
      screen.getByRole('button', { name: onboardingT('continueButton') }),
    )
    await user.click(
      screen.getByRole('button', { name: onboardingT('continueButton') }),
    )

    await user.click(screen.getByText(onboardingT('plaintextDesc')))
    expect(
      await screen.findByText(new RegExp(onboardingT('tradeoffNoPassword')), {
        selector: '.tradeoff-row',
      }),
    ).toBeVisible()

    await user.click(screen.getByText(onboardingT('encryptedDesc')))
    expect(
      await screen.findByText(onboardingT('masterPasswordLabel')),
    ).toBeVisible()
  })

  test('renders explorer filters, detail, export, and audit run detail from live shell data', async () => {
    await seedArchiveRun()
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/explorer?q=sqlite'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()

    await user.click(screen.getByRole('link', { name: 'Audit Ledger' }))

    expect(await screen.findByTestId('audit-page')).toBeInTheDocument()
  })

  test.each([
    {
      entry: '/schedule',
      pageTestId: 'schedule-page',
      sentinel: scheduleT('backupSchedule'),
      prepare: () => seedInteractiveSchedule(),
    },
    {
      entry: '/security',
      pageTestId: 'security-page',
      sentinel: securityT('encryptionStatus'),
    },
    {
      entry: '/assistant',
      pageTestId: null,
      sentinel: assistantT('disabledTitle'),
    },
    {
      entry: '/insights',
      pageTestId: 'insights-page',
      sentinel: insightsT('onThisDay'),
    },
    {
      entry: '/settings',
      pageTestId: 'settings-page',
      sentinel: settingsT('aiProvider'),
    },
  ])(
    'renders route $entry with live data-backed content',
    async ({ entry, pageTestId, sentinel, prepare }) => {
      await seedArchiveRun()
      prepare?.()
      const router = createMemoryRouter(appRoutes, {
        initialEntries: [entry],
      })

      render(<App router={router} />)

      if (!pageTestId) {
        expect(await screen.findByText(sentinel)).toBeVisible()
        return
      }

      const page = await screen.findByTestId(pageTestId)
      expect(await within(page).findByText(sentinel)).toBeVisible()
    },
  )

  test('walks the settings remote backup PME and derived-state controls', async () => {
    await seedArchiveRun()
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const settingsPage = await screen.findByTestId('settings-page')
    const remotePanel = expectHtmlElement(
      within(settingsPage)
        .getByText(settingsT('remoteBackup'))
        .closest('.panel'),
    )
    expect(
      within(settingsPage).getByText(settingsT('enrichmentDerivedState')),
    ).toBeVisible()
    expect(
      within(settingsPage).getByText(settingsT('archiveDatabase')),
    ).toBeVisible()
    expect(
      within(settingsPage).getByText(settingsT('auditRepository')),
    ).toBeVisible()
    expect(within(settingsPage).getByText(settingsT('gitCommit'))).toBeVisible()
    expect(
      within(settingsPage).getByText(commonT('common.previewTab')),
    ).toBeVisible()

    await user.clear(
      within(remotePanel).getByLabelText(settingsT('bucketLabel')),
    )
    await user.type(
      within(remotePanel).getByLabelText(settingsT('bucketLabel')),
      'example-bucket',
    )
    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('saveRemoteSettings'),
      }),
    )

    await user.type(
      within(remotePanel).getByLabelText(settingsT('accessKeyId')),
      'preview-key',
    )
    await user.type(
      within(remotePanel).getByLabelText(settingsT('secretAccessKey')),
      'preview-secret',
    )
    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('storeRemoteCredentials'),
      }),
    )

    await waitFor(() => {
      expect(
        within(remotePanel).getByText(settingsT('credentialsSaved')),
      ).toBeVisible()
    })

    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('previewRemoteBackup'),
      }),
    )

    await waitFor(() => {
      expect(
        within(remotePanel).getByText(settingsT('bundlePath')),
      ).toBeVisible()
      expect(
        within(remotePanel).getAllByText(/pathkeep-remote-.*\.zip/).length,
      ).toBeGreaterThan(0)
    })

    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('executeRemoteBackup'),
      }),
    )

    await waitFor(() => {
      expect(
        within(remotePanel).getByText(
          'Browser preview mode simulated the upload and produced a local bundle for verification.',
        ),
      ).toBeVisible()
    })

    await waitFor(() => {
      expect(
        within(remotePanel).getByRole('button', {
          name: settingsT('verifyRemoteBackup'),
        }),
      ).toBeEnabled()
    })

    await user.click(
      within(remotePanel).getByRole('button', {
        name: settingsT('verifyRemoteBackup'),
      }),
    )

    await waitFor(() => {
      expect(
        within(remotePanel).getByText(settingsT('bundleVersion')),
      ).toBeVisible()
      expect(
        within(remotePanel).getByText('pathkeep.remote-backup.v1'),
      ).toBeVisible()
    })

    await user.click(
      within(settingsPage).getByRole('button', {
        name: settingsT('disablePlugin'),
      }),
    )
    await waitFor(() => {
      expect(
        within(settingsPage).getByRole('button', {
          name: settingsT('enablePlugin'),
        }),
      ).toBeVisible()
    })

    await user.click(
      within(settingsPage).getByRole('button', {
        name: settingsT('clearDerivedState'),
      }),
    )
    await waitFor(() => {
      expect(
        within(settingsPage).getByText(settingsT('clearCompletedTitle')),
      ).toBeVisible()
    })

    await user.click(
      within(settingsPage).getByRole('button', {
        name: settingsT('rebuildDerivedState'),
      }),
    )
    await waitFor(() => {
      expect(
        within(settingsPage).getByText(settingsT('rebuildCompletedTitle')),
      ).toBeVisible()
    })
  })

  test('keeps AI provider field edits local until save is confirmed', async () => {
    await seedArchiveRun()
    await seedAiProviders()
    const user = userEvent.setup()
    const saveConfigSpy = vi.spyOn(backend, 'saveConfig')
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const settingsPage = await screen.findByTestId('settings-page')
    const aiPanel = expectHtmlElement(
      within(settingsPage).getByText(settingsT('aiProvider')).closest('.panel'),
    )
    const providerNameInput =
      within(settingsPage).getByDisplayValue('Local LLM')

    await user.clear(providerNameInput)
    await user.type(providerNameInput, 'Local LLM Draft')

    expect(saveConfigSpy).not.toHaveBeenCalled()
    expect(
      within(aiPanel).getByText(settingsT('aiUnsavedChanges')),
    ).toBeVisible()

    await user.click(
      within(aiPanel).getByRole('button', {
        name: settingsT('aiSaveConfig'),
      }),
    )

    await waitFor(() => {
      expect(saveConfigSpy).toHaveBeenCalledTimes(1)
    })
  })

  test('shows AI integration preview artifacts and consent boundaries in settings', async () => {
    await seedArchiveRun()
    await seedAiProviders()
    const user = userEvent.setup()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const settingsPage = await screen.findByTestId('settings-page')
    const aiPanel = expectHtmlElement(
      within(settingsPage).getByText(settingsT('aiProvider')).closest('.panel'),
    )

    expect(
      await within(aiPanel).findByText(settingsT('aiIntegrationReview')),
    ).toBeVisible()
    expect(
      within(aiPanel).getByText(settingsT('aiCapabilityNotes')),
    ).toBeVisible()
    expect(
      within(aiPanel).getByText(settingsT('aiGeneratedFiles')),
    ).toBeVisible()
    expect(
      within(aiPanel).getByRole('button', {
        name: 'integrations/pathkeep-mcp.json',
      }),
    ).toBeVisible()
    expect(within(aiPanel).getByText(/"mcpServers"/)).toBeVisible()

    await user.click(
      within(aiPanel).getByRole('button', {
        name: 'integrations/codex-pathkeep-skill/SKILL.md',
      }),
    )

    expect(within(aiPanel).getByText(/# PathKeep Search/)).toBeVisible()
  })

  test('keeps sidebar information architecture grouped by section', () => {
    expect(sidebarSections).toEqual([
      {
        id: 'core',
        labelKey: 'navigation.coreSection',
        items: [
          expect.objectContaining({
            id: 'dashboard',
            labelKey: 'navigation.dashboardLabel',
            subtitleKey: 'navigation.dashboardSubtitle',
            icon: '⌂',
            href: '/',
          }),
          expect.objectContaining({
            id: 'explorer',
            labelKey: 'navigation.explorerLabel',
            subtitleKey: 'navigation.explorerSubtitle',
            icon: '◎',
            href: '/explorer',
          }),
          expect.objectContaining({
            id: 'insights',
            labelKey: 'navigation.insightsLabel',
            subtitleKey: 'navigation.insightsSubtitle',
            icon: '◈',
            href: '/insights',
          }),
          expect.objectContaining({
            id: 'assistant',
            labelKey: 'navigation.assistantLabel',
            subtitleKey: 'navigation.assistantSubtitle',
            icon: '▷',
            href: '/assistant',
            badgeKey: 'navigation.assistantBadge',
          }),
        ],
      },
      {
        id: 'operations',
        labelKey: 'navigation.operationsSection',
        items: [
          expect.objectContaining({
            id: 'import',
            labelKey: 'navigation.importLabel',
            subtitleKey: 'navigation.importSubtitle',
            icon: '↓',
            href: '/import',
          }),
          expect.objectContaining({
            id: 'audit',
            labelKey: 'navigation.auditLabel',
            subtitleKey: 'navigation.auditSubtitle',
            icon: '⊞',
            href: '/audit',
          }),
          expect.objectContaining({
            id: 'schedule',
            labelKey: 'navigation.scheduleLabel',
            subtitleKey: 'navigation.scheduleSubtitle',
            icon: '⏀',
            href: '/schedule',
          }),
        ],
      },
      {
        id: 'system',
        labelKey: 'navigation.systemSection',
        items: [
          expect.objectContaining({
            id: 'security',
            labelKey: 'navigation.securityLabel',
            subtitleKey: 'navigation.securitySubtitle',
            icon: '⊘',
            href: '/security',
          }),
          expect.objectContaining({
            id: 'settings',
            labelKey: 'navigation.settingsLabel',
            subtitleKey: 'navigation.settingsSubtitle',
            icon: '⚙',
            href: '/settings',
          }),
        ],
      },
    ])
    expect(onboardingScreen).toEqual(
      expect.objectContaining({
        labelKey: 'navigation.onboardingLabel',
        titleKey: 'navigation.onboardingTitle',
        subtitleKey: 'navigation.onboardingSubtitle',
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
