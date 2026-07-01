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
import { ArchiveKeyField, SnapshotCard, SnapshotRestoreList } from './index'
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
    encrypted: false,
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
    encryptedNeedsKeyBadge: 'Encrypted · needs your key',
    keyFieldLabel: 'Archive key',
    keyFieldPlaceholder: 'Enter your archive key',
    keyFieldHint:
      'This snapshot is encrypted. Enter your archive key so PathKeep can verify and restore it. A wrong key fails safely — nothing is changed.',
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

  test('encrypted snapshot shows the honest "needs your key" badge, never the green "Verified"', () => {
    renderWithI18n(
      <SnapshotCard
        // verifiedOpenable is only a size heuristic for encrypted snapshots, so
        // even when true the card must NOT claim the snapshot is verified.
        snap={makeSnapshot({ encrypted: true, verifiedOpenable: true })}
        onRestore={vi.fn()}
        busy={false}
        t={t}
      />,
    )
    expect(screen.getByText(/needs your key/i)).toBeInTheDocument()
    expect(screen.queryByText('Verified')).toBeNull()
  })
})

describe('ArchiveKeyField', () => {
  const t = makeTranslator()

  test('renders the label, placeholder, and hint', () => {
    renderWithI18n(<ArchiveKeyField id="k" value="" onChange={vi.fn()} t={t} />)
    expect(screen.getByText('Archive key')).toBeInTheDocument()
    const input = screen.getByLabelText('Archive key')
    expect(input).toHaveAttribute('placeholder', 'Enter your archive key')
    expect(input).toHaveAttribute('type', 'password')
    expect(screen.getByText(/fails safely/i)).toBeInTheDocument()
  })

  test('onChange fires with the typed value', () => {
    const onChange = vi.fn()
    renderWithI18n(
      <ArchiveKeyField id="k" value="" onChange={onChange} t={t} />,
    )
    fireEvent.change(screen.getByLabelText('Archive key'), {
      target: { value: 'secret' },
    })
    expect(onChange).toHaveBeenCalledWith('secret')
  })

  test('honors the disabled prop', () => {
    renderWithI18n(
      <ArchiveKeyField id="k" value="x" onChange={vi.fn()} t={t} disabled />,
    )
    expect(screen.getByLabelText('Archive key')).toBeDisabled()
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

  test('initialKey seeds the archiveKey state', () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore, initialKey: 'seeded' }),
    )
    expect(result.current.archiveKey).toBe('seeded')
  })

  test('setArchiveKey updates the archiveKey state', () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore }),
    )
    expect(result.current.archiveKey).toBe('')
    act(() => {
      result.current.setArchiveKey('typed')
    })
    expect(result.current.archiveKey).toBe('typed')
  })

  test('confirmRestore passes the trimmed key and clears it on success', async () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore }),
    )
    const snap = makeSnapshot({ encrypted: true })

    act(() => {
      result.current.setArchiveKey('  my-key  ')
    })
    await act(async () => {
      await result.current.confirmRestore(snap)
    })

    expect(runFullArchiveRestore).toHaveBeenCalledWith(snap.path, 'my-key')
    // The key is never kept past a successful restore.
    expect(result.current.archiveKey).toBe('')
  })

  test('confirmRestore passes null when the key field is blank', async () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore }),
    )
    const snap = makeSnapshot()

    act(() => {
      result.current.setArchiveKey('   ')
    })
    await act(async () => {
      await result.current.confirmRestore(snap)
    })

    expect(runFullArchiveRestore).toHaveBeenCalledWith(snap.path, null)
  })

  test('confirmRestore keeps confirming AND the key on error so the user can retry', async () => {
    const runFullArchiveRestore = vi
      .fn()
      .mockRejectedValue(new Error('wrong key'))
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore }),
    )
    const snap = makeSnapshot({ encrypted: true })

    act(() => {
      result.current.startRestore(snap)
      result.current.setArchiveKey('bad-key')
    })
    await act(async () => {
      await result.current.confirmRestore(snap)
    })

    expect(result.current.restoreError).not.toBeNull()
    // Both the snapshot AND the entered key survive so the user can correct and retry.
    expect(result.current.confirming).toEqual(snap)
    expect(result.current.archiveKey).toBe('bad-key')
  })

  test('cancelRestore and resetError both clear the archiveKey', () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore }),
    )

    act(() => {
      result.current.setArchiveKey('key-a')
      result.current.cancelRestore()
    })
    expect(result.current.archiveKey).toBe('')

    act(() => {
      result.current.setArchiveKey('key-b')
      result.current.resetError()
    })
    expect(result.current.archiveKey).toBe('')
  })

  test('startRestore does NOT clear a key the user already typed', () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    const { result } = renderHook(() =>
      useSnapshotRestore({ runFullArchiveRestore }),
    )
    act(() => {
      result.current.setArchiveKey('carry-me')
      result.current.startRestore(makeSnapshot({ encrypted: true }))
    })
    expect(result.current.archiveKey).toBe('carry-me')
  })
})
