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
import { renderToString } from 'react-dom/server'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createTranslator } from '../../lib/i18n'
import { I18nContext } from '../../lib/i18n/context'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import { ShellDataProvider } from '../shell-data'
import {
  createI18nValue,
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

  test('server render starts in the loading state before bootstrap effects run', () => {
    const html = renderToString(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <ShellProbe />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nContext.Provider>,
    )

    expect(html).toContain('data-testid="loading">true</div>')
  })

  test('uses the current language for refresh fallbacks after the i18n context changes', async () => {
    const user = userEvent.setup()
    const { dashboard, snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'getAppSnapshot')
      .mockResolvedValueOnce(snapshot)
      .mockRejectedValueOnce('refresh offline')
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)

    const view = render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <ShellProbe />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )
    view.rerender(
      <I18nContext.Provider value={createI18nValue('zh-TW')}>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <ShellProbe />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nContext.Provider>,
    )

    await user.click(screen.getByRole('button', { name: 'refresh' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('refresh offline'),
    )
  })

  test('uses the current language for automatic dashboard refresh fallbacks after i18n changes', async () => {
    const { dashboard, snapshot } = await seedSnapshot()
    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    const loadDashboardSnapshotSpy = vi
      .spyOn(backend, 'loadDashboardSnapshot')
      .mockResolvedValue(dashboard)

    const view = render(
      <I18nContext.Provider value={createI18nValue('en')}>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <ShellProbe />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('dashboard-generated-at')).toHaveTextContent(
        dashboard.generatedAt,
      ),
    )
    await waitFor(() =>
      expect(screen.getByTestId('dashboard-loading')).toHaveTextContent(
        'false',
      ),
    )
    const callsBeforeLocaleSwitch = loadDashboardSnapshotSpy.mock.calls.length
    loadDashboardSnapshotSpy.mockRejectedValueOnce('dashboard offline')

    view.rerender(
      <I18nContext.Provider value={createI18nValue('zh-TW')}>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <ShellProbe />
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nContext.Provider>,
    )

    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'dashboard offline',
      ),
    )
    expect(loadDashboardSnapshotSpy.mock.calls.length).toBeGreaterThan(
      callsBeforeLocaleSwitch,
    )
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
      .mockRejectedValueOnce(
        new Error(
          'processing profile safari:Default: Safari History.db is not readable yet. Grant Full Disk Access to PathKeep or the running development process, then run the backup again.',
        ),
      )

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
      expect(screen.getByTestId('error')).toHaveTextContent('not-an-error'),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        'initialize failed',
      ),
    )

    await user.click(screen.getByRole('button', { name: 'initialize' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('not-an-error'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('backup failed'),
    )
    // An ordinary backup failure is classified as 'backup' (not FDA) so the shell
    // renders the backup-specific toast — never 'full-disk-access'.
    expect(screen.getByTestId('error-kind')).toHaveTextContent('backup')

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('not-an-error'),
    )

    await user.click(screen.getByRole('button', { name: 'backup' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent(
        translator('shell.fullDiskAccessBackupError'),
      ),
    )
    // The FDA failure classifies the error via locale-independent state — proving
    // the wrapped `setError` reset the kind and the FDA branch re-asserted it.
    expect(screen.getByTestId('error-kind')).toHaveTextContent(
      'full-disk-access',
    )
    expect(screen.getByTestId('busy-label')).toHaveTextContent('none')

    // Dismissing the error must clear BOTH the message and the locale-independent
    // classification, so a later unrelated error never inherits a stale FDA kind.
    await user.click(screen.getByRole('button', { name: 'clear-error' }))
    await waitFor(() =>
      expect(screen.getByTestId('error')).toHaveTextContent('none'),
    )
    expect(screen.getByTestId('error-kind')).toHaveTextContent('none')

    expect(() => render(<ShellProbe />)).toThrow(
      'useShellData must be used inside ShellDataProvider',
    )

    consoleError.mockRestore()
  })
})
