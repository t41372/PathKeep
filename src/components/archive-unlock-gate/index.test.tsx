/**
 * @file index.test.tsx
 * @description Coverage for the ArchiveUnlockGate blocking unlock overlay.
 * @module components/archive-unlock-gate
 *
 * ## What this suite covers
 * - Gate renders (and mounts dialog role) when provided a locked encrypted snapshot.
 * - Password autofocus happens on mount.
 * - "Use saved password" button is shown when keychain has a stored secret.
 * - Successful manual unlock: setSessionDatabaseKey + securityStatus + afterSuccessfulUnlock path.
 * - "Remember on this device" checked → stores in keyring + saves config on success.
 * - reconcileArchiveEncryption is called fire-and-forget after unlock.
 * - Wrong password (securityStatus.unlocked = false) surfaces an error, gate stays mounted.
 * - retryBackupOnUnlock=true → onRetryBackup() is called after refreshAppData resolves.
 */

import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
} from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { backend } from '../../lib/backend-client'
import type * as BackendClient from '../../lib/backend-client'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import type {
  AppSnapshot,
  AppConfig,
  KeyringStatusReport,
  ReconcileReport,
  RecoverySnapshot,
} from '../../lib/types'
import { ArchiveUnlockGate } from './index'

// ──────────────────────────────────────────────────────────────────────────────
// Mock backend so IPC calls never actually fire.
// ──────────────────────────────────────────────────────────────────────────────
vi.mock('../../lib/backend-client', async (importOriginal) => {
  const actual = await importOriginal<typeof BackendClient>()
  return {
    ...actual,
    backend: {
      ...actual.backend,
      setSessionDatabaseKey: vi.fn().mockResolvedValue(undefined),
      clearSessionDatabaseKey: vi.fn().mockResolvedValue(undefined),
      securityStatus: vi.fn().mockResolvedValue({ unlocked: true }),
      keyringGetDatabaseKey: vi.fn().mockResolvedValue('saved-pw'),
      keyringStoreDatabaseKey: vi.fn().mockResolvedValue({
        available: true,
        backend: 'macOS Keychain',
        storedSecret: true,
      }),
      keyringClearDatabaseKey: vi.fn().mockResolvedValue({
        available: true,
        backend: 'macOS Keychain',
        storedSecret: false,
      }),
      reconcileArchiveEncryption: vi.fn().mockResolvedValue({
        repaired: false,
        fromMode: null,
        toMode: 'Encrypted',
      } satisfies ReconcileReport),
      // Used by the SnapshotRecoveryPanel rendered inside the recover-mode escape hatch.
      listRecoverySnapshots: vi.fn().mockResolvedValue([]),
      revealLogs: vi.fn().mockResolvedValue(undefined),
    },
  }
})

// ──────────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ──────────────────────────────────────────────────────────────────────────────

function keyringFixture(
  overrides: Partial<KeyringStatusReport> = {},
): KeyringStatusReport {
  return {
    available: true,
    backend: 'macOS Keychain',
    storedSecret: false,
    ...overrides,
  }
}

function configFixture(overrides: Partial<AppConfig> = {}): AppConfig {
  return {
    initialized: true,
    archiveMode: 'Encrypted',
    preferredLanguage: 'en',
    dueAfterHours: 24,
    scheduleCheckIntervalHours: 1,
    checkpointDays: 7,
    captureFavicons: true,
    selectedProfileIds: [],
    gitEnabled: false,
    rememberDatabaseKeyInKeyring: false,
    appAutostart: false,
    explorerBackgroundPrefetchPages: 2,
    appLock: {
      enabled: false,
      idleTimeoutMinutes: 15,
      biometricEnabled: false,
    },
    enrichment: { enabled: false, mode: 'manual' },
    deterministic: { deterministicMode: false },
    ai: {
      baseUrl: '',
      embeddingModel: '',
      llmModel: '',
      apiKey: null,
      embeddingEnabled: false,
    },
    ...overrides,
  } as AppConfig
}

function snapshotFixture(
  overrides: Partial<{
    encrypted: boolean
    unlocked: boolean
    keyring: Partial<KeyringStatusReport>
    config: Partial<AppConfig>
  }> = {},
): AppSnapshot {
  const {
    encrypted = true,
    unlocked = false,
    keyring = {},
    config = {},
  } = overrides

  return {
    config: configFixture(config),
    archiveStatus: {
      initialized: true,
      encrypted,
      unlocked,
      warning: null,
    },
    keyringStatus: keyringFixture(keyring),
    browserProfiles: [],
  } as unknown as AppSnapshot
}

function shellContextValue(
  overrides: Partial<ShellDataContextValue> = {},
): ShellDataContextValue {
  const refreshAppData = vi.fn().mockResolvedValue(undefined)
  const saveConfig = vi.fn().mockResolvedValue({})
  return {
    buildInfo: null,
    appLockStatus: null,
    snapshot: null,
    dashboard: null,
    loading: false,
    busyAction: null,
    busyOverlay: null,
    error: null,
    rawError: null,
    notice: null,
    refreshKey: 0,
    errorKind: null,
    refreshAppData,
    refreshRuntimeStatus: vi.fn().mockResolvedValue({}),
    saveConfig,
    initializeArchive: vi.fn(),
    runBackup: vi.fn().mockResolvedValue({}),
    setAppLockPasscode: vi.fn(),
    clearAppLockPasscode: vi.fn(),
    lockAppSession: vi.fn().mockResolvedValue({}),
    unlockAppSession: vi.fn().mockResolvedValue({}),
    clearNotice: vi.fn(),
    clearError: vi.fn(),
    recovery: null,
    runFullArchiveRestore: vi.fn().mockResolvedValue({}),
    ...overrides,
  } as ShellDataContextValue
}

function renderGate(
  snapshot: AppSnapshot = snapshotFixture(),
  shellOverrides: Partial<ShellDataContextValue> = {},
  gateProps: Partial<{
    retryBackupOnUnlock: boolean
    onRetryBackup: () => void
  }> = {},
) {
  const shell = shellContextValue(shellOverrides)
  return {
    shell,
    ...render(
      <I18nProvider>
        <ShellDataContext.Provider value={shell}>
          <ArchiveUnlockGate snapshot={snapshot} {...gateProps} />
        </ShellDataContext.Provider>
      </I18nProvider>,
    ),
  }
}

// ──────────────────────────────────────────────────────────────────────────────
// Reset mocks between tests
// ──────────────────────────────────────────────────────────────────────────────
beforeEach(() => {
  vi.mocked(backend.setSessionDatabaseKey).mockResolvedValue(undefined)
  vi.mocked(backend.clearSessionDatabaseKey).mockResolvedValue(undefined)
  vi.mocked(backend.securityStatus).mockResolvedValue({
    unlocked: true,
  } as Awaited<ReturnType<typeof backend.securityStatus>>)
  vi.mocked(backend.keyringGetDatabaseKey).mockResolvedValue('saved-pw')
  vi.mocked(backend.keyringStoreDatabaseKey).mockResolvedValue({
    available: true,
    backend: 'macOS Keychain',
    storedSecret: true,
  })
  vi.mocked(backend.keyringClearDatabaseKey).mockResolvedValue({
    available: true,
    backend: 'macOS Keychain',
    storedSecret: false,
  })
  vi.mocked(backend.reconcileArchiveEncryption).mockResolvedValue({
    repaired: false,
    fromMode: null,
    toMode: 'Encrypted',
  })
  vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([])
  vi.mocked(backend.revealLogs).mockResolvedValue('' as never)
})

// A minimal recovery snapshot fixture for the recover-mode escape-hatch tests.
function recoverySnapshotFixture(
  overrides: Partial<RecoverySnapshot> = {},
): RecoverySnapshot {
  return {
    id: 'snap-1',
    path: '/snap.sqlite',
    createdAt: '2026-06-01T10:00:00Z',
    sizeBytes: 1024 * 1024,
    verifiedOpenable: true,
    encrypted: false,
    sourceOp: 'rekey',
    label: 'Encryption change',
    ...overrides,
  }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
describe('ArchiveUnlockGate', () => {
  test('renders dialog with title when archive is locked', () => {
    renderGate()
    expect(
      screen.getByRole('dialog', { name: /archive locked/i }),
    ).toBeInTheDocument()
    expect(screen.getByTestId('archive-unlock-gate')).toBeInTheDocument()
  })

  test('password input is present and label is correct', () => {
    renderGate()
    expect(screen.getByLabelText(/PASSWORD/i)).toBeInTheDocument()
  })

  test('"Use saved password" button is hidden when keychain has no stored secret', () => {
    renderGate(snapshotFixture({ keyring: { storedSecret: false } }))
    expect(screen.queryByText(/use saved password/i)).toBeNull()
  })

  test('"Use saved password" button is visible when keychain has a stored secret', () => {
    renderGate(snapshotFixture({ keyring: { storedSecret: true } }))
    expect(
      screen.getByRole('button', { name: /use saved password/i }),
    ).toBeInTheDocument()
  })

  test('remember checkbox is checked by default when keychain is available', () => {
    renderGate(
      snapshotFixture({ keyring: { available: true, storedSecret: false } }),
    )
    const checkbox = screen.getByRole('checkbox')
    expect(checkbox).toBeChecked()
  })

  test('remember checkbox is absent when keychain is unavailable', () => {
    renderGate(
      snapshotFixture({ keyring: { available: false, storedSecret: false } }),
    )
    expect(screen.queryByRole('checkbox')).toBeNull()
  })

  test('shows backup-retry body text when retryBackupOnUnlock=true', () => {
    renderGate(snapshotFixture(), {}, { retryBackupOnUnlock: true })
    expect(
      screen.getByText(/the last backup could not run/i),
    ).toBeInTheDocument()
  })

  test('successful unlock: calls setSessionDatabaseKey, securityStatus, reconcile, and refreshAppData', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    renderGate(snapshotFixture(), { refreshAppData })

    fireEvent.change(screen.getByLabelText(/PASSWORD/i), {
      target: { value: 'correct-pw' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }))

    await waitFor(() =>
      expect(backend.setSessionDatabaseKey).toHaveBeenCalledWith('correct-pw'),
    )
    expect(backend.securityStatus).toHaveBeenCalled()
    await waitFor(() => expect(refreshAppData).toHaveBeenCalled())
    // reconcile is fire-and-forget but still invoked
    expect(backend.reconcileArchiveEncryption).toHaveBeenCalled()
  })

  test('remember checked + keyring available: stores key and saves config on unlock', async () => {
    const saveConfig = vi.fn().mockResolvedValue({})
    const snap = snapshotFixture({
      keyring: { available: true, storedSecret: false },
    })
    renderGate(snap, { saveConfig })

    // Checkbox starts checked (default when keyring available).
    fireEvent.change(screen.getByLabelText(/PASSWORD/i), {
      target: { value: 'my-pw' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }))

    await waitFor(() =>
      expect(backend.keyringStoreDatabaseKey).toHaveBeenCalledWith('my-pw'),
    )
    expect(saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({ rememberDatabaseKeyInKeyring: true }),
      { quiet: true },
    )
  })

  test('remember unchecked: clears keychain + persists opt-out (M1), never stores', async () => {
    const saveConfig = vi.fn().mockResolvedValue({})
    const snap = snapshotFixture({
      keyring: { available: true, storedSecret: true },
    })
    renderGate(snap, { saveConfig })

    fireEvent.click(screen.getByRole('checkbox')) // uncheck
    // Exact label (not /password/i) so we don't also match the
    // "Use saved password" button that appears when a secret is stored.
    fireEvent.change(screen.getByLabelText('PASSWORD'), {
      target: { value: 'my-pw' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }))

    await waitFor(() =>
      expect(backend.setSessionDatabaseKey).toHaveBeenCalledWith('my-pw'),
    )
    // Unchecking actively opts out: clear the stored key AND persist the flag,
    // so a previously stored key cannot keep auto-unlocking (no stale desync).
    await waitFor(() =>
      expect(backend.keyringClearDatabaseKey).toHaveBeenCalledTimes(1),
    )
    expect(saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({ rememberDatabaseKeyInKeyring: false }),
      { quiet: true },
    )
    expect(backend.keyringStoreDatabaseKey).not.toHaveBeenCalled()
  })

  test('wrong password: surfaces error and gate stays visible', async () => {
    vi.mocked(backend.securityStatus).mockResolvedValue({
      unlocked: false,
    } as Awaited<ReturnType<typeof backend.securityStatus>>)

    renderGate()
    fireEvent.change(screen.getByLabelText(/PASSWORD/i), {
      target: { value: 'wrong-pw' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }))

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    // Gate is still visible (no refresh happened).
    expect(screen.getByTestId('archive-unlock-gate')).toBeInTheDocument()
  })

  test('empty password: shows required error without calling backend', async () => {
    renderGate()
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }))

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(backend.setSessionDatabaseKey).not.toHaveBeenCalled()
  })

  test('use saved password path: retrieves from keyring and calls afterSuccessfulUnlock', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const snap = snapshotFixture({
      keyring: { storedSecret: true, available: true },
    })
    renderGate(snap, { refreshAppData })

    fireEvent.click(screen.getByRole('button', { name: /use saved password/i }))

    await waitFor(() =>
      expect(backend.keyringGetDatabaseKey).toHaveBeenCalled(),
    )
    expect(backend.setSessionDatabaseKey).toHaveBeenCalledWith('saved-pw')
    await waitFor(() => expect(refreshAppData).toHaveBeenCalled())
  })

  test('retryBackupOnUnlock: calls onRetryBackup after refreshAppData resolves', async () => {
    const onRetryBackup = vi.fn()
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    renderGate(
      snapshotFixture(),
      { refreshAppData },
      { retryBackupOnUnlock: true, onRetryBackup },
    )

    fireEvent.change(screen.getByLabelText(/PASSWORD/i), {
      target: { value: 'correct-pw' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }))

    await waitFor(() => expect(refreshAppData).toHaveBeenCalled())
    expect(onRetryBackup).toHaveBeenCalledTimes(1)
  })

  test('reconcile rejection is swallowed (fire-and-forget) — unlock still completes', async () => {
    // Force the reconcile promise to reject so the `.catch(() => undefined)`
    // tail runs; the unlock flow must still refresh and complete.
    vi.mocked(backend.reconcileArchiveEncryption).mockRejectedValue(
      new Error('reconcile boom'),
    )
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    renderGate(
      snapshotFixture({ keyring: { available: false, storedSecret: false } }),
      { refreshAppData },
    )

    fireEvent.change(screen.getByLabelText(/PASSWORD/i), {
      target: { value: 'correct-pw' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }))

    await waitFor(() => expect(refreshAppData).toHaveBeenCalled())
    expect(backend.reconcileArchiveEncryption).toHaveBeenCalled()
  })

  test('keychain store failure is swallowed — unlock still refreshes', async () => {
    // The keychain sync is best-effort: a store rejection must NOT block unlock.
    vi.mocked(backend.keyringStoreDatabaseKey).mockRejectedValue(
      new Error('store boom'),
    )
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    renderGate(
      snapshotFixture({ keyring: { available: true, storedSecret: false } }),
      { refreshAppData },
    )

    fireEvent.change(screen.getByLabelText(/PASSWORD/i), {
      target: { value: 'correct-pw' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }))

    await waitFor(() => expect(refreshAppData).toHaveBeenCalled())
  })

  test('use saved password: throws when keychain returns no key', async () => {
    vi.mocked(backend.keyringGetDatabaseKey).mockResolvedValue(null)
    const snap = snapshotFixture({
      keyring: { storedSecret: true, available: true },
    })
    renderGate(snap)

    fireEvent.click(screen.getByRole('button', { name: /use saved password/i }))

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(backend.setSessionDatabaseKey).not.toHaveBeenCalled()
  })

  test('use saved password: re-locks and surfaces error when status stays locked', async () => {
    vi.mocked(backend.securityStatus).mockResolvedValue({
      unlocked: false,
    } as Awaited<ReturnType<typeof backend.securityStatus>>)
    // Make the defensive clear reject too, so its `.catch(() => undefined)` runs.
    vi.mocked(backend.clearSessionDatabaseKey).mockRejectedValue(
      new Error('clear boom'),
    )
    const snap = snapshotFixture({
      keyring: { storedSecret: true, available: true },
    })
    renderGate(snap)

    fireEvent.click(screen.getByRole('button', { name: /use saved password/i }))

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(backend.clearSessionDatabaseKey).toHaveBeenCalled()
    expect(screen.getByTestId('archive-unlock-gate')).toBeInTheDocument()
  })

  test('manual unlock: defensive clear rejection is swallowed when status stays locked', async () => {
    vi.mocked(backend.securityStatus).mockResolvedValue({
      unlocked: false,
    } as Awaited<ReturnType<typeof backend.securityStatus>>)
    vi.mocked(backend.clearSessionDatabaseKey).mockRejectedValue(
      new Error('clear boom'),
    )
    renderGate()

    fireEvent.change(screen.getByLabelText(/PASSWORD/i), {
      target: { value: 'wrong-pw' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }))

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(backend.clearSessionDatabaseKey).toHaveBeenCalled()
  })

  test('focus trap: Tab from the last element wraps to the first', () => {
    renderGate(snapshotFixture({ keyring: { storedSecret: true } }))
    const focusables = within(
      screen.getByTestId('archive-unlock-gate'),
    ).getAllByRole('button')
    const first = screen.getByRole('button', { name: /use saved password/i })
    const last = focusables[focusables.length - 1]

    last.focus()
    expect(document.activeElement).toBe(last)
    // The trap handler is bound to the dialog; fire from the focused child so
    // the event bubbles up to it (as a real Tab keystroke would).
    fireEvent.keyDown(last, { key: 'Tab' })
    expect(document.activeElement).toBe(first)
  })

  test('focus trap: Shift+Tab from the first element wraps to the last', () => {
    renderGate(snapshotFixture({ keyring: { storedSecret: true } }))
    const buttons = within(
      screen.getByTestId('archive-unlock-gate'),
    ).getAllByRole('button')
    const first = screen.getByRole('button', { name: /use saved password/i })
    const last = buttons[buttons.length - 1]

    first.focus()
    expect(document.activeElement).toBe(first)
    fireEvent.keyDown(first, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(last)
  })

  test('focus trap: Tab in the middle does not force-wrap', () => {
    renderGate(snapshotFixture({ keyring: { storedSecret: true } }))
    const middle = screen.getByLabelText('PASSWORD')
    middle.focus()
    fireEvent.keyDown(middle, { key: 'Tab' })
    // Not on the last element, so the gate lets the browser handle it.
    expect(document.activeElement).toBe(middle)
  })

  test('focus trap: Shift+Tab in the middle does not force-wrap', () => {
    renderGate(snapshotFixture({ keyring: { storedSecret: true } }))
    const middle = screen.getByLabelText('PASSWORD')
    middle.focus()
    fireEvent.keyDown(middle, { key: 'Tab', shiftKey: true })
    // Not on the first element, so the gate lets the browser handle it.
    expect(document.activeElement).toBe(middle)
  })

  test('focus trap: a non-Tab key bubbling to the dialog is ignored', () => {
    renderGate()
    const gate = screen.getByTestId('archive-unlock-gate')
    // Should not throw or move focus — the handler early-returns on non-Tab keys.
    fireEvent.keyDown(gate, { key: 'ArrowDown' })
    expect(gate).toBeInTheDocument()
  })

  test('Escape key does NOT dismiss the gate', () => {
    renderGate()
    fireEvent.keyDown(screen.getByTestId('archive-unlock-gate'), {
      key: 'Escape',
    })
    expect(screen.getByTestId('archive-unlock-gate')).toBeInTheDocument()
  })

  test('Enter key in password field triggers unlock', async () => {
    renderGate()
    fireEvent.change(screen.getByLabelText(/PASSWORD/i), {
      target: { value: 'my-pw' },
    })
    fireEvent.keyDown(screen.getByLabelText(/PASSWORD/i), { key: 'Enter' })

    await waitFor(() =>
      expect(backend.setSessionDatabaseKey).toHaveBeenCalledWith('my-pw'),
    )
  })

  test('a non-Enter key in the password field does not trigger unlock', () => {
    renderGate()
    fireEvent.change(screen.getByLabelText(/PASSWORD/i), {
      target: { value: 'my-pw' },
    })
    fireEvent.keyDown(screen.getByLabelText(/PASSWORD/i), { key: 'a' })
    expect(backend.setSessionDatabaseKey).not.toHaveBeenCalled()
  })

  test('restores focus to the previously focused element on unmount', () => {
    const trigger = document.createElement('button')
    trigger.textContent = 'opener'
    document.body.appendChild(trigger)
    trigger.focus()
    expect(document.activeElement).toBe(trigger)

    const { unmount } = render(
      <I18nProvider>
        <ShellDataContext.Provider value={shellContextValue()}>
          <ArchiveUnlockGate snapshot={snapshotFixture()} />
        </ShellDataContext.Provider>
      </I18nProvider>,
    )
    unmount()
    expect(document.activeElement).toBe(trigger)
    document.body.removeChild(trigger)
  })

  // ── Recovery escape hatch: "Can't unlock? Recover from a snapshot" ─────────────

  test('unlock mode always shows the "Can\'t unlock? Recover" escape hatch', () => {
    renderGate()
    expect(
      screen.getByRole('button', {
        name: /recover from a saved snapshot instead/i,
      }),
    ).toBeInTheDocument()
  })

  test('clicking the escape hatch routes to recover mode and lists snapshots', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      recoverySnapshotFixture(),
    ])
    renderGate()

    fireEvent.click(
      screen.getByRole('button', {
        name: /recover from a saved snapshot instead/i,
      }),
    )

    // Recover-mode title + reassuring body are shown in the SAME dialog shell.
    expect(
      screen.getByRole('dialog', { name: /restore from a snapshot/i }),
    ).toBeInTheDocument()
    expect(screen.getByText(/locked out/i)).toBeInTheDocument()
    // The SnapshotRecoveryPanel loaded and listed the snapshot.
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    expect(backend.listRecoverySnapshots).toHaveBeenCalled()
  })

  test('recover mode prefills the archive key from the typed password', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      recoverySnapshotFixture({ encrypted: true }),
    ])
    renderGate()

    // Type a password in unlock mode first, then jump to recover.
    fireEvent.change(screen.getByLabelText('PASSWORD'), {
      target: { value: 'typed-pw' },
    })
    fireEvent.click(
      screen.getByRole('button', {
        name: /recover from a saved snapshot instead/i,
      }),
    )

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    // The panel seeded its key field from the password the user already typed.
    expect(screen.getByLabelText('Archive key')).toHaveValue('typed-pw')
  })

  test('"Back to unlock" returns to the password form', () => {
    renderGate()
    fireEvent.click(
      screen.getByRole('button', {
        name: /recover from a saved snapshot instead/i,
      }),
    )
    expect(
      screen.getByRole('dialog', { name: /restore from a snapshot/i }),
    ).toBeInTheDocument()

    fireEvent.click(
      screen.getByRole('button', { name: /go back to the unlock form/i }),
    )
    expect(
      screen.getByRole('dialog', { name: /archive locked/i }),
    ).toBeInTheDocument()
    expect(screen.getByLabelText('PASSWORD')).toBeInTheDocument()
  })

  test('a successful encrypted restore returns the user to unlock mode', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      recoverySnapshotFixture({ encrypted: true }),
    ])
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    renderGate(snapshotFixture(), { runFullArchiveRestore })

    fireEvent.click(
      screen.getByRole('button', {
        name: /recover from a saved snapshot instead/i,
      }),
    )
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.change(screen.getByLabelText('Archive key'), {
      target: { value: 'my-key' },
    })
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() =>
      expect(runFullArchiveRestore).toHaveBeenCalledWith(
        '/snap.sqlite',
        'my-key',
      ),
    )
    // onRestored flips the gate back to unlock mode so the user can enter their key once.
    await waitFor(() =>
      expect(
        screen.getByRole('dialog', { name: /archive locked/i }),
      ).toBeInTheDocument(),
    )
  })

  test('switching to recover mode keeps focus inside the dialog (trap intact)', () => {
    renderGate()
    // Simulate a keyboard user who tabbed onto the escape hatch before activating it.
    const hatch = screen.getByRole('button', {
      name: /recover from a saved snapshot instead/i,
    })
    hatch.focus()
    fireEvent.click(hatch)

    // The escape-hatch button unmounts with the unlock body; without explicit focus
    // management focus would drop to document.body and Tab could escape the modal.
    const dialog = screen.getByRole('dialog', {
      name: /restore from a snapshot/i,
    })
    expect(dialog.contains(document.activeElement)).toBe(true)
    expect(document.activeElement).toBe(
      screen.getByRole('heading', { name: /restore from a snapshot/i }),
    )
  })

  test('"Back to unlock" returns focus to the password field', () => {
    renderGate()
    fireEvent.click(
      screen.getByRole('button', {
        name: /recover from a saved snapshot instead/i,
      }),
    )
    const back = screen.getByRole('button', {
      name: /go back to the unlock form/i,
    })
    back.focus()
    fireEvent.click(back)

    // Mirrors the mount autofocus: returning to unlock lands on the password input.
    expect(document.activeElement).toBe(screen.getByLabelText('PASSWORD'))
  })

  test('recover mode recaptures focus that escapes to a control behind the locked gate', () => {
    renderGate()
    fireEvent.click(
      screen.getByRole('button', {
        name: /recover from a saved snapshot instead/i,
      }),
    )
    const dialog = screen.getByRole('dialog', {
      name: /restore from a snapshot/i,
    })

    // A focusable control "behind" the locked gate (e.g. a sidebar button in the
    // still-focusable shell) steals focus.
    const behind = document.createElement('button')
    behind.textContent = 'sidebar'
    document.body.appendChild(behind)
    behind.focus()

    // The recapture guard pulls focus straight back inside the dialog, so it can
    // never rest on a control behind the locked gate.
    expect(document.activeElement).not.toBe(behind)
    expect(dialog.contains(document.activeElement)).toBe(true)

    document.body.removeChild(behind)
  })
})
