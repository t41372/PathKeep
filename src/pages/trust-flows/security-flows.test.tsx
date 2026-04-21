/**
 * @file security-flows.test.tsx
 * @description Focused trust-flow regression suite for Security route review, locked-archive recovery, and unlock failure behavior.
 * @module pages/trust-flows
 *
 * ## Responsibilities
 * - Preserve the shipped Security route trust-flow promises that were previously covered by the mega-suite.
 * - Verify translated rekey review, locked-archive warning localization, and fail-fast unlock behavior.
 * - Reuse the canonical trust-flow test harness instead of rebuilding page-level providers in every case.
 *
 * ## Non-Responsibilities
 * - Does not own Security page implementation details outside these five inherited regressions.
 * - Does not redefine shared trust-flow helpers or migrate the original mega-suite cutover on its own.
 * - Does not validate unrelated Import, Schedule, Settings, or Audit route flows.
 *
 * ## Dependencies
 * - Depends on the Security route, shared trust-flow harness helpers, and the backend client test harness.
 * - Keeps the Tauri core and import-progress module boundaries mocked at the suite edge so route loading stays deterministic.
 *
 * ## Performance Notes
 * - Reuses the seeded archive snapshot helper to avoid multiplying route bootstrap work while splitting the mega-suite.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'

const { invoke, isTauri, subscribeToImportProgress } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
  subscribeToImportProgress: vi.fn(() => Promise.resolve(vi.fn())),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

vi.mock('../../lib/ipc/import-progress', () => ({
  subscribeToImportProgress,
}))

import { ShellDataContext } from '../../app/shell-data-context'
import { createNamespaceTranslator, createTranslator } from '../../lib/i18n'
import { I18nContext } from '../../lib/i18n/context'
import { backend } from '../../lib/backend-client'
import { securityModeKey } from '../../lib/trust-review'
import { SecurityPage } from '../security'
import {
  createI18nValue,
  createShellValue,
  renderTrustPage,
  resetTrustFlowHarness,
  seedInitializedSnapshot,
} from './test-helpers'

describe('trust flows security', () => {
  beforeEach(() => {
    resetTrustFlowHarness({ invoke, isTauri, subscribeToImportProgress })
  })

  test('renders rekey preview in Traditional Chinese without English mode fallbacks', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    const securityT = createNamespaceTranslator('zh-TW', 'security')
    const zhTwT = createTranslator('zh-TW')

    renderTrustPage(<SecurityPage />, {
      language: 'zh-TW',
      route: '/security',
      snapshot,
    })

    expect(
      await screen.findByText(
        zhTwT('security.archiveIs', {
          mode: zhTwT(securityModeKey('encrypted')),
        }),
      ),
    ).toBeVisible()

    await user.selectOptions(
      screen.getByLabelText(securityT('targetMode')),
      screen.getByRole('option', { name: '明文' }),
    )
    await user.click(
      screen.getByRole('button', { name: securityT('previewRekey') }),
    )

    expect(await screen.findByText('加密 → 明文')).toBeVisible()
  })

  test('shows the latest rekey review path and audit shortcut on the security page', async () => {
    await seedInitializedSnapshot()
    await backend.rekeyArchive({ newMode: 'Plaintext', newKey: null })
    const snapshot = await backend.getAppSnapshot()

    renderTrustPage(<SecurityPage />, {
      language: 'en',
      route: '/security',
      snapshot,
    })

    expect(await screen.findByText(/archive-before-rekey/)).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Open last rekey review' }),
    ).toHaveAttribute('href', expect.stringContaining('/audit?run='))
  })

  test('renders the security unlock flow even when the shell snapshot is unavailable', async () => {
    const { snapshot } = await seedInitializedSnapshot()
    await backend.clearSessionDatabaseKey()

    render(
      <MemoryRouter initialEntries={['/security']}>
        <I18nContext.Provider value={createI18nValue('en')}>
          <ShellDataContext.Provider
            value={{
              ...createShellValue(snapshot),
              snapshot: null,
              dashboard: null,
              error: 'database key is required for encrypted archives',
            }}
          >
            <SecurityPage />
          </ShellDataContext.Provider>
        </I18nContext.Provider>
      </MemoryRouter>,
    )

    expect(await screen.findByText('ENCRYPTION')).toBeVisible()
    expect(screen.getByRole('button', { name: 'Unlock' })).toBeVisible()
    expect(screen.getByText('Archive is Encrypted / Locked')).toBeVisible()
    expect(
      screen.getByText('Locked — unlock to browse history and view audit logs'),
    ).toBeVisible()
  })

  test('localizes locked-archive security warnings instead of rendering raw backend English', async () => {
    const { snapshot } = await seedInitializedSnapshot()
    await backend.clearSessionDatabaseKey()
    const lockedStatus = await backend.securityStatus()
    const securityStatusSpy = vi
      .spyOn(backend, 'securityStatus')
      .mockResolvedValue({
        ...lockedStatus,
        warnings: ['database key is required for encrypted archives'],
      })

    renderTrustPage(<SecurityPage />, {
      language: 'zh-TW',
      route: '/security',
      snapshot,
    })

    expect(
      await screen.findByText(
        '請先用目前密碼解鎖這個加密封存，再查看歷史記錄或稽核資料。',
      ),
    ).toBeVisible()
    expect(
      screen.queryByText('database key is required for encrypted archives'),
    ).not.toBeInTheDocument()

    securityStatusSpy.mockRestore()
  })

  test('fails fast when a candidate archive key does not unlock the archive', async () => {
    const user = userEvent.setup()
    const { snapshot } = await seedInitializedSnapshot()
    await backend.clearSessionDatabaseKey()
    const lockedStatus = await backend.securityStatus()
    const refreshSpy = vi.fn().mockResolvedValue(undefined)
    const setSessionSpy = vi
      .spyOn(backend, 'setSessionDatabaseKey')
      .mockResolvedValue(undefined)
    const clearSessionSpy = vi
      .spyOn(backend, 'clearSessionDatabaseKey')
      .mockResolvedValue(undefined)
    const securityStatusSpy = vi
      .spyOn(backend, 'securityStatus')
      .mockResolvedValueOnce(lockedStatus)
      .mockResolvedValueOnce(lockedStatus)

    render(
      <MemoryRouter initialEntries={['/security#unlock-archive']}>
        <I18nContext.Provider value={createI18nValue('en')}>
          <ShellDataContext.Provider
            value={{
              ...createShellValue(snapshot),
              refreshAppData: refreshSpy,
            }}
          >
            <SecurityPage />
          </ShellDataContext.Provider>
        </I18nContext.Provider>
      </MemoryRouter>,
    )

    await user.type(await screen.findByLabelText('PASSWORD'), '000000')
    await user.click(screen.getByRole('button', { name: 'Unlock' }))

    expect(
      await screen.findByText(
        'That key did not unlock this archive. Check the password or saved key, then try again.',
      ),
    ).toBeVisible()
    expect(setSessionSpy).toHaveBeenCalledWith('000000')
    expect(clearSessionSpy).toHaveBeenCalled()
    expect(refreshSpy).not.toHaveBeenCalled()
    expect(securityStatusSpy).toHaveBeenCalledTimes(2)
  })
})
