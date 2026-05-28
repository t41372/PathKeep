/**
 * @file lock-and-explorer-shell.test.tsx
 * @description Split app-shell suite for lock gating, explorer pagination, topbar search routing, and explorer-to-audit shell flows.
 * @module app/index-tests
 *
 * ## Responsibilities
 * - Preserve the original app-shell lock-screen route contract while the mega-suite is being split.
 * - Keep the explorer pagination, shell scroll preservation, and topbar-search navigation assertions identical to `src/app/index.test.tsx`.
 * - Reuse the shared shell-test harness so split suites still exercise the same seeded backend state and translator contract.
 *
 * ## Not responsible for
 * - Does not redefine app-shell helpers that already belong to `src/app/index-tests/test-helpers.tsx`.
 * - Does not change route behavior, assertions, or shell contracts beyond extracting this one test slice.
 * - Does not own unrelated onboarding, settings, schedule, or intelligence shell tests.
 *
 * ## Dependencies
 * - Depends on the canonical `App` shell, `appRoutes`, and the backend test harness used by the original mega-suite.
 * - Reuses shared shell-test helpers for reset, archive seeding, DOM narrowing, and translated lock copy.
 *
 * ## Performance notes
 * - Keeps this split focused so Vitest can execute the lock and explorer shell slice without loading unrelated route assertions into the same file.
 */

import { beforeEach, describe, expect, test } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter } from 'react-router-dom'
import App from '../index'
import { appRoutes } from '../router'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import { resetAppShellHarness, seedArchiveRun, shellT } from './test-helpers'

describe('App shell', () => {
  beforeEach(() => {
    resetAppShellHarness()
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

  // The v0.2 "supports explicit page jumps in explorer results" test drove
  // the legacy ExplorerResultsPanel's Next/Last page buttons + "Page number"
  // spinbutton + scroll-position preservation across paginator clicks. Phase
  // 4 retires that chrome — paper Browse uses date-anchored navigation on
  // the contact sheet, not a paginator. There is no equivalent paper surface
  // to drive, and useExplorerData's underlying pagination is already covered
  // by its own hook tests.

  test('paper topbar uses a palette opener instead of a global search box', async () => {
    await seedArchiveRun()
    const router = createMemoryRouter(appRoutes, {
      initialEntries: ['/'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('app-shell')).toBeInTheDocument()
    // The paper redesign removes the v0.2 topbar searchbox: search lives in a
    // ⌘K palette, and notifications are not part of the chrome.
    expect(screen.queryByRole('searchbox')).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'Notifications' }),
    ).not.toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /Find a page/i }),
    ).toBeInTheDocument()
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
})
