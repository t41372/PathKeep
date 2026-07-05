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

import { act, fireEvent, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import type { TakeoutInspection } from '../../lib/types'
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
    expect(screen.getByTestId('dashboard-loading')).toHaveTextContent('false')

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
    const addWindowListenerSpy = vi.spyOn(window, 'addEventListener')
    const removeWindowListenerSpy = vi.spyOn(window, 'removeEventListener')
    const addDocumentListenerSpy = vi.spyOn(document, 'addEventListener')
    const removeDocumentListenerSpy = vi.spyOn(document, 'removeEventListener')

    const { unmount } = renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('true')
    for (const eventName of ['pointerdown', 'keydown', 'mousemove', 'focus']) {
      await waitFor(() =>
        expect(addWindowListenerSpy).toHaveBeenCalledWith(
          eventName,
          expect.any(Function),
          { passive: true },
        ),
      )
    }
    await waitFor(() =>
      expect(addDocumentListenerSpy).toHaveBeenCalledWith(
        'visibilitychange',
        expect.any(Function),
      ),
    )

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

      expect(lockSpy).toHaveBeenCalledWith('idle-timeout')
      await waitFor(() =>
        expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true'),
      )
      expect(screen.getByTestId('snapshot-language')).toHaveTextContent('none')
      expect(screen.getByTestId('refresh-key')).toHaveTextContent(/^2$/)
      unmount()
      for (const eventName of [
        'pointerdown',
        'keydown',
        'mousemove',
        'focus',
      ]) {
        expect(removeWindowListenerSpy).toHaveBeenCalledWith(
          eventName,
          expect.any(Function),
        )
      }
      expect(removeDocumentListenerSpy).toHaveBeenCalledWith(
        'visibilitychange',
        expect.any(Function),
      )
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

  test('does not arm idle locking while a shell busy action is active', async () => {
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
      idleTimeoutMinutes: 1,
    }
    let resolveBackup: (() => void) | undefined
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
    const lockSpy = vi.spyOn(backend, 'lockAppSession').mockResolvedValue({
      ...unlockedStatus,
      locked: true,
      lockReason: 'idle-timeout',
    })
    const clearTimeoutSpy = vi.spyOn(window, 'clearTimeout')
    vi.spyOn(backend, 'runBackupNow').mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveBackup = () =>
            resolve({
              dueSkipped: false,
              run: null,
              profiles: [],
              warnings: [],
            })
        }),
    )

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    try {
      vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
      fireEvent.click(screen.getByRole('button', { name: 'backup' }))
      expect(screen.getByTestId('busy-label')).not.toHaveTextContent('none')
      expect(clearTimeoutSpy).toHaveBeenCalled()
      act(() => {
        vi.advanceTimersByTime(60_000)
      })
      await Promise.resolve()
      expect(lockSpy).not.toHaveBeenCalled()

      await act(async () => {
        resolveBackup?.()
        await Promise.resolve()
      })
    } finally {
      if (vi.isFakeTimers()) {
        vi.useRealTimers()
      }
    }
  })

  test('does not auto-lock while an archive import is in flight, then re-arms after it settles', async () => {
    // Data-safety: `runImport` never sets busyAction, so the idle timer must be
    // suppressed by the running archive task instead — otherwise a long import
    // could hit the idle timeout and lock the archive out from under an in-flight
    // encrypted write. This is a DEFER: once the task settles the timer re-arms.
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
      idleTimeoutMinutes: 1,
    }
    let resolveImport: ((value: TakeoutInspection) => void) | null = null

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
    const lockSpy = vi.spyOn(backend, 'lockAppSession').mockResolvedValue({
      ...unlockedStatus,
      locked: true,
      lockReason: 'idle-timeout',
    })
    vi.spyOn(backend, 'importTakeout').mockImplementation(
      () =>
        new Promise<TakeoutInspection>((resolve) => {
          resolveImport = resolve
        }),
    )

    renderShellProbe()
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    // Start the import under real timers so `waitForNextPaint` (real rAF) can
    // resolve and the pending backend call is actually reached.
    fireEvent.click(screen.getByRole('button', { name: 'import-takeout' }))
    await waitFor(() => expect(backend.importTakeout).toHaveBeenCalled())
    expect(screen.getByTestId('active-archive-task')).not.toHaveTextContent(
      'none',
    )

    try {
      vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })

      // A running archive task suppresses the idle timer: the idle listeners are
      // detached while the task is in flight, so even a user-activity event must
      // NOT (re)arm the deadline, and advancing past the timeout must not lock
      // the session mid-write. On the old busyAction-only guard the listeners
      // stayed attached during an import, so this pointerdown would arm a timer
      // that then fires — that is what makes this assertion actually bite.
      act(() => {
        window.dispatchEvent(new Event('pointerdown'))
        vi.advanceTimersByTime(60_000)
      })
      await Promise.resolve()
      expect(lockSpy).not.toHaveBeenCalled()

      // The import settles → the task leaves the running set → the idle timer
      // re-arms and CAN fire if the user stays idle.
      await act(async () => {
        resolveImport?.(takeoutInspection())
        await Promise.resolve()
        await Promise.resolve()
      })
      await act(async () => {
        vi.advanceTimersByTime(60_000)
        await Promise.resolve()
        await Promise.resolve()
      })
      vi.runOnlyPendingTimers()
      vi.useRealTimers()

      expect(lockSpy).toHaveBeenCalledWith('idle-timeout')
    } finally {
      if (vi.isFakeTimers()) {
        vi.runOnlyPendingTimers()
        vi.useRealTimers()
      }
    }
  })

  test('arms idle locking only when the document becomes visible', async () => {
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
    const lockSpy = vi.spyOn(backend, 'lockAppSession').mockResolvedValue({
      ...unlockedStatus,
      locked: true,
      lockReason: 'idle-timeout',
    })
    const visibilityDescriptor = Object.getOwnPropertyDescriptor(
      document,
      'visibilityState',
    )

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    const setTimeoutSpy = vi.spyOn(window, 'setTimeout')
    setTimeoutSpy.mockClear()

    try {
      act(() => {
        Object.defineProperty(document, 'visibilityState', {
          configurable: true,
          value: 'hidden',
        })
        document.dispatchEvent(new Event('visibilitychange'))
      })
      expect(setTimeoutSpy).not.toHaveBeenCalled()
      expect(lockSpy).not.toHaveBeenCalled()

      act(() => {
        Object.defineProperty(document, 'visibilityState', {
          configurable: true,
          value: 'visible',
        })
        document.dispatchEvent(new Event('visibilitychange'))
      })
      await waitFor(() =>
        expect(setTimeoutSpy).toHaveBeenCalledWith(
          expect.any(Function),
          60_000,
        ),
      )
      expect(lockSpy).not.toHaveBeenCalled()
    } finally {
      setTimeoutSpy.mockRestore()
      if (visibilityDescriptor) {
        Object.defineProperty(document, 'visibilityState', visibilityDescriptor)
      } else {
        delete (document as { visibilityState?: string }).visibilityState
      }
    }
  })

  test('surfaces idle-timeout lock failures without clearing the loaded shell state', async () => {
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
        expect(screen.getByTestId('error')).toHaveTextContent('not-an-error'),
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
      expect(screen.getByTestId('error')).toHaveTextContent('not-an-error'),
    )

    await user.click(screen.getByRole('button', { name: 'clear-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'clear passcode failed',
      ),
    )
    await user.click(screen.getByRole('button', { name: 'clear-passcode' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('not-an-error'),
    )

    await user.click(screen.getByRole('button', { name: 'lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('lock failed'),
    )
    await user.click(screen.getByRole('button', { name: 'lock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('not-an-error'),
    )

    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('unlock failed'),
    )
    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('not-an-error'),
    )
  })
})

function takeoutInspection(): TakeoutInspection {
  return {
    sourcePath: '/tmp/Takeout',
    dryRun: false,
    recognizedFiles: [],
    quarantinedFiles: [],
    candidateItems: 6,
    importedItems: 4,
    duplicateItems: 2,
    previewEntries: [],
    importBatch: {
      id: 9,
      sourceKind: 'google-takeout',
      sourcePath: '/tmp/Takeout',
      profileId: 'takeout::browser-history',
      createdAt: '2026-04-27T10:00:00.000Z',
      importedAt: '2026-04-27T10:02:00.000Z',
      revertedAt: null,
      status: 'imported',
      candidateItems: 6,
      importedItems: 4,
      duplicateItems: 2,
      visibleItems: 4,
      auditPath: null,
      gitCommit: null,
    },
    notes: [],
    detectedLocale: null,
    previewRangeStart: null,
    previewRangeEnd: null,
  }
}
