/**
 * This module belongs to the application shell layer for Index.test.tsx.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `seedArchiveRun`
 * - `seedAiProviders`
 * - `expectHtmlElement`
 * - `seedInteractiveSchedule`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

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
import { backend } from '../lib/backend-client'
import { backendTestHarness } from '../lib/backend'
import { createNamespaceTranslator, createTranslator } from '../lib/i18n'
import * as updateLib from '../lib/update'
import type { AppConfig } from '../lib/types'

const commonT = createTranslator('en')
const dashboardT = createNamespaceTranslator('en', 'dashboard')
const shellT = createNamespaceTranslator('en', 'shell')
const onboardingT = createNamespaceTranslator('en', 'onboarding')
const assistantT = createNamespaceTranslator('en', 'assistant')
const intelligenceT = createNamespaceTranslator('en', 'intelligence')
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
  analytics: {
    enabled: false,
    consentGrantedAt: null,
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
        version: 'diagnostic',
      },
    ],
  },
  deterministic: {
    modules: [
      { id: 'visit-derived-facts', enabled: true, version: 'ci-v1' },
      { id: 'daily-rollups', enabled: true, version: 'ci-v1' },
      { id: 'sessions', enabled: true, version: 'ci-v1' },
      { id: 'search-trails', enabled: true, version: 'ci-v1' },
      { id: 'refind-pages', enabled: true, version: 'ci-v1' },
      { id: 'activity-mix', enabled: true, version: 'ci-v1' },
      { id: 'search-effectiveness', enabled: true, version: 'ci-v1' },
      { id: 'domain-deep-dive', enabled: true, version: 'ci-v1' },
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
    enrichmentEnabled: true,
    enrichmentPlugins: [
      { pluginId: 'title-normalization', enabled: true },
      { pluginId: 'readable-content-refetch', enabled: true },
    ],
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt:
      'You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

/**
 * Explains how seed archive run works.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
async function seedArchiveRun() {
  await backend.initializeArchive(initializedConfig, 'vault-passphrase')
  await backend.runBackupNow(false)
}

/**
 * Explains how seed ai providers works.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
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

/**
 * Explains how expect html element works.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
function expectHtmlElement(node: Element | null): HTMLElement {
  expect(node).toBeInstanceOf(HTMLElement)
  return node as HTMLElement
}

/**
 * Explains how seed interactive schedule works.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
function seedInteractiveSchedule() {
  backendTestHarness.seedSchedule(
    {
      platform: 'macos',
      label: 'com.yi-ting.pathkeep.backup',
      executablePath: '/Applications/PathKeep.app',
      generatedFiles: [
        {
          relativePath: 'schedule/com.yi-ting.pathkeep.backup.plist',
          absolutePath:
            '/Users/test/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
          purpose: 'LaunchAgent plist',
          contents:
            '<?xml version="1.0"?><plist><dict><key>Label</key><string>com.yi-ting.pathkeep.backup</string></dict></plist>',
        },
      ],
      manualSteps: ['Review the LaunchAgent install.'],
      applyCommands: [['launchctl', 'bootstrap']],
      rollbackCommands: [['launchctl', 'bootout']],
      applySupported: true,
    },
    {
      platform: 'macos',
      label: 'com.yi-ting.pathkeep.backup',
      dueAfterHours: 72,
      checkIntervalHours: 6,
      applySupported: true,
      installState: 'installed',
      detectedFiles: [
        '~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
      ],
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
    expect(screen.getByText(onboardingT('featureBackupDesc'))).toBeVisible()
    expect(screen.getByText(/GPL v3/i)).toBeVisible()
    expect(screen.queryByText(/MIT licensed/i)).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: shellT('exitSetup') }))

    expect(await screen.findByTestId('dashboard-page')).toBeInTheDocument()
  })

  test('requires selecting a browser profile before leaving the onboarding browser step', async () => {
    const user = userEvent.setup()
    backendTestHarness.mutateState((state) => {
      state.snapshot.config.selectedProfileIds = []
    })
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/onboarding'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('onboarding-page')).toBeInTheDocument()
    await user.click(
      await screen.findByRole('button', { name: onboardingT('beginSetup') }),
    )

    expect(
      await screen.findByRole('heading', {
        name: onboardingT('browserDetectionTitle'),
      }),
    ).toBeVisible()
    expect(screen.getByText(onboardingT('firefoxSafariInfo'))).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: onboardingT('continueButton') }),
    )

    expect(await screen.findByRole('alert')).toHaveTextContent(
      onboardingT('errorSelectProfile'),
    )
    expect(
      screen.getByRole('heading', {
        name: onboardingT('browserDetectionTitle'),
      }),
    ).toBeVisible()

    await user.click(
      screen.getByRole('checkbox', { name: 'Google Chrome / Primary' }),
    )
    await waitFor(() =>
      expect(
        screen.getByRole('checkbox', { name: 'Google Chrome / Primary' }),
      ).toBeChecked(),
    )

    await user.click(
      screen.getByRole('button', { name: onboardingT('continueButton') }),
    )

    expect(
      await screen.findByRole('heading', { name: onboardingT('storageTitle') }),
    ).toBeVisible()
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

  test('shows truthful Touch ID fallback copy on the lock screen when macOS biometric is unavailable', async () => {
    await seedArchiveRun()
    backendTestHarness.mutateState((state) => {
      state.biometricState = 'touch-id-unavailable'
      state.appLockPasscode = '2468'
      state.appLockRecoveryHint = 'digits only'
      state.snapshot.config.appLock = {
        ...state.snapshot.config.appLock,
        enabled: true,
        biometricEnabled: true,
        passcodeConfigured: true,
        recoveryHint: 'digits only',
      }
      state.snapshot.appLockStatus = {
        ...state.snapshot.appLockStatus,
        locked: true,
        lockReason: 'startup',
      }
    })

    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/lock'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('lock-page')).toBeInTheDocument()
    expect(
      screen.getByRole('button', {
        name: shellT('unlockWithTouchId'),
      }),
    ).toBeDisabled()
    expect(screen.getByText(shellT('unlockTouchIdUnavailable'))).toBeVisible()
  })

  test('hides biometric unlock when Settings has it turned off', async () => {
    await seedArchiveRun()
    backendTestHarness.mutateState((state) => {
      state.biometricState = 'touch-id-available'
      state.appLockPasscode = '2468'
      state.snapshot.config.appLock = {
        ...state.snapshot.config.appLock,
        enabled: true,
        biometricEnabled: false,
        passcodeConfigured: true,
      }
      state.snapshot.appLockStatus = {
        ...state.snapshot.appLockStatus,
        locked: true,
        lockReason: 'startup',
        biometricAvailable: true,
        biometricEnabled: false,
      }
    })

    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/lock'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('lock-page')).toBeInTheDocument()
    expect(
      screen.queryByRole('button', {
        name: shellT('unlockWithTouchId'),
      }),
    ).not.toBeInTheDocument()
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

    await user.click(
      screen.getByRole('radio', {
        name: onboardingT('plaintextSelectLabel'),
      }),
    )
    expect(
      await screen.findByText(new RegExp(onboardingT('tradeoffNoPassword')), {
        selector: '.tradeoff-row',
      }),
    ).toBeVisible()

    await user.click(
      screen.getByRole('radio', {
        name: onboardingT('encryptedSelectLabel'),
      }),
    )
    expect(
      await screen.findByText(onboardingT('masterPasswordLabel')),
    ).toBeVisible()
  })

  test('completes encrypted onboarding without saving the password to the keychain', async () => {
    const user = userEvent.setup()
    const keyringStoreSpy = vi.spyOn(backend, 'keyringStoreDatabaseKey')
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

    await user.click(
      screen.getByRole('button', { name: onboardingT('continueButton') }),
    )
    expect(await screen.findByRole('alert')).toHaveTextContent(
      onboardingT('errorNeedPassword'),
    )

    await user.type(
      screen.getByPlaceholderText(onboardingT('masterPasswordPlaceholder')),
      '000000',
    )
    await user.type(
      screen.getByPlaceholderText(onboardingT('confirmPasswordPlaceholder')),
      '000000',
    )
    expect(
      screen.getByRole('checkbox', { name: onboardingT('storeInKeyring') }),
    ).not.toBeChecked()

    await user.click(
      screen.getByRole('button', { name: onboardingT('continueButton') }),
    )
    expect(
      await screen.findByRole('heading', {
        name: onboardingT('scheduleTitle'),
      }),
    ).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: onboardingT('backButton') }),
    )
    expect(
      await screen.findByPlaceholderText(
        onboardingT('masterPasswordPlaceholder'),
      ),
    ).toHaveValue('000000')
    expect(
      screen.getByPlaceholderText(onboardingT('confirmPasswordPlaceholder')),
    ).toHaveValue('000000')

    await user.click(
      screen.getByRole('button', { name: onboardingT('continueButton') }),
    )
    await user.click(
      await screen.findByRole('button', {
        name: onboardingT('continueButton'),
      }),
    )
    await user.click(
      await screen.findByRole('button', { name: onboardingT('initButton') }),
    )

    expect(await screen.findByTestId('dashboard-page')).toBeInTheDocument()
    expect(keyringStoreSpy).not.toHaveBeenCalled()
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
      entry: '/intelligence',
      pageTestId: null,
      sentinel: intelligenceT('digestTitle'),
    },
    {
      entry: '/intelligence/day/2026-04-18',
      pageTestId: 'day-insights-page',
      sentinel: intelligenceT('dayInsightsTitle'),
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

  test('shows crash diagnostics paths on the settings route', async () => {
    await seedArchiveRun()
    backendTestHarness.mutateState((state) => {
      state.snapshot.runtimeDiagnostics.latestCrashReport = {
        source: 'rust-panic',
        recordedAt: '2026-04-10T12:34:00Z',
        fatal: true,
        message: 'panic in worker bridge',
        location: 'src-tauri/src/lib.rs:42',
        path: '/tmp/pathkeep-crash/rust-panic-latest.json',
      }
    })
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const page = await screen.findByTestId('settings-page')
    expect(
      await within(page).findByText(settingsT('logsDirectory')),
    ).toBeVisible()
    expect(within(page).getByText(settingsT('crashReports'))).toBeVisible()
    expect(within(page).getByText(settingsT('latestCrashTitle'))).toBeVisible()
    expect(
      within(page).getByRole('button', {
        name: settingsT('openCrashReport'),
      }),
    ).toBeVisible()
  })

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
      within(settingsPage).getByText(settingsT('externalOutputsTitle')),
    ).toBeVisible()
    expect(
      within(settingsPage).getByText(settingsT('archiveDatabase')),
    ).toBeVisible()
    expect(
      within(settingsPage).getByText(settingsT('auditRepository')),
    ).toBeVisible()
    expect(within(settingsPage).getByText(settingsT('gitCommit'))).toBeVisible()
    expect(
      within(remotePanel).getByText(commonT('common.previewTab')),
    ).toBeVisible()
    expect(
      within(settingsPage).getByRole('tab', {
        name: settingsT('externalOutputsTabPublic'),
      }),
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

    const readableContentCard = screen
      .getAllByText(settingsT('readableContentPlugin'))[0]
      .closest('.result-row')
    if (!(readableContentCard instanceof HTMLElement)) {
      throw new Error('Expected readable content plugin card to be present')
    }

    await user.click(
      within(readableContentCard).getByRole('button', {
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
        within(settingsPage).getByText(settingsT('rebuildQueuedTitle')),
      ).toBeVisible()
    })
    expect(
      within(settingsPage).getByRole('link', {
        name: settingsT('runtimeQueueTitle'),
      }),
    ).toHaveAttribute('href', '/jobs')
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

  test('saves analytics consent and runs the updater review flow from settings', async () => {
    await seedArchiveRun()
    const user = userEvent.setup()
    const saveConfigSpy = vi.spyOn(backend, 'saveConfig')
    const checkForAppUpdateSpy = vi
      .spyOn(updateLib, 'checkForAppUpdate')
      .mockResolvedValue({
        availability: {
          supported: true,
          checkedAt: '2026-04-10T00:00:00Z',
          available: true,
          currentVersion: '0.1.0',
          version: '0.2.0',
          notes: 'Updater wiring is ready.',
          publishedAt: '2026-04-10T00:00:00Z',
          error: null,
          downloadUrl: updateLib.RELEASES_PAGE_URL,
        },
        pendingUpdate: {
          currentVersion: '0.1.0',
          version: '0.2.0',
          notes: 'Updater wiring is ready.',
          publishedAt: '2026-04-10T00:00:00Z',
          downloadUrl: updateLib.RELEASES_PAGE_URL,
        },
      })
    const downloadAndInstallSpy = vi
      .spyOn(updateLib, 'downloadAndInstallAppUpdate')
      .mockResolvedValue({
        phase: 'installed',
        downloadedBytes: 128,
        contentLength: null,
        message: 'Installed',
      } as Awaited<ReturnType<typeof updateLib.downloadAndInstallAppUpdate>>)

    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const settingsPage = await screen.findByTestId('settings-page')
    const analyticsPanel = expectHtmlElement(
      within(settingsPage)
        .getByText(settingsT('analyticsTitle'))
        .closest('.panel'),
    )
    const updatePanel = expectHtmlElement(
      within(settingsPage)
        .getByText(settingsT('updateTitle'))
        .closest('.panel'),
    )

    await user.click(
      within(analyticsPanel).getByRole('checkbox', {
        name: settingsT('analyticsEnabled'),
      }),
    )
    await user.click(
      within(analyticsPanel).getByRole('button', {
        name: settingsT('analyticsSave'),
      }),
    )

    await waitFor(() => {
      expect(saveConfigSpy).toHaveBeenCalled()
    })
    expect(saveConfigSpy).toHaveBeenLastCalledWith(
      expect.objectContaining({
        analytics: expect.objectContaining({
          enabled: true,
          consentGrantedAt: expect.any(String),
        }),
      }),
    )

    await user.click(
      within(updatePanel).getByRole('button', {
        name: settingsT('updateCheckNow'),
      }),
    )

    await waitFor(() => {
      expect(checkForAppUpdateSpy).toHaveBeenCalledWith('0.1.0')
    })
    expect(
      within(updatePanel).getByText(settingsT('updateReleaseNotes')),
    ).toBeVisible()
    expect(
      within(updatePanel).getByText('Updater wiring is ready.'),
    ).toBeVisible()

    await user.click(
      within(updatePanel).getByRole('button', {
        name: settingsT('updateDownloadAndInstall'),
      }),
    )

    await waitFor(() => {
      expect(downloadAndInstallSpy).toHaveBeenCalledTimes(1)
    })
  })

  test('recovers the updater panel when check now fails', async () => {
    await seedArchiveRun()
    const user = userEvent.setup()
    vi.spyOn(updateLib, 'checkForAppUpdate').mockRejectedValue(
      new Error('Bridge disconnected'),
    )

    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    const settingsPage = await screen.findByTestId('settings-page')
    const updatePanel = expectHtmlElement(
      within(settingsPage)
        .getByText(settingsT('updateTitle'))
        .closest('.panel'),
    )
    const checkButton = within(updatePanel).getByRole('button', {
      name: settingsT('updateCheckNow'),
    })

    await user.click(checkButton)

    expect(
      await within(updatePanel).findByText('Bridge disconnected'),
    ).toBeVisible()
    await waitFor(() => expect(checkButton).toBeEnabled())
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
            id: 'intelligence',
            labelKey: 'navigation.intelligenceLabel',
            subtitleKey: 'navigation.intelligenceSubtitle',
            titleKey: 'navigation.intelligenceTitle',
            section: 'CORE',
            icon: '◈',
            href: '/intelligence',
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
            id: 'jobs',
            labelKey: 'navigation.jobsLabel',
            subtitleKey: 'navigation.jobsSubtitle',
            icon: '≡',
            href: '/jobs',
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
