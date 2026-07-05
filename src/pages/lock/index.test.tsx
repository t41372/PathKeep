/**
 * @file index.test.tsx
 * @description Route-level coverage for the standalone App Lock screen.
 * @module pages/lock
 *
 * ## Responsibilities
 * - Verify loading, passcode unlock, biometric fallback, copy/open support actions, and lock reason copy.
 * - Keep the lock route wired to shell data and router navigation.
 *
 * ## Not responsible for
 * - Re-testing App Lock backend policy.
 * - Re-testing shared review path rows.
 *
 * ## Dependencies
 * - Wraps LockPage in shell, router, and i18n providers.
 * - Mocks only desktop file-manager opening.
 *
 * ## Performance notes
 * - Uses a tiny shell-value fixture so tests stay route-focused.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { I18nProvider } from '../../lib/i18n'
import type { AppLockStatus, UnlockAppSessionRequest } from '../../lib/types'
import { LockPage } from './index'

describe('LockPage', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('renders a loading shell until App Lock status is available', () => {
    renderLock({ appLockStatus: null })

    expect(screen.getByText('Loading')).toBeVisible()
  })

  test('unlocks with passcode, copies config path, and opens recovery path', async () => {
    const user = userEvent.setup()
    const unlockAppSession = vi.fn().mockResolvedValue(lockStatusFixture())
    const openPath = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/tmp/pathkeep/config.json')
    const originalClipboard = navigator.clipboard
    const writeText = vi.fn(() => Promise.resolve(undefined))
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    })

    try {
      renderLock({
        appLockStatus: lockStatusFixture({
          lockReason: 'idle-timeout',
          warnings: ['Lock warning'],
        }),
        unlockAppSession,
      })

      expect(screen.getByText('Idle timeout')).toBeVisible()
      expect(screen.getByText('Lock warning')).toBeVisible()
      await user.click(screen.getByRole('button', { name: 'Copy' }))
      expect(writeText).toHaveBeenCalledWith('/tmp/pathkeep/config.json')
      await user.click(screen.getByRole('button', { name: 'Open config path' }))
      expect(openPath).toHaveBeenCalledWith('/tmp/pathkeep/config.json')

      await user.type(screen.getByLabelText('Passcode'), '123456')
      await user.click(screen.getByRole('button', { name: 'Unlock' }))

      expect(unlockAppSession).toHaveBeenCalledWith({
        passcode: '123456',
        useBiometric: false,
      })
      expect(await screen.findByTestId('next-route')).toHaveTextContent(
        'settings',
      )
    } finally {
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: originalClipboard,
      })
    }
  })

  test('uses biometric unlock when available and shows fallback copy when unavailable', async () => {
    const user = userEvent.setup()
    const unlockAppSession = vi.fn().mockResolvedValue(lockStatusFixture())
    renderLock({
      appLockStatus: lockStatusFixture({
        biometricAvailable: true,
        biometricEnabled: true,
        biometricState: 'unsupported',
        lockReason: 'startup',
      }),
      unlockAppSession,
    })

    expect(screen.getByText('Startup check')).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'Use biometric' }))
    expect(unlockAppSession).toHaveBeenCalledWith({
      passcode: null,
      useBiometric: true,
    })
    expect(await screen.findByTestId('next-route')).toHaveTextContent(
      'settings',
    )

    renderLock({
      appLockStatus: lockStatusFixture({
        biometricAvailable: false,
        biometricEnabled: true,
        biometricState: 'touch-id-unavailable',
        lockReason: 'manual',
      }),
      unlockAppSession,
    })

    expect(screen.getByText('Manual lock')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Use Touch ID' })).toBeDisabled()
    expect(
      screen.getByText(
        'Touch ID is unavailable on this Mac right now, so PathKeep is using the passcode fallback.',
      ),
    ).toBeVisible()
  })

  test('renders shell error, generic biometric fallback, and empty optional notes', () => {
    renderLock({
      appLockStatus: lockStatusFixture({
        biometricAvailable: false,
        biometricEnabled: true,
        biometricState: 'unsupported',
        degradationNotes: [],
        recoveryHint: null,
      }),
      error: 'unlock failed',
    })

    expect(
      screen.getByText('PathKeep could not unlock the current app session.'),
    ).toBeVisible()
    expect(screen.getByText('unlock failed')).toBeVisible()
    expect(
      screen.getByText(
        'Biometric unlock is not available in this desktop build yet, so PathKeep is using the passcode fallback.',
      ),
    ).toBeVisible()
    expect(
      screen.getByText(
        'PathKeep does not offer a fake recovery flow here. Open the config path and follow the support guidance if you need to reset the UI session lock.',
      ),
    ).toBeVisible()
    expect(
      screen.queryByText('Archive encryption still protects data at rest.'),
    ).not.toBeInTheDocument()
  })
})

function renderLock({
  appLockStatus,
  error = null,
  unlockAppSession = vi.fn(),
}: {
  appLockStatus: AppLockStatus | null
  error?: string | null
  unlockAppSession?: (
    request: UnlockAppSessionRequest,
  ) => Promise<AppLockStatus>
}) {
  return render(lockNode({ appLockStatus, error, unlockAppSession }))
}

function lockNode({
  appLockStatus,
  error = null,
  unlockAppSession = vi.fn(),
}: {
  appLockStatus: AppLockStatus | null
  error?: string | null
  unlockAppSession?: (
    request: UnlockAppSessionRequest,
  ) => Promise<AppLockStatus>
}) {
  return (
    <MemoryRouter initialEntries={['/lock?next=/settings']}>
      <I18nProvider>
        <ShellDataContext.Provider
          value={shellValue({ appLockStatus, error, unlockAppSession })}
        >
          <Routes>
            <Route path="/lock" element={<LockPage />} />
            <Route
              path="/settings"
              element={<div data-testid="next-route">settings</div>}
            />
          </Routes>
        </ShellDataContext.Provider>
      </I18nProvider>
    </MemoryRouter>
  )
}

function shellValue({
  appLockStatus,
  error,
  unlockAppSession,
}: {
  appLockStatus: AppLockStatus | null
  error: string | null
  unlockAppSession: (request: UnlockAppSessionRequest) => Promise<AppLockStatus>
}): ShellDataContextValue {
  return {
    appLockStatus,
    buildInfo: null,
    busyAction: null,
    busyOverlay: null,
    clearAppLockPasscode: vi.fn(),
    clearNotice: vi.fn(),
    errorKind: null,
    clearError: vi.fn(),
    dashboard: null,
    error,
    initializeArchive: vi.fn(),
    loading: false,
    lockAppSession: vi.fn(),
    notice: null,
    refreshAppData: vi.fn(),
    refreshKey: 0,
    refreshRuntimeStatus: vi.fn(),
    runBackup: vi.fn(),
    saveConfig: vi.fn(),
    setAppLockPasscode: vi.fn(),
    snapshot: null,
    unlockAppSession,
  } as unknown as ShellDataContextValue
}

function lockStatusFixture(
  overrides: Partial<AppLockStatus> = {},
): AppLockStatus {
  return {
    biometricAvailable: false,
    biometricEnabled: false,
    biometricState: 'unsupported',
    configPath: '/tmp/pathkeep/config.json',
    degradationNotes: ['Archive encryption still protects data at rest.'],
    enabled: true,
    idleTimeoutMinutes: 5,
    lastUnlockedAt: null,
    locked: true,
    lockedAt: '2026-04-25T12:00:00Z',
    lockReason: null,
    passcodeConfigured: true,
    passcodeEnabled: true,
    recoveryHint: 'six digits',
    warnings: [],
    ...overrides,
  }
}
