/**
 * @file lock-bootstrap.test.tsx
 * @description Split shell-data suite for remembered-key bootstrap, unlock reload, and lock-refresh fallback behavior.
 * @module app/shell-data-tests
 *
 * ## Responsibilities
 * - Preserve the original remembered-key bootstrap coverage from `src/app/shell-data.test.tsx`.
 * - Assert the lock-state bootstrap, unlock reload, and refresh fallback behavior without changing provider wiring.
 * - Reuse the canonical split-suite harness in `test-helpers.tsx` so this file only owns lock bootstrap expectations.
 *
 * ## Not responsible for
 * - Re-defining the shared shell-data probe, i18n wrapper, or backend harness reset flow.
 * - Covering unrelated shell-data mutations such as backup progress, config saves, or idle-timeout behavior.
 * - Changing `ShellDataProvider` behavior or introducing new test-only abstractions.
 *
 * ## Dependencies
 * - Depends on the real `backend` client spies plus the split-suite helpers from `./test-helpers`.
 * - Uses Testing Library and `userEvent` to preserve the legacy provider interaction contract.
 *
 * ## Performance notes
 * - Reuses the shared seeded archive/bootstrap helpers to avoid duplicating expensive setup across split suites.
 */

import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import type { AppSnapshot } from '../../lib/types'
import {
  getDefaultBuildInfo,
  renderShellProbe,
  resetShellDataHarness,
  seedSnapshot,
} from './test-helpers'

describe('ShellDataProvider', () => {
  beforeEach(resetShellDataHarness)

  test('auto-unlocks a remembered archive key once and reuses the session key afterwards', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const rememberedSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        rememberDatabaseKeyInKeyring: true,
      },
      archiveStatus: {
        ...snapshot.archiveStatus,
        encrypted: true,
        unlocked: false,
      },
      keyringStatus: {
        ...snapshot.keyringStatus,
        available: true,
        storedSecret: true,
      },
    }
    const unlockedSnapshot: AppSnapshot = {
      ...snapshot,
      config: rememberedSnapshot.config,
      keyringStatus: rememberedSnapshot.keyringStatus,
    }

    const keyringSpy = vi
      .spyOn(backend, 'keyringGetDatabaseKey')
      .mockResolvedValue('vault-passphrase')
    const sessionSpy = vi
      .spyOn(backend, 'setSessionDatabaseKey')
      .mockResolvedValue(undefined)
    const getAppSnapshotSpy = vi
      .spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(rememberedSnapshot)
      .mockResolvedValue(unlockedSnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(
      snapshot.appLockStatus,
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(keyringSpy).toHaveBeenCalledTimes(1)
    expect(sessionSpy).toHaveBeenCalledWith('vault-passphrase')
    const snapshotCallsAfterBoot = getAppSnapshotSpy.mock.calls.length
    expect(snapshotCallsAfterBoot).toBeGreaterThanOrEqual(2)

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(getAppSnapshotSpy.mock.calls.length).toBeGreaterThan(
        snapshotCallsAfterBoot,
      ),
    )
    expect(keyringSpy).toHaveBeenCalledTimes(1)
  })

  test('keeps the locked snapshot when the keyring returns no database key', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const rememberedSnapshot: AppSnapshot = {
      ...snapshot,
      config: {
        ...snapshot.config,
        rememberDatabaseKeyInKeyring: true,
      },
      archiveStatus: {
        ...snapshot.archiveStatus,
        encrypted: true,
        unlocked: false,
      },
      keyringStatus: {
        ...snapshot.keyringStatus,
        available: true,
        storedSecret: true,
      },
    }

    const keyringSpy = vi
      .spyOn(backend, 'keyringGetDatabaseKey')
      .mockResolvedValue(null)
    const sessionSpy = vi
      .spyOn(backend, 'setSessionDatabaseKey')
      .mockResolvedValue(undefined)
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(rememberedSnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAppLockStatus').mockResolvedValue(
      snapshot.appLockStatus,
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(keyringSpy).toHaveBeenCalledTimes(1)
    expect(sessionSpy).not.toHaveBeenCalled()
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false')

    const refreshKeyBefore = screen.getByTestId('refresh-key').textContent
    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('refresh-key')).not.toHaveTextContent(
        refreshKeyBefore ?? '',
      ),
    )
    expect(keyringSpy).toHaveBeenCalledTimes(1)
    expect(sessionSpy).not.toHaveBeenCalled()
  })

  test('boots into the lock state and reloads shell data after unlock', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const lockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: true,
      passcodeConfigured: true,
      lockReason: 'startup',
    }
    const unlockedStatus = {
      ...lockedStatus,
      locked: false,
      lockReason: null,
      lastUnlockedAt: '2026-04-08T01:00:00Z',
    }

    const getAppSnapshotSpy = vi
      .spyOn(backend, 'getAppSnapshot')
      .mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAppLockStatus')
      .mockResolvedValueOnce(lockedStatus)
      .mockResolvedValue(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'unlockAppSession').mockResolvedValue(unlockedStatus)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('app-lock-enabled')).toHaveTextContent('true')
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true')
    expect(screen.getByTestId('snapshot-language')).toHaveTextContent('none')
    expect(screen.getByTestId('refresh-key')).toHaveTextContent(/^1$/)
    expect(getAppSnapshotSpy).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: 'unlock' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false'),
    )
    await waitFor(() =>
      expect(screen.getByTestId('snapshot-language')).not.toHaveTextContent(
        'none',
      ),
    )
    expect(getAppSnapshotSpy).toHaveBeenCalledTimes(1)
  })

  test('keeps the original error when a lock refresh still reports an unlocked session', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
    }

    vi.spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce(
        new Error(
          'PathKeep is currently locked. Unlock the app before requesting archive data.',
        ),
      )
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAppLockStatus')
      .mockResolvedValueOnce(unlockedStatus)
      .mockResolvedValueOnce(unlockedStatus)
      .mockResolvedValueOnce(unlockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('currently locked'),
    )
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false')
  })

  test('falls back to locked app state when archive refresh reports a lock error', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    const unlockedStatus = {
      ...snapshot.appLockStatus,
      enabled: true,
      locked: false,
      passcodeConfigured: true,
    }
    const lockedStatus = {
      ...unlockedStatus,
      locked: true,
      lockReason: 'manual',
    }

    vi.spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce(
        new Error(
          'PathKeep is currently locked. Unlock the app before requesting archive data.',
        ),
      )
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadAppLockStatus')
      .mockResolvedValueOnce(unlockedStatus)
      .mockResolvedValueOnce(unlockedStatus)
      .mockResolvedValueOnce(lockedStatus)
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('false')

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('app-lock-locked')).toHaveTextContent('true'),
    )
    expect(screen.getByTestId('snapshot-language')).toHaveTextContent('none')
    expect(screen.getByTestId('error')).toHaveTextContent('none')
    expect(screen.getByTestId('refresh-key')).toHaveTextContent(/^2$/)
  })
})
