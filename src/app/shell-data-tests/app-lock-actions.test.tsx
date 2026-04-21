/**
 * @file app-lock-actions.test.tsx
 * @description App-lock action coverage split from the legacy `src/app/shell-data.test.tsx` mega-suite.
 * @module app/shell-data-tests/app-lock-actions
 *
 * ## Responsibilities
 * - Preserve the original shell-provider coverage for app-lock success, idle auto-lock, and action failure flows.
 * - Reuse the shared shell-data harness so this split keeps the same probe, i18n wrapper, and backend bootstrap contract.
 * - Keep the extracted cases isolated without changing their existing titles, mocked behavior, or assertions.
 *
 * ## Not responsible for
 * - Rewriting the remaining `shell-data` mega-suite or changing neighboring split ownership.
 * - Introducing new helper abstractions that would alter how the provider is exercised.
 * - Broadening app-lock coverage beyond the four cases assigned to this slice.
 *
 * ## Dependencies
 * - Depends on the real `ShellDataProvider` contract through the shared test harness in `test-helpers.tsx`.
 * - Uses Testing Library, Vitest mocks, and the backend client spies already established by the legacy suite.
 *
 * ## Performance notes
 * - Keeps setup centralized in the shared helper so the split suite stays lightweight and avoids duplicate provider bootstrap work.
 */

import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createTranslator } from '../../lib/i18n'
import {
  getDefaultBuildInfo,
  renderShellProbe,
  resetShellDataHarness,
  seedSnapshot,
} from './test-helpers'

describe('ShellDataProvider', () => {
  beforeEach(() => {
    resetShellDataHarness()
  })

  test('runs app lock success actions through the shell provider', async () => {
    const user = userEvent.setup()

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'set-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'enable-lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('true'),
    )
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false')

    await user.click(screen.getByRole('button', { name: 'lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true'),
    )
    expect(screen.getByTestId('snapshot-language')).toHaveTextContent('none')

    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false'),
    )
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
        'none',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'lock-default' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true'),
    )
    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'clear-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('false'),
    )
  })

  test('auto-locks after idle timeout when app lock is enabled', async () => {
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
      idleTimeoutMinutes: 1,
    }
    const lockedStatus = {
      ...unlockedStatus,
      locked: true,
      lockReason: 'idle-timeout',
    }

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        appLock: {
          ...snapshot.config.appLock,
          enabled: true,
          idleTimeoutMinutes: 1,
          passcodeConfigured: true,
        },
      },
      appLockStatus: unlockedStatus,
    })
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    const lockSpy = vi
      .spyOn(backend, 'lockAppSession')
      .mockResolvedValue(lockedStatus)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('true')

    const visibilityDescriptor = Object.getOwnPropertyDescriptor(
      document,
      'visibilityState',
    )
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })

    try {
      await act(async () => {
        Object.defineProperty(document, 'visibilityState', {
          configurable: true,
          value: 'hidden',
        })
        document.dispatchEvent(new Event('visibilitychange'))
        await Promise.resolve()
        Object.defineProperty(document, 'visibilityState', {
          configurable: true,
          value: 'visible',
        })
        window.dispatchEvent(new Event('pointerdown'))
        document.dispatchEvent(new Event('visibilitychange'))
        await Promise.resolve()
      })

      await act(async () => {
        vi.advanceTimersByTime(60_000)
        await Promise.resolve()
        await Promise.resolve()
      })

      vi.runOnlyPendingTimers()
      vi.useRealTimers()

      await waitFor(() => expect(lockSpy).toHaveBeenCalledWith('idle-timeout'))
      await waitFor(() =>
        expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true'),
      )
      expect(screen.getByTestId('snapshot-language')).toHaveTextContent('none')
    } finally {
      if (vi.isFakeTimers()) {
        vi.runOnlyPendingTimers()
      }
      if (visibilityDescriptor) {
        Object.defineProperty(document, 'visibilityState', visibilityDescriptor)
      } else {
        delete (document as { visibilityState?: string }).visibilityState
      }
      vi.useRealTimers()
    }
  })

  test('surfaces idle-timeout lock failures without clearing the loaded shell state', async () => {
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
      idleTimeoutMinutes: 1,
    }

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        appLock: {
          ...snapshot.config.appLock,
          enabled: true,
          idleTimeoutMinutes: 1,
          passcodeConfigured: true,
        },
      },
      appLockStatus: unlockedStatus,
    })
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    const lockSpy = vi
      .spyOn(backend, 'lockAppSession')
      .mockRejectedValueOnce(new Error('idle lock failed'))
      .mockRejectedValueOnce('not-an-error')

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })

    try {
      await act(async () => {
        window.dispatchEvent(new Event('pointerdown'))
        await Promise.resolve()
      })
      await act(async () => {
        vi.advanceTimersByTime(60_000)
        await Promise.resolve()
        await Promise.resolve()
      })
      vi.runOnlyPendingTimers()
      vi.useRealTimers()

      await waitFor(() =>
        expect(screen.getByTestId('error')).toHaveTextContent(
          'idle lock failed',
        ),
      )
      expect(lockSpy).toHaveBeenCalledWith('idle-timeout')
      expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
        'none',
      )

      vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
      await act(async () => {
        window.dispatchEvent(new Event('pointerdown'))
        await Promise.resolve()
      })
      await act(async () => {
        vi.advanceTimersByTime(60_000)
        await Promise.resolve()
        await Promise.resolve()
      })
      vi.runOnlyPendingTimers()
      vi.useRealTimers()

      await waitFor(() =>
        expect(screen.getByTestId('error')).toHaveTextContent(
          translator('shell.lockAppFailed'),
        ),
      )
      expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
        'none',
      )
    } finally {
      if (vi.isFakeTimers()) {
        vi.runOnlyPendingTimers()
      }
      vi.useRealTimers()
    }
  })

  test('surfaces app lock action failures with both explicit and fallback errors', async () => {
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
      idleTimeoutMinutes: 1,
    }

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue({
      ...snapshot,
      config: {
        ...snapshot.config,
        appLock: {
          ...snapshot.config.appLock,
          enabled: true,
          idleTimeoutMinutes: 1,
          passcodeConfigured: true,
        },
      },
      appLockStatus: unlockedStatus,
    })
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'setAppLockPasscode')
      .mockRejectedValueOnce(new Error('set passcode failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'clearAppLockPasscode')
      .mockRejectedValueOnce(new Error('clear passcode failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'lockAppSession')
      .mockRejectedValueOnce(new Error('lock failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'unlockAppSession')
      .mockRejectedValueOnce(new Error('unlock failed'))
      .mockRejectedValueOnce('not-an-error')

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'set-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'set passcode failed',
      ),
    )
    await user.click(screen.getByRole('button', { name: 'set-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.setAppLockPasscodeFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'clear-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'clear passcode failed',
      ),
    )
    await user.click(screen.getByRole('button', { name: 'clear-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.clearAppLockPasscodeFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('lock failed'),
    )
    await user.click(screen.getByRole('button', { name: 'lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.lockAppFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('unlock failed'),
    )
    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.unlockAppFailed'),
      ),
    )
  })
})
