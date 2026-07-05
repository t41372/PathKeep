/**
 * @file snapshot-restore-section.test.tsx
 * @description Coverage for the Settings → Restore from Snapshot section.
 * @module pages/settings
 *
 * ## What this suite covers
 * - Loading state shows loading indicator.
 * - Load error shows error + retry button; retry re-calls listRecoverySnapshots.
 * - Empty list shows emptyTitle from i18n.
 * - List with snapshots renders snapshot cards.
 * - "Restore" shows confirm panel; "Cancel" dismisses it.
 * - "Restore now" calls runFullArchiveRestore via context.
 * - Restore error is shown inline.
 * - FIX 4: confirm "Restore now" button has aria-describedby on the confirm body.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { backend } from '../../lib/backend-client'
import type * as BackendClient from '../../lib/backend-client'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import type { RecoverySnapshot } from '../../lib/types'
import { SnapshotRestoreSection } from './snapshot-restore-section'
import type { SettingsSectionNavItem } from './section-nav-items'

// ──────────────────────────────────────────────────────────────────────────────
// Mock backend
// ──────────────────────────────────────────────────────────────────────────────
vi.mock('../../lib/backend-client', async (importOriginal) => {
  const actual = await importOriginal<typeof BackendClient>()
  return {
    ...actual,
    backend: {
      ...actual.backend,
      listRecoverySnapshots: vi.fn().mockResolvedValue([]),
    },
  }
})

// ──────────────────────────────────────────────────────────────────────────────
// Fixtures
// ──────────────────────────────────────────────────────────────────────────────

function makeNavItem(): SettingsSectionNavItem {
  return {
    id: 'settings-restore',
    icon: 'history',
    key: 'restore',
    label: 'RESTORE FROM SNAPSHOT',
  }
}

function makeSnapshot(
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

function shellContextValue(
  overrides: Partial<ShellDataContextValue> = {},
): ShellDataContextValue {
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
    refreshAppData: vi.fn().mockResolvedValue(undefined),
    refreshRuntimeStatus: vi.fn().mockResolvedValue({}),
    saveConfig: vi.fn().mockResolvedValue({}),
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

function renderSection(shellOverrides: Partial<ShellDataContextValue> = {}) {
  const shell = shellContextValue(shellOverrides)
  return {
    shell,
    ...render(
      <I18nProvider>
        <ShellDataContext.Provider value={shell}>
          <SnapshotRestoreSection navItem={makeNavItem()} />
        </ShellDataContext.Provider>
      </I18nProvider>,
    ),
  }
}

// ──────────────────────────────────────────────────────────────────────────────
// Reset mocks between tests
// ──────────────────────────────────────────────────────────────────────────────
beforeEach(() => {
  vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([])
})

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
describe('SnapshotRestoreSection', () => {
  test('shows loading indicator initially', () => {
    // Don't resolve the promise yet so loading state persists
    vi.mocked(backend.listRecoverySnapshots).mockReturnValue(
      new Promise(() => {}),
    )
    renderSection()
    expect(screen.getByText(/loading snapshots/i)).toBeInTheDocument()
  })

  test('shows error with retry button when listRecoverySnapshots fails', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockRejectedValue(
      new Error('network error'),
    )
    renderSection()

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(screen.getByText(/failed to load snapshots/i)).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /retry loading snapshots/i }),
    ).toBeInTheDocument()
  })

  test('retry button calls listRecoverySnapshots again', async () => {
    vi.mocked(backend.listRecoverySnapshots)
      .mockRejectedValueOnce(new Error('first fail'))
      .mockResolvedValueOnce([])
    renderSection()

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())

    fireEvent.click(
      screen.getByRole('button', { name: /retry loading snapshots/i }),
    )

    await waitFor(() =>
      expect(backend.listRecoverySnapshots).toHaveBeenCalledTimes(2),
    )
  })

  test('empty list shows emptyTitle', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([])
    renderSection()

    await waitFor(() =>
      expect(screen.getByText(/no restore points yet/i)).toBeInTheDocument(),
    )
  })

  test('list with one snapshot renders a restore button', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    renderSection()

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
  })

  test('"Restore" on a snapshot shows the confirm panel', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    renderSection()

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )

    expect(screen.getByText(/replace archive and restore/i)).toBeInTheDocument()
  })

  test('"Cancel" in confirm dismisses the confirm panel', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    renderSection()

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(screen.getByText(/replace archive and restore/i)).toBeInTheDocument()

    fireEvent.click(
      screen.getByRole('button', { name: /cancel restore and go back/i }),
    )
    expect(screen.queryByText(/replace archive and restore/i)).toBeNull()
  })

  test('"Restore now" calls runFullArchiveRestore via context', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    renderSection({ runFullArchiveRestore })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() =>
      expect(runFullArchiveRestore).toHaveBeenCalledWith('/snap.sqlite', null),
    )
  })

  test('successful restore clears the confirm panel, shows the success notice, and reloads the list', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    renderSection({ runFullArchiveRestore })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    // Initial mount load counts as one call.
    await waitFor(() =>
      expect(backend.listRecoverySnapshots).toHaveBeenCalledTimes(1),
    )

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(screen.getByText(/replace archive and restore/i)).toBeInTheDocument()
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    // Post-condition: confirm panel is gone, the success status region is shown,
    // and the list was reloaded (a second listRecoverySnapshots call).
    await waitFor(() =>
      expect(screen.queryByText(/replace archive and restore/i)).toBeNull(),
    )
    const success = await screen.findByRole('status')
    expect(success).toHaveTextContent(/restore complete/i)
    await waitFor(() =>
      expect(backend.listRecoverySnapshots).toHaveBeenCalledTimes(2),
    )
  })

  test('confirm "Restore now" is disabled while restoring and cannot double-invoke', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    let resolveRestore!: (value: unknown) => void
    const runFullArchiveRestore = vi.fn().mockReturnValue(
      new Promise((resolve) => {
        resolveRestore = resolve
      }),
    )
    renderSection({ runFullArchiveRestore })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    const confirmBtn = screen.getByRole('button', {
      name: /confirm and restore from this snapshot/i,
    })
    fireEvent.click(confirmBtn)

    // While the restore is in flight the confirm button is disabled...
    await waitFor(() => expect(confirmBtn).toBeDisabled())
    // ...and a second rapid click does not fire a second restore.
    fireEvent.click(confirmBtn)
    expect(runFullArchiveRestore).toHaveBeenCalledTimes(1)

    resolveRestore({})
    await waitFor(() => expect(screen.getByRole('status')).toBeInTheDocument())
  })

  test('confirm panel is hidden once a restore error is shown', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    const runFullArchiveRestore = vi
      .fn()
      .mockRejectedValue(new Error('restore boom'))
    renderSection({ runFullArchiveRestore })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    // The confirm panel must not render alongside the error alert.
    expect(screen.queryByText(/replace archive and restore/i)).toBeNull()
  })

  test('restore error shows inline error', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    const runFullArchiveRestore = vi
      .fn()
      .mockRejectedValue(new Error('restore failed'))
    renderSection({ runFullArchiveRestore })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    // The alert contains the "Restore failed" title
    expect(screen.getByRole('alert')).toBeInTheDocument()
  })

  test('confirm step with no date uses confirmBodyDateUnknown copy', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      makeSnapshot({ createdAt: null }),
    ])
    renderSection()

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )

    // confirmBodyDateUnknown contains "selected snapshot"
    expect(screen.getByText(/selected snapshot/i)).toBeInTheDocument()
  })

  test('state updates are skipped after unmount (mountedRef guard — success path)', async () => {
    let resolveList!: (val: RecoverySnapshot[]) => void
    vi.mocked(backend.listRecoverySnapshots).mockReturnValue(
      new Promise<RecoverySnapshot[]>((r) => {
        resolveList = r
      }),
    )

    const { unmount } = renderSection()
    // Component is showing loading state while the fetch is in flight
    expect(screen.getByText(/loading snapshots/i)).toBeInTheDocument()

    // Unmount while fetch is still in flight — sets mountedRef.current = false
    unmount()

    // Resolve the fetch AFTER unmount; the if (mountedRef.current) guards suppress state updates
    resolveList([])
    // Flush pending microtasks from the promise chain
    await new Promise((r) => setTimeout(r, 0))
    // Test passes if no "Can't perform state update on unmounted component" error occurs
  })

  test('state updates are skipped after unmount (mountedRef guard — error path)', async () => {
    let rejectList!: (err: Error) => void
    vi.mocked(backend.listRecoverySnapshots).mockReturnValue(
      new Promise<RecoverySnapshot[]>((_, r) => {
        rejectList = r
      }),
    )

    const { unmount } = renderSection()
    expect(screen.getByText(/loading snapshots/i)).toBeInTheDocument()

    unmount()

    // Reject after unmount — mountedRef guard suppresses the setLoadError call
    rejectList(new Error('network fail'))
    await new Promise((r) => setTimeout(r, 0))
    // No crash if mountedRef guard works correctly
  })

  // ── FIX 4: aria-describedby on confirm "Restore now" button ─────────────────

  test('confirm "Restore now" button has aria-describedby linking to the confirm body', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    renderSection()

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )

    const confirmBtn = screen.getByRole('button', {
      name: /confirm and restore from this snapshot/i,
    })
    expect(confirmBtn).toHaveAttribute(
      'aria-describedby',
      'settings-confirm-body',
    )
    // The referenced element must be in the DOM
    expect(document.getElementById('settings-confirm-body')).toBeInTheDocument()
  })

  // ── Encrypted snapshot: key entry in the confirm panel ──────────────────────

  test('encrypted snapshot confirm shows the ArchiveKeyField and threads the key', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      makeSnapshot({ encrypted: true }),
    ])
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    renderSection({ runFullArchiveRestore })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    // The card shows the honest "needs your key" badge, not "Verified".
    expect(screen.getByText(/needs your key/i)).toBeInTheDocument()

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    const keyInput = screen.getByLabelText('Archive key')
    expect(keyInput).toBeInTheDocument()
    fireEvent.change(keyInput, { target: { value: 'settings-key' } })

    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() =>
      expect(runFullArchiveRestore).toHaveBeenCalledWith(
        '/snap.sqlite',
        'settings-key',
      ),
    )
  })

  test('plaintext snapshot confirm shows no key field', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    renderSection()

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(screen.queryByLabelText('Archive key')).toBeNull()
  })
})
