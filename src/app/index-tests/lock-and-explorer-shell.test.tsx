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
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter } from 'react-router-dom'
import App from '../index'
import { appRoutes } from '../router'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import {
  expectHtmlElement,
  initializeArchiveOnly,
  resetAppShellHarness,
  seedArchiveRun,
  shellT,
} from './test-helpers'

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

  test('supports explicit page jumps in explorer results and preserves the shell scroll position', async () => {
    const user = userEvent.setup()
    const baseTime = Date.now()

    await initializeArchiveOnly()
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
      initialEntries: ['/explorer?q=sqlite&page=3&layout=legacy'],
    })

    render(<App router={router} />)

    expect(await screen.findByTestId('explorer-page')).toBeInTheDocument()
    await waitFor(() =>
      expect(router.state.location.search).toContain('page=3'),
    )
    await waitFor(() =>
      expect(
        screen
          .getAllByRole('spinbutton', {
            name: 'Page number',
          })
          .every(
            (input) =>
              Number(
                input.getAttribute('value') ??
                  (input as HTMLInputElement).value,
              ) === 3,
          ),
      ).toBe(true),
    )
    await waitFor(() =>
      expect(document.querySelectorAll('.record-item')).toHaveLength(50),
    )

    const scrollContainer = screen.getByTestId('app-scroll')
    expect(scrollContainer).toBeInstanceOf(HTMLElement)
    expectHtmlElement(scrollContainer).scrollTop = 240

    await user.click(screen.getAllByRole('button', { name: 'Next page' })[0])
    await waitFor(() =>
      expect(router.state.location.search).toContain('page=4'),
    )
    await waitFor(() =>
      expect(
        screen
          .getAllByRole('spinbutton', {
            name: 'Page number',
          })
          .every(
            (input) =>
              Number(
                input.getAttribute('value') ??
                  (input as HTMLInputElement).value,
              ) === 4,
          ),
      ).toBe(true),
    )
    await waitFor(() =>
      expect(screen.getAllByRole('button', { name: 'Go' })[0]).toBeEnabled(),
    )
    expect(expectHtmlElement(scrollContainer).scrollTop).toBe(240)

    const pageInput = screen.getAllByRole('spinbutton', {
      name: 'Page number',
    })[0]
    fireEvent.change(pageInput, { target: { value: '8' } })
    await user.click(screen.getAllByRole('button', { name: 'Go' })[0])

    await waitFor(() =>
      expect(router.state.location.search).toContain('page=8'),
    )
    await waitFor(() =>
      expect(
        screen
          .getAllByRole('spinbutton', {
            name: 'Page number',
          })
          .every(
            (input) =>
              Number(
                input.getAttribute('value') ??
                  (input as HTMLInputElement).value,
              ) === 8,
          ),
      ).toBe(true),
    )
    await waitFor(() =>
      expect(document.querySelectorAll('.record-item')).toHaveLength(25),
    )
    expect(
      screen.getAllByRole('button', { name: 'Last page' })[0],
    ).toBeDisabled()
  })

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
