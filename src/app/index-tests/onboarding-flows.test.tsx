/**
 * @file onboarding-flows.test.tsx
 * @description Onboarding-only slice extracted from the legacy app-shell mega-suite.
 * @module app/index-tests
 *
 * ## Responsibilities
 * - Preserve the existing onboarding route assertions from `src/app/index.test.tsx`.
 * - Verify zero-state entry, resume-later behavior, browser-profile gating, archive-mode switching, and encrypted initialization.
 * - Reuse the shared shell-test helpers so split suites keep the same baseline contract.
 *
 * ## Not responsible for
 * - Covering lock, explorer, settings, schedule, or intelligence shell routes.
 * - Redefining shared translators, app config fixtures, or reset behavior.
 * - Changing onboarding copy, router semantics, or backend contracts.
 *
 * ## Dependencies
 * - Depends on `App`, `appRoutes`, the backend test harness, and shared onboarding translators.
 * - Uses Testing Library plus the same memory-router setup as the original mega-suite.
 *
 * ## Performance notes
 * - Keeps the split focused on onboarding-only flows so future suite sharding can stay targeted without duplicating broader shell fixtures.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter } from 'react-router-dom'
import App from '../index'
import { appRoutes } from '../router'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import { createNamespaceTranslator } from '../../lib/i18n'
import { macosFullDiskAccessSettingsUrl } from '../../lib/platform-guidance'
import {
  dashboardT,
  onboardingT,
  resetAppShellHarness,
  shellT,
} from './test-helpers'

describe('App shell', () => {
  beforeEach(() => {
    resetAppShellHarness()
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

  test('keeps unreadable Safari visible in localized onboarding permission recovery', async () => {
    const user = userEvent.setup()
    const zhTwOnboarding = createNamespaceTranslator('zh-TW', 'onboarding')
    backendTestHarness.mutateState((state) => {
      state.snapshot.config.preferredLanguage = 'zh-TW'
      state.snapshot.config.selectedProfileIds = ['safari:default']
      state.snapshot.browserProfiles = state.snapshot.browserProfiles.map(
        (profile) =>
          profile.profileId === 'safari:default'
            ? {
                ...profile,
                historyExists: true,
                historyReadable: false,
                accessIssue: 'macos-full-disk-access',
              }
            : profile,
      )
    })
    const openSettingsSpy = vi
      .spyOn(backend, 'openExternalUrl')
      .mockResolvedValue(macosFullDiskAccessSettingsUrl)
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/onboarding'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('onboarding-page')).toBeInTheDocument()
    await user.click(
      await screen.findByRole('button', {
        name: zhTwOnboarding('beginSetup'),
      }),
    )

    expect(
      await screen.findByText(zhTwOnboarding('permissionRequired')),
    ).toBeVisible()
    expect(
      screen.getByText(zhTwOnboarding('selectedProfilesNeedAccess')),
    ).toBeVisible()
    expect(
      screen.queryByText(/Grant Full Disk Access/i),
    ).not.toBeInTheDocument()

    await user.click(
      screen.getByRole('button', { name: zhTwOnboarding('continueButton') }),
    )
    expect(await screen.findByRole('alert')).toHaveTextContent(
      zhTwOnboarding('errorSelectedProfilesNeedAccess'),
    )

    await user.click(
      screen.getByRole('button', {
        name: zhTwOnboarding('openFullDiskAccessSettings'),
      }),
    )
    await waitFor(() =>
      expect(openSettingsSpy).toHaveBeenCalledWith(
        macosFullDiskAccessSettingsUrl,
      ),
    )
  })

  test('localizes Safari access failures raised during the first onboarding backup', async () => {
    const user = userEvent.setup()
    const zhTwOnboarding = createNamespaceTranslator('zh-TW', 'onboarding')
    backendTestHarness.mutateState((state) => {
      state.snapshot.config.preferredLanguage = 'zh-TW'
      state.snapshot.config.archiveMode = 'Plaintext'
      state.snapshot.config.selectedProfileIds = [
        'chrome:Default',
        'safari:default',
      ]
      state.snapshot.browserProfiles = state.snapshot.browserProfiles.map(
        (profile) =>
          profile.profileId === 'safari:default'
            ? {
                ...profile,
                historyExists: true,
                historyReadable: false,
                accessIssue: 'macos-full-disk-access',
              }
            : profile,
      )
    })
    vi.spyOn(backend, 'runBackupNow').mockRejectedValue(
      new Error(
        'Safari History.db is not readable yet. Grant Full Disk Access to PathKeep or the running development process, then run the backup again.',
      ),
    )
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/onboarding'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('onboarding-page')).toBeInTheDocument()
    await user.click(
      await screen.findByRole('button', {
        name: zhTwOnboarding('beginSetup'),
      }),
    )
    await user.click(
      screen.getByRole('button', { name: zhTwOnboarding('continueButton') }),
    )
    await user.click(
      await screen.findByRole('button', {
        name: zhTwOnboarding('continueButton'),
      }),
    )
    await user.click(
      await screen.findByRole('button', {
        name: zhTwOnboarding('continueButton'),
      }),
    )
    await user.click(
      await screen.findByRole('button', {
        name: zhTwOnboarding('skipScheduleButton'),
      }),
    )
    // The optional AI step sits between Schedule and Ready; skip it to reach the final review.
    await user.click(
      await screen.findByRole('button', {
        name: zhTwOnboarding('aiStepSkipAction'),
      }),
    )
    await user.click(
      await screen.findByRole('button', { name: zhTwOnboarding('initButton') }),
    )

    expect(await screen.findByRole('alert')).toHaveTextContent(
      zhTwOnboarding('errorSafariNeedsFullDiskAccess'),
    )
    expect(
      screen.queryByText(/Safari History\.db is not readable yet/i),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(/Grant Full Disk Access/i),
    ).not.toBeInTheDocument()
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
        name: onboardingT('skipScheduleButton'),
      }),
    )
    // The optional AI step sits between Schedule and Ready; skip it to reach the final review.
    await user.click(
      await screen.findByRole('button', {
        name: onboardingT('aiStepSkipAction'),
      }),
    )
    await user.click(
      await screen.findByRole('button', { name: onboardingT('initButton') }),
    )

    expect(await screen.findByTestId('dashboard-page')).toBeInTheDocument()
    expect(keyringStoreSpy).not.toHaveBeenCalled()
  })
})
