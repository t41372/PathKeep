/**
 * @file index.test.tsx
 * @description Route-shell coverage for Security page gates and panel wiring.
 * @module pages/security
 *
 * ## Responsibilities
 * - Verify loading, unavailable, uninitialized, and keyring-unavailable route gates.
 * - Exercise route-owned deep-link focus and open-path wiring around extracted panels.
 *
 * ## Not responsible for
 * - Re-testing backend security mutations; `use-security-workflow.test.tsx` owns that.
 * - Re-testing detailed Security panel rendering; `panels.test.tsx` owns that.
 *
 * ## Dependencies
 * - Mocks the shell-data and workflow hooks so each route branch is deterministic.
 *
 * ## Performance notes
 * - Route-shell tests avoid native backend setup and stay small enough for strict coverage runs.
 */

import type { RefObject } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { I18nProvider } from '../../lib/i18n'
import type { SecurityStatus } from '../../lib/types'
import { SecurityPage } from './index'

const { shellDataMock, unlockPanelMock, workflowMock } = vi.hoisted(() => ({
  shellDataMock: vi.fn(),
  unlockPanelMock: { attachRef: true },
  workflowMock: vi.fn(),
}))

vi.mock('../../app/shell-data-context', () => ({
  useShellData: shellDataMock,
}))

vi.mock('./use-security-workflow', () => ({
  useSecurityWorkflow: workflowMock,
}))

vi.mock('./panels', () => ({
  SecurityStatusPanel: ({
    onOpenPath,
    status,
  }: {
    onOpenPath: (path: string) => void
    status: SecurityStatus
  }) => (
    <section>
      <span>status panel:{String(status.unlocked)}</span>
      <button type="button" onClick={() => onOpenPath('/tmp/security.sqlite')}>
        open security path
      </button>
    </section>
  ),
  SecurityUnlockPanel: ({
    unlockInputRef,
  }: {
    unlockInputRef: RefObject<HTMLInputElement | null>
  }) =>
    unlockPanelMock.attachRef ? (
      <input aria-label="unlock-input" ref={unlockInputRef} />
    ) : (
      <div>unlock field unavailable</div>
    ),
  SecurityRekeyPanel: ({
    localizedWarning,
  }: {
    localizedWarning: (warning: string) => string
  }) => (
    <section>
      rekey panel
      <span>
        {localizedWarning('database key is required for encrypted archives')}
      </span>
    </section>
  ),
}))

describe('SecurityPage route shell', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    unlockPanelMock.attachRef = true
    shellDataMock.mockReturnValue({
      refreshAppData: vi.fn().mockResolvedValue(undefined),
      refreshKey: 1,
    })
    workflowMock.mockReturnValue(workflowFixture())
  })

  test('renders loading, unavailable, and not-initialized gates', () => {
    workflowMock.mockReturnValueOnce(
      workflowFixture({ status: null, pageError: null }),
    )
    const { rerender } = renderPage()

    expect(screen.getByText('Loading security status…')).toBeVisible()

    workflowMock.mockReturnValueOnce(
      workflowFixture({
        status: null,
        pageError: 'security bridge offline',
      }),
    )
    rerender(pageNode())
    expect(screen.getByText('Security status unavailable')).toBeVisible()
    expect(screen.getByText('security bridge offline')).toBeVisible()

    workflowMock.mockReturnValueOnce(
      workflowFixture({
        status: securityStatusFixture({ initialized: false }),
      }),
    )
    rerender(pageNode())
    expect(screen.getByText('No archive yet')).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Set up archive first' }),
    ).toHaveAttribute('href', '/onboarding')
  })

  test('wires keyring warning, panel open-path action, and unlock deep-link focus', async () => {
    const user = userEvent.setup()
    const scrollIntoView = vi.fn()
    Element.prototype.scrollIntoView = scrollIntoView
    const openPath = vi
      .spyOn(backend, 'openPathInFileManager')
      .mockResolvedValue('/tmp/security.sqlite')

    workflowMock.mockReturnValue(
      workflowFixture({
        status: securityStatusFixture({
          encrypted: true,
          initialized: true,
          keyringStatus: {
            available: false,
            backend: 'unavailable',
            storedSecret: false,
          },
          unlocked: false,
          warnings: ['database key is required for encrypted archives'],
        }),
      }),
    )

    renderPage('/security#unlock-archive')

    expect(
      await screen.findByText('System keychain not available'),
    ).toBeVisible()
    expect(screen.getByRole('link', { name: 'Settings' })).toHaveAttribute(
      'href',
      '/settings',
    )
    await waitFor(() =>
      expect(screen.getByLabelText('unlock-input')).toHaveFocus(),
    )
    expect(scrollIntoView).toHaveBeenCalledWith({ block: 'center' })

    await user.click(screen.getByRole('button', { name: 'open security path' }))
    expect(openPath).toHaveBeenCalledWith('/tmp/security.sqlite')
    expect(screen.getByText('rekey panel')).toBeVisible()
    expect(
      screen.getByText(
        'Unlock this encrypted archive with its current password before reviewing history or audit data.',
      ),
    ).toBeVisible()
  })

  test('does not crash a deep-link focus request when the unlock field is not mounted', () => {
    unlockPanelMock.attachRef = false
    workflowMock.mockReturnValue(
      workflowFixture({
        status: securityStatusFixture({
          encrypted: true,
          initialized: true,
          unlocked: false,
        }),
      }),
    )

    expect(() => renderPage('/security#unlock-archive')).not.toThrow()
    expect(screen.getByText('unlock field unavailable')).toBeVisible()
  })

  test('skips the scrollIntoView nudge when the host environment lacks the API', async () => {
    // Branch coverage for L94 `typeof unlockInput.scrollIntoView === 'function'`.
    // jsdom doesn't ship scrollIntoView; the test setup installs a noop in
    // `beforeAll` so most assertions can rely on it. To cover the falsy
    // branch we replace the prototype property with a non-function value
    // for the duration of one render, then restore it.
    const originalDescriptor = Object.getOwnPropertyDescriptor(
      Element.prototype,
      'scrollIntoView',
    )
    Object.defineProperty(Element.prototype, 'scrollIntoView', {
      configurable: true,
      writable: true,
      value: undefined,
    })

    try {
      workflowMock.mockReturnValue(
        workflowFixture({
          status: securityStatusFixture({
            encrypted: true,
            initialized: true,
            unlocked: false,
          }),
        }),
      )

      renderPage('/security#unlock-archive')

      // Even without scrollIntoView available, the focus call still lands —
      // the route shell must not throw or short-circuit unrelated work.
      await waitFor(() =>
        expect(screen.getByLabelText('unlock-input')).toHaveFocus(),
      )
    } finally {
      if (originalDescriptor) {
        Object.defineProperty(
          Element.prototype,
          'scrollIntoView',
          originalDescriptor,
        )
      } else {
        Reflect.deleteProperty(Element.prototype, 'scrollIntoView')
      }
    }
  })

  test('renders the busy overlay while a security workflow is in flight', () => {
    workflowMock.mockReturnValue(
      workflowFixture({
        busy: 'Unlocking archive…',
      }),
    )

    renderPage()

    expect(screen.getByText('Unlocking archive…')).toBeVisible()
  })
})

function renderPage(route = '/security') {
  return render(pageNode(route))
}

function pageNode(route = '/security') {
  return (
    <MemoryRouter initialEntries={[route]}>
      <I18nProvider>
        <SecurityPage />
      </I18nProvider>
    </MemoryRouter>
  )
}

type SecurityWorkflowFixture = Omit<
  ReturnType<typeof workflowBase>,
  'pageError' | 'status'
> & {
  pageError: string | null
  status: SecurityStatus | null
}

function workflowFixture(
  overrides: Partial<SecurityWorkflowFixture> = {},
): SecurityWorkflowFixture {
  return {
    ...workflowBase(),
    ...overrides,
  }
}

function workflowBase() {
  return {
    actionError: null,
    busy: null as string | null,
    handleClearKeyring: vi.fn(),
    handleExecuteRekey: vi.fn(),
    handleLockArchive: vi.fn(),
    handlePreviewRekey: vi.fn(),
    handleStoreKeyringKey: vi.fn(),
    handleUnlock: vi.fn(),
    handleUnlockFromKeyring: vi.fn(),
    notice: null,
    pageError: null,
    preview: null,
    rekeyConfirmText: '',
    rekeyKey: '',
    rekeyMode: 'Encrypted' as const,
    saveRekeyKey: false,
    sessionKey: '',
    setPreview: vi.fn(),
    setRekeyConfirmText: vi.fn(),
    setRekeyKey: vi.fn(),
    setRekeyMode: vi.fn(),
    setSaveRekeyKey: vi.fn(),
    setSessionKey: vi.fn(),
    status: securityStatusFixture(),
  }
}

function securityStatusFixture(
  overrides: Partial<SecurityStatus> = {},
): SecurityStatus {
  return {
    initialized: true,
    mode: 'Encrypted',
    encrypted: true,
    unlocked: true,
    databasePath: '/tmp/history.sqlite',
    strongholdPath: '/tmp/stronghold',
    rememberDatabaseKeyInKeyring: false,
    lastSuccessfulBackupAt: null,
    lastRekeyAt: null,
    lastRekeyRunId: null,
    lastRekeySnapshotPath: null,
    keyringStatus: {
      available: true,
      backend: 'file-backed-test',
      storedSecret: false,
    },
    warnings: [],
    ...overrides,
  }
}
