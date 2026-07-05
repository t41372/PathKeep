/**
 * @file recovery-panel.test.tsx
 * @description Behavioral coverage for the shared SnapshotRecoveryPanel.
 * @module components/snapshot-restore
 *
 * ## What this suite covers
 * - Loads the snapshot list; loading / error+retry / empty states.
 * - Encrypted snapshot → confirm shows the ArchiveKeyField; a typed key threads into the
 *   restore call; a successful restore surfaces the success notice, reloads, and calls onRestored.
 * - A wrong-key rejection surfaces an honest role="alert" error and keeps the user in the flow.
 * - A persistent "Reveal logs" forward path is present in EVERY state (never a dead end).
 * - initialKey prefills the key field.
 * - A plaintext snapshot confirm shows no key field and restores with a null key.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { backend } from '../../lib/backend-client'
import type * as BackendClient from '../../lib/backend-client'
import type { RecoverySnapshot } from '../../lib/types'
import { SnapshotRecoveryPanel } from './index'

// ──────────────────────────────────────────────────────────────────────────────
// Mock backend so IPC calls never actually fire.
// ──────────────────────────────────────────────────────────────────────────────
vi.mock('../../lib/backend-client', async (importOriginal) => {
  const actual = await importOriginal<typeof BackendClient>()
  return {
    ...actual,
    backend: {
      ...actual.backend,
      listRecoverySnapshots: vi.fn().mockResolvedValue([]),
      revealLogs: vi.fn().mockResolvedValue(undefined),
    },
  }
})

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

function renderPanel(
  props: Partial<{
    runFullArchiveRestore: (
      snapshotPath: string,
      key?: string | null,
    ) => Promise<unknown>
    onRestored: () => void
    initialKey: string
  }> = {},
) {
  const runFullArchiveRestore =
    props.runFullArchiveRestore ?? vi.fn().mockResolvedValue({})
  return {
    runFullArchiveRestore,
    ...render(
      <I18nProvider>
        <SnapshotRecoveryPanel
          runFullArchiveRestore={
            runFullArchiveRestore as (
              snapshotPath: string,
              key?: string | null,
            ) => Promise<never>
          }
          onRestored={props.onRestored}
          initialKey={props.initialKey}
        />
      </I18nProvider>,
    ),
  }
}

const revealLogsBtn = () =>
  screen.getByRole('button', { name: /open the pathkeep logs/i })

beforeEach(() => {
  vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([])
  vi.mocked(backend.revealLogs).mockResolvedValue('' as never)
})

describe('SnapshotRecoveryPanel', () => {
  test('shows the loading state (with a reveal-logs forward path) while fetching', () => {
    vi.mocked(backend.listRecoverySnapshots).mockReturnValue(
      new Promise(() => {}),
    )
    renderPanel()
    expect(screen.getByText(/loading snapshots/i)).toBeInTheDocument()
    // Never a dead end, even while loading.
    expect(revealLogsBtn()).toBeInTheDocument()
  })

  test('shows a load error with a retry that re-fetches the list', async () => {
    vi.mocked(backend.listRecoverySnapshots)
      .mockRejectedValueOnce(new Error('network fail'))
      .mockResolvedValueOnce([])
    renderPanel()

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(screen.getByText(/failed to load snapshots/i)).toBeInTheDocument()
    expect(revealLogsBtn()).toBeInTheDocument()

    fireEvent.click(
      screen.getByRole('button', { name: /retry loading snapshots/i }),
    )
    await waitFor(() =>
      expect(backend.listRecoverySnapshots).toHaveBeenCalledTimes(2),
    )
  })

  test('shows the empty state (with a reveal-logs forward path) when no snapshots exist', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([])
    renderPanel()
    await waitFor(() =>
      expect(screen.getByText(/no restore points yet/i)).toBeInTheDocument(),
    )
    expect(revealLogsBtn()).toBeInTheDocument()
  })

  test('renders a restore button for each listed snapshot', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      makeSnapshot(),
      makeSnapshot({ id: 'snap-2', path: '/snap2.sqlite' }),
    ])
    renderPanel()
    await waitFor(() =>
      expect(
        screen.getAllByRole('button', { name: /restore from this snapshot/i }),
      ).toHaveLength(2),
    )
  })

  test('encrypted snapshot: confirm shows the key field, threads the key, then succeeds + reloads + notifies', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      makeSnapshot({ encrypted: true }),
    ])
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    const onRestored = vi.fn()
    renderPanel({ runFullArchiveRestore, onRestored })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    // The encrypted snapshot shows the honest "needs your key" badge, not "Verified".
    expect(screen.getByText(/needs your key/i)).toBeInTheDocument()

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )

    const keyInput = screen.getByLabelText('Archive key')
    expect(keyInput).toBeInTheDocument()
    fireEvent.change(keyInput, { target: { value: 'my-secret' } })

    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() =>
      expect(runFullArchiveRestore).toHaveBeenCalledWith(
        '/snap.sqlite',
        'my-secret',
      ),
    )
    // Success notice + reload (2nd list fetch) + onRestored callback.
    expect(await screen.findByText(/restore complete/i)).toBeInTheDocument()
    await waitFor(() =>
      expect(backend.listRecoverySnapshots).toHaveBeenCalledTimes(2),
    )
    expect(onRestored).toHaveBeenCalledTimes(1)
  })

  test('wrong key: surfaces an honest error and keeps the user in the flow', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      makeSnapshot({ encrypted: true }),
    ])
    const runFullArchiveRestore = vi
      .fn()
      .mockRejectedValue(new Error('wrong key'))
    renderPanel({ runFullArchiveRestore })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.change(screen.getByLabelText('Archive key'), {
      target: { value: 'bad-key' },
    })
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(screen.getByText(/restore failed/i)).toBeInTheDocument()
    // Still in the flow: a "Try another snapshot" reset AND a reveal-logs path remain.
    expect(
      screen.getByRole('button', {
        name: /dismiss error and pick another snapshot/i,
      }),
    ).toBeInTheDocument()
    expect(revealLogsBtn()).toBeInTheDocument()
  })

  test('reveal-logs is present in the confirm state and invokes the backend', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    renderPanel()

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    // Confirm step is open — the forward path is still present.
    expect(screen.getByText(/replace archive and restore/i)).toBeInTheDocument()
    fireEvent.click(revealLogsBtn())
    await waitFor(() => expect(backend.revealLogs).toHaveBeenCalled())
  })

  test('initialKey prefills the key field for an encrypted snapshot', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      makeSnapshot({ encrypted: true }),
    ])
    renderPanel({ initialKey: 'prefilled' })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(screen.getByLabelText('Archive key')).toHaveValue('prefilled')
  })

  test('plaintext snapshot: confirm shows no key field and restores with a null key', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    renderPanel({ runFullArchiveRestore })

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(screen.queryByLabelText('Archive key')).toBeNull()

    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )
    await waitFor(() =>
      expect(runFullArchiveRestore).toHaveBeenCalledWith('/snap.sqlite', null),
    )
  })

  test('confirm step for a snapshot with no date uses the date-unknown copy', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      makeSnapshot({ createdAt: null }),
    ])
    renderPanel()

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /restore from this snapshot/i }),
      ).toBeInTheDocument(),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    // confirmBodyDateUnknown contains "selected snapshot".
    expect(screen.getByText(/selected snapshot/i)).toBeInTheDocument()
  })

  test('a successful restore with no onRestored callback still succeeds', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    // No onRestored prop — exercises the optional-chain nullish path.
    renderPanel({ runFullArchiveRestore })

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

    expect(await screen.findByText(/restore complete/i)).toBeInTheDocument()
  })

  test('skips state updates after unmount when the list load resolves (mountedRef guard)', async () => {
    let resolveList!: (val: RecoverySnapshot[]) => void
    vi.mocked(backend.listRecoverySnapshots).mockReturnValue(
      new Promise<RecoverySnapshot[]>((r) => {
        resolveList = r
      }),
    )
    const { unmount } = renderPanel()
    expect(screen.getByText(/loading snapshots/i)).toBeInTheDocument()

    unmount()
    resolveList([makeSnapshot()])
    await new Promise((r) => setTimeout(r, 0))
  })

  test('skips state updates after unmount when the list load rejects (mountedRef guard)', async () => {
    let rejectList!: (err: Error) => void
    vi.mocked(backend.listRecoverySnapshots).mockReturnValue(
      new Promise<RecoverySnapshot[]>((_, r) => {
        rejectList = r
      }),
    )
    const { unmount } = renderPanel()
    expect(screen.getByText(/loading snapshots/i)).toBeInTheDocument()

    // Unmount while the fetch is in flight, then reject — the mountedRef guard
    // must suppress the setLoadError call (no unmounted-update warning/crash).
    unmount()
    rejectList(new Error('network fail'))
    await new Promise((r) => setTimeout(r, 0))
  })

  test('Cancel in the confirm step returns to the snapshot list', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    renderPanel()

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
    expect(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    ).toBeInTheDocument()
  })

  // ── Focus containment across internal transitions ────────────────────────────
  // Every transition unmounts the focused control; without deterministic focus
  // moves, focus falls to document.body and the next Tab escapes the modal onto a
  // control behind the locked gate. These assert focus lands on a specific
  // in-dialog element instead (they fail on a panel with no focus management).

  test('focus: "Restore this" (plaintext) moves focus to the confirm button, not the body', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    renderPanel()
    const restoreBtn = await screen.findByRole('button', {
      name: /restore from this snapshot/i,
    })
    restoreBtn.focus()
    fireEvent.click(restoreBtn)
    const confirmBtn = screen.getByRole('button', {
      name: /confirm and restore from this snapshot/i,
    })
    expect(document.activeElement).toBe(confirmBtn)
    expect(document.activeElement).not.toBe(document.body)
  })

  test('focus: "Restore this" (encrypted) moves focus to the key field', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      makeSnapshot({ encrypted: true }),
    ])
    renderPanel()
    const restoreBtn = await screen.findByRole('button', {
      name: /restore from this snapshot/i,
    })
    restoreBtn.focus()
    fireEvent.click(restoreBtn)
    expect(document.activeElement).toBe(screen.getByLabelText('Archive key'))
  })

  test('focus: Cancel returns focus to a snapshot restore button, not the body', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    renderPanel()
    fireEvent.click(
      await screen.findByRole('button', {
        name: /restore from this snapshot/i,
      }),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /cancel restore and go back/i }),
    )
    const listRestoreBtn = screen.getByRole('button', {
      name: /restore from this snapshot/i,
    })
    expect(document.activeElement).toBe(listRestoreBtn)
    expect(document.activeElement).not.toBe(document.body)
  })

  test('focus: an error focuses "Try another snapshot", then dismissing returns focus to the list', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([
      makeSnapshot({ encrypted: true }),
    ])
    const runFullArchiveRestore = vi
      .fn()
      .mockRejectedValue(new Error('wrong key'))
    renderPanel({ runFullArchiveRestore })
    fireEvent.click(
      await screen.findByRole('button', {
        name: /restore from this snapshot/i,
      }),
    )
    fireEvent.change(screen.getByLabelText('Archive key'), {
      target: { value: 'bad-key' },
    })
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )
    const retryBtn = await screen.findByRole('button', {
      name: /dismiss error and pick another snapshot/i,
    })
    expect(document.activeElement).toBe(retryBtn)
    fireEvent.click(retryBtn)
    const listRestoreBtn = screen.getByRole('button', {
      name: /restore from this snapshot/i,
    })
    expect(document.activeElement).toBe(listRestoreBtn)
    expect(document.activeElement).not.toBe(document.body)
  })

  test('focus: entering the restoring state moves focus to the status region', async () => {
    vi.mocked(backend.listRecoverySnapshots).mockResolvedValue([makeSnapshot()])
    // Never resolves — hold the panel in the restoring state.
    const runFullArchiveRestore = vi.fn().mockReturnValue(new Promise(() => {}))
    renderPanel({ runFullArchiveRestore })
    fireEvent.click(
      await screen.findByRole('button', {
        name: /restore from this snapshot/i,
      }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )
    await waitFor(() => expect(screen.getByRole('status')).toHaveFocus())
  })
})
