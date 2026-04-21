/**
 * @file provider-boundaries.test.tsx
 * @description Preserves the legacy shell-data provider boundary regression case while the mega-suite is split into focused files.
 * @module app/shell-data-tests
 *
 * ## Responsibilities
 * - Keep the original provider-boundary failure coverage from `src/app/shell-data.test.tsx` intact.
 * - Verify shell actions still surface backend and subscription failures with the same error-shaping contract.
 * - Confirm `useShellData` still throws a clear invariant when rendered outside `ShellDataProvider`.
 *
 * ## Not responsible for
 * - Re-testing happy-path shell mutations, app-lock flows, or backup progress success cases covered elsewhere.
 * - Changing the shared shell-data harness or inventing new provider abstractions for the split suites.
 * - Expanding the legacy assertions beyond the one extracted regression case.
 *
 * ## Dependencies
 * - Depends on the shared `test-helpers` harness for archive seeding, provider rendering, and mock reset behavior.
 * - Uses the real backend mock surface plus the shared `subscribeToBackupProgress` mock contract.
 *
 * ## Performance notes
 * - Reuses the seeded archive bootstrap from `test-helpers` so the split suite avoids duplicating expensive setup work.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createTranslator } from '../../lib/i18n'
import {
  getBackupProgressMock,
  getDefaultBuildInfo,
  renderShellProbe,
  resetShellDataHarness,
  seedSnapshot,
  ShellProbe,
} from './test-helpers'

describe('ShellDataProvider', () => {
  beforeEach(() => {
    resetShellDataHarness()
  })

  test('surfaces provider errors and context misuse clearly', async () => {
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined)
    const user = userEvent.setup()
    const translator = createTranslator('en')
    const { dashboard, snapshot } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    vi.spyOn(backend, 'saveConfig')
      .mockRejectedValueOnce(new Error('save failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'initializeArchive')
      .mockRejectedValueOnce(new Error('initialize failed'))
      .mockRejectedValueOnce('not-an-error')
    vi.spyOn(backend, 'runBackupNow')
      .mockRejectedValueOnce(new Error('backup failed'))
      .mockRejectedValueOnce('not-an-error')

    renderShellProbe()

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('save failed'),
    )

    await user.click(screen.getByRole('button', { name: 'save' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.savingSettingsFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'initialize failed',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.initializeArchiveFailed'),
      ),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('backup failed'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.manualBackupFailed'),
      ),
    )

    getBackupProgressMock().mockRejectedValueOnce(new Error('subscribe failed'))
    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('subscribe failed'),
    )

    expect(() => render(<ShellProbe />)).toThrow(
      'useShellData must be used inside ShellDataProvider',
    )

    consoleError.mockRestore()
  })
})
