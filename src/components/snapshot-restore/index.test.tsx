/**
 * @file index.test.tsx
 * @description Coverage for SnapshotCard and SnapshotRestoreList components.
 * @module components/snapshot-restore
 *
 * ## What this suite covers
 * - SnapshotCard renders date, size, badge (verified/unverified), sourceOp.
 * - SnapshotCard "Restore" button fires callback.
 * - SnapshotRestoreList renders loading, error, empty, and list states.
 * - SnapshotRestoreList onRestore propagates to item callbacks.
 * - sourceOpLabel maps all backend KNOWN_OPS correctly.
 * - useSnapshotRestore state machine: confirm, re-entry guard, failure, resets.
 */

import {
  render,
  screen,
  fireEvent,
  renderHook,
  act,
} from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { RecoverySnapshot } from '../../lib/types'
import { SnapshotCard, SnapshotRestoreList } from './index'
import { useSnapshotRestore } from './use-snapshot-restore'

function makeSnapshot(
  overrides: Partial<RecoverySnapshot> = {},
): RecoverySnapshot {
  return {
    id: 'snap-1',
    path: '/snap.sqlite',
    createdAt: '2026-06-01T10:00:00Z',
    sizeBytes: 1024 * 1024,
    verifiedOpenable: true,
    sourceOp: 'rekey',
    label: 'Encryption change',
    ...overrides,
  }
}

// Simple translator that returns the key
function makeTranslator() {
  const translations: Record<string, string> = {
    snapshotDate: 'Created {date}',
    snapshotDateUnknown: 'Date unknown',
    snapshotSize: '{size}',
    verifiedBadge: 'Verified',
    notVerifiedBadge: 'Not verified',
    restoreThis: 'Restore this',
    restoreThisAria: 'Restore from this snapshot',
    'sourceOp.rekey': 'Encryption change',
    'sourceOp.reconcile': 'Encryption maintenance',
    'sourceOp.import': 'Archive import',
    'sourceOp.periodic': 'Periodic snapshot',
    'sourceOp.unknown': 'Automatic snapshot',
    loadingSnapshots: 'Loading snapshots…',
    loadingSnapshotsAria: 'Loading available snapshots',
    loadError: 'Failed to load snapshots',
    loadErrorAria: 'Error loading snapshot list',
    emptyTitle: 'No restore points yet',
    emptyBody: 'PathKeep captures snapshots before rewrites.',
  }
  return (key: string) => translations[key] ?? key
}

function renderWithI18n(ui: React.ReactElement) {
  return render(<I18nProvider>{ui}</I18nProvider>)
}

describe('SnapshotCard', () => {
  const t = makeTranslator()

  test('renders verified badge and date for a verified snapshot', () => {
    renderWithI18n(
      <SnapshotCard
        snap={makeSnapshot()}
        onRestore={vi.fn()}
        busy={false}
        t={t}
      />,
    )
    expect(screen.getByText(/verified/i)).toBeInTheDocument()
    expect(screen.getByText(/created/i)).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    ).toBeInTheDocument()
  })

  test('renders "Not verified" badge for unverified snapshot', () => {
    renderWithI18n(
      <SnapshotCard
        snap={makeSnapshot({ verifiedOpenable: false })}
        onRestore={vi.fn()}
        busy={false}
        t={t}
      />,
    )
    expect(screen.getByText(/not verified/i)).toBeInTheDocument()
  })

  test('shows "Date unknown" when createdAt is null', () => {
    renderWithI18n(
      <SnapshotCard
        snap={makeSnapshot({ createdAt: null })}
        onRestore={vi.fn()}
        busy={false}
        t={t}
      />,
    )
    expect(screen.getByText(/date unknown/i)).toBeInTheDocument()
  })

  test('"Restore" button calls onRestore', () => {
    const onRestore = vi.fn()
    renderWithI18n(
      <SnapshotCard
        snap={makeSnapshot()}
        onRestore={onRestore}
        busy={false}
        t={t}
      />,
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(onRestore).toHaveBeenCalledTimes(1)
  })

  test('button is disabled when busy', () => {
    renderWithI18n(
      <SnapshotCard
        snap={makeSnapshot()}
        onRestore={vi.fn()}
        busy={true}
        t={t}
      />,
    )
    const btn = screen.getByRole('button', {
      name: /restore from this snapshot/i,
    })
    expect(btn).toBeDisabled()
  })

  test('renders known sourceOp labels — all backend KNOWN_OPS', () => {
    const ops = ['rekey', 'reconcile', 'import', 'periodic'] as const
    for (const op of ops) {
      const { unmount } = renderWithI18n(
        <SnapshotCard
          snap={makeSnapshot({ sourceOp: op })}
          onRestore={vi.fn()}
          busy={false}
          t={t}
        />,
      )
      expect(screen.getByText(t(`sourceOp.${op}`))).toBeInTheDocument()
      unmount()
    }
  })

  test('renders "Automatic snapshot" for unknown sourceOp', () => {
    renderWithI18n(
      <SnapshotCard
        snap={makeSnapshot({ sourceOp: 'mystery' })}
        onRestore={vi.fn()}
        busy={false}
        t={t}
      />,
    )
    expect(screen.getByText(/automatic snapshot/i)).toBeInTheDocument()
  })
})

describe('SnapshotRestoreList', () => {
  test('shows loading indicator when loading', () => {
    renderWithI18n(
      <SnapshotRestoreList
        snapshots={[]}
        loading={true}
        error={null}
        onRestore={vi.fn()}
        busy={false}
      />,
    )
    expect(screen.getByText(/loading snapshots/i)).toBeInTheDocument()
  })

  test('shows error when error is set and not loading', () => {
    renderWithI18n(
      <SnapshotRestoreList
        snapshots={[]}
        loading={false}
        error="network error"
        onRestore={vi.fn()}
        busy={false}
      />,
    )
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(screen.getByText(/failed to load snapshots/i)).toBeInTheDocument()
  })

  test('shows empty state when snapshot list is empty', () => {
    renderWithI18n(
      <SnapshotRestoreList
        snapshots={[]}
        loading={false}
        error={null}
        onRestore={vi.fn()}
        busy={false}
      />,
    )
    expect(screen.getByText(/no restore points yet/i)).toBeInTheDocument()
  })

  test('renders snapshot cards when snapshots are provided', () => {
    renderWithI18n(
      <SnapshotRestoreList
        snapshots={[makeSnapshot(), makeSnapshot({ id: 'snap-2' })]}
        loading={false}
        error={null}
        onRestore={vi.fn()}
        busy={false}
      />,
    )
    const restoreButtons = screen.getAllByRole('button', {
      name: /restore from this snapshot/i,
    })
    expect(restoreButtons).toHaveLength(2)
  })

  test('calls onRestore with the correct snapshot', () => {
    const snap = makeSnapshot()
    const onRestore = vi.fn()
    renderWithI18n(
      <SnapshotRestoreList
        snapshots={[snap]}
        loading={false}
        error={null}
        onRestore={onRestore}
        busy={false}
      />,
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(onRestore).toHaveBeenCalledWith(snap)
  })
})

describe('useSnapshotRestore', () => {
  test('confirmRestore success clears restoring/confirming and flags success', async () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    const onSuccess = vi.fn()
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore, onSuccess }),
    )
    const snap = makeSnapshot()

    act(() => {
      result.current.startRestore(snap)
    })
    expect(result.current.confirming).toEqual(snap)
    expect(result.current.restoreSucceeded).toBe(false)

    await act(async () => {
      await result.current.confirmRestore(snap)
    })

    expect(runFullArchiveRestore).toHaveBeenCalledTimes(1)
    expect(result.current.restoring).toBe(false)
    expect(result.current.confirming).toBeNull()
    expect(result.current.restoreSucceeded).toBe(true)
    expect(onSuccess).toHaveBeenCalledTimes(1)
  })

  test('confirmRestore re-entry guard: a second call while restoring is a no-op', async () => {
    let resolveRestore!: (value: unknown) => void
    const runFullArchiveRestore = vi.fn().mockReturnValue(
      new Promise((resolve) => {
        resolveRestore = resolve
      }),
    )
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore }),
    )
    const snap = makeSnapshot()

    // First call starts the (still pending) restore.
    act(() => {
      void result.current.confirmRestore(snap)
    })
    expect(result.current.restoring).toBe(true)
    expect(runFullArchiveRestore).toHaveBeenCalledTimes(1)

    // Second call while a restore is in flight hits the guard and returns immediately.
    await act(async () => {
      await result.current.confirmRestore(snap)
    })
    expect(runFullArchiveRestore).toHaveBeenCalledTimes(1)

    await act(async () => {
      resolveRestore({})
      await Promise.resolve()
    })
    expect(result.current.restoring).toBe(false)
    expect(result.current.restoreSucceeded).toBe(true)
  })

  test('confirmRestore failure surfaces the error and does not flag success', async () => {
    const runFullArchiveRestore = vi
      .fn()
      .mockRejectedValue(new Error('restore boom'))
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore }),
    )
    const snap = makeSnapshot()

    await act(async () => {
      await result.current.confirmRestore(snap)
    })

    expect(result.current.restoring).toBe(false)
    expect(result.current.restoreSucceeded).toBe(false)
    expect(result.current.restoreError).not.toBeNull()
  })

  test('restoreSucceeded resets on startRestore, cancelRestore, and resetError', async () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore }),
    )
    const snap = makeSnapshot()

    const succeed = async () => {
      await act(async () => {
        await result.current.confirmRestore(snap)
      })
      expect(result.current.restoreSucceeded).toBe(true)
    }

    await succeed()
    act(() => {
      result.current.startRestore(snap)
    })
    expect(result.current.restoreSucceeded).toBe(false)

    await succeed()
    act(() => {
      result.current.cancelRestore()
    })
    expect(result.current.restoreSucceeded).toBe(false)

    await succeed()
    act(() => {
      result.current.resetError()
    })
    expect(result.current.restoreSucceeded).toBe(false)
  })
})
