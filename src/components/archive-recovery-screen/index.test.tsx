/**
 * @file index.test.tsx
 * @description Coverage for the ArchiveRecoveryScreen full-screen recovery gate.
 * @module components/archive-recovery-screen
 *
 * ## What this suite covers
 * - Screen renders with verified snapshots (title + headline card visible).
 * - "See all snapshots" expands the list; "Hide list" collapses it.
 * - "Restore this" shows the confirm step.
 * - "Cancel" in confirm dismisses the confirm panel.
 * - Escape key during confirm dismisses the confirm panel.
 * - "Restore now" calls `runFullArchiveRestore` with the correct path.
 * - Restore error surface: error shown + "Try another snapshot" resets state.
 * - Empty state: emptyTitle + emptyReassurance shown, "Reveal logs" calls backend.
 * - Focus trap: Tab cycles within dialog.
 * - Focus is restored to prior element on unmount.
 * - bodyUnverifiedOnly shown when snapshots exist but none are verified.
 * - FIX 3: reconcile / periodic sourceOp labels are shown correctly.
 * - FIX 4: confirm "Restore now" button has aria-describedby linking to the confirm body.
 * - FIX 5: entering restoring state moves focus to the status region.
 * - FIX 8: expand button shows "See all snapshots" without a count.
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
import type { ArchiveRecoveryReport, RecoverySnapshot } from '../../lib/types'
import { ArchiveRecoveryScreen } from './index'

// ──────────────────────────────────────────────────────────────────────────────
// Mock backend so IPC calls never actually fire.
// ──────────────────────────────────────────────────────────────────────────────
vi.mock('../../lib/backend-client', async (importOriginal) => {
  const actual = await importOriginal<typeof BackendClient>()
  return {
    ...actual,
    backend: {
      ...actual.backend,
      revealLogs: vi.fn().mockResolvedValue(undefined),
    },
  }
})

// ──────────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ──────────────────────────────────────────────────────────────────────────────

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

function makeReport(
  overrides: Partial<ArchiveRecoveryReport> = {},
): ArchiveRecoveryReport {
  return {
    kind: 'atRestDriftUnresolved',
    configMode: 'Plaintext',
    availableSnapshots: ['/snap.sqlite'],
    recoverySnapshots: [makeSnapshot()],
    detail: 'test detail',
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

function renderScreen(
  report: ArchiveRecoveryReport = makeReport(),
  shellOverrides: Partial<ShellDataContextValue> = {},
) {
  const shell = shellContextValue(shellOverrides)
  return {
    shell,
    ...render(
      <I18nProvider>
        <ShellDataContext.Provider value={shell}>
          <ArchiveRecoveryScreen report={report} />
        </ShellDataContext.Provider>
      </I18nProvider>,
    ),
  }
}

// ──────────────────────────────────────────────────────────────────────────────
// Reset mocks between tests
// ──────────────────────────────────────────────────────────────────────────────
beforeEach(() => {
  vi.mocked(backend.revealLogs).mockResolvedValue(undefined as never)
})

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
describe('ArchiveRecoveryScreen', () => {
  test('renders with verified snapshots — shows title and Restore this button', () => {
    renderScreen()
    expect(
      screen.getByRole('dialog', { name: /restore from snapshot/i }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    ).toBeInTheDocument()
  })

  test('"See all snapshots" button expands the list', () => {
    const snap2 = makeSnapshot({ id: 'snap-2', path: '/snap2.sqlite' })
    const report = makeReport({
      recoverySnapshots: [makeSnapshot(), snap2],
    })
    renderScreen(report)

    const seeAll = screen.getByRole('button', { name: /show all available/i })
    expect(seeAll).toBeInTheDocument()
    fireEvent.click(seeAll)
    // After expansion, the second snapshot row should appear (snap-2 is in remaining list)
    const restoreButtons = screen.getAllByRole('button', {
      name: /restore from this snapshot/i,
    })
    expect(restoreButtons.length).toBeGreaterThanOrEqual(2)
  })

  test('"Hide snapshot list" button collapses the list', () => {
    const snap2 = makeSnapshot({ id: 'snap-2', path: '/snap2.sqlite' })
    const report = makeReport({
      recoverySnapshots: [makeSnapshot(), snap2],
    })
    renderScreen(report)

    // Expand first
    fireEvent.click(screen.getByRole('button', { name: /show all available/i }))
    // Now collapse
    fireEvent.click(
      screen.getByRole('button', { name: /collapse snapshot list/i }),
    )
    // Back to one restore button
    const restoreButtons = screen.getAllByRole('button', {
      name: /restore from this snapshot/i,
    })
    expect(restoreButtons.length).toBe(1)
  })

  test('clicking "Restore this" shows the confirm step', () => {
    renderScreen()
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(screen.getByText(/replace archive and restore/i)).toBeInTheDocument()
  })

  test('"Cancel" in confirm step dismisses it', () => {
    renderScreen()
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(screen.getByText(/replace archive and restore/i)).toBeInTheDocument()
    fireEvent.click(
      screen.getByRole('button', { name: /cancel restore and go back/i }),
    )
    expect(screen.queryByText(/replace archive and restore/i)).toBeNull()
  })

  test('Escape key during confirm dismisses the confirm step', () => {
    renderScreen()
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(screen.getByText(/replace archive and restore/i)).toBeInTheDocument()
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })
    expect(screen.queryByText(/replace archive and restore/i)).toBeNull()
  })

  test('Escape key outside confirm does nothing (screen stays)', () => {
    renderScreen()
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' })
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })

  test('"Restore now" calls runFullArchiveRestore with the snapshot path', async () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    renderScreen(makeReport(), { runFullArchiveRestore })

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

  test('runFullArchiveRestore rejection shows restoreError and "Try another snapshot"', async () => {
    const runFullArchiveRestore = vi
      .fn()
      .mockRejectedValue(new Error('restore boom'))
    renderScreen(makeReport(), { runFullArchiveRestore })

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() =>
      expect(
        screen.getByRole('button', {
          name: /dismiss error and pick another snapshot/i,
        }),
      ).toBeInTheDocument(),
    )
    expect(screen.getByRole('alert')).toBeInTheDocument()
  })

  test('"Try another snapshot" resets error and confirm state', async () => {
    const runFullArchiveRestore = vi
      .fn()
      .mockRejectedValue(new Error('restore boom'))
    renderScreen(makeReport(), { runFullArchiveRestore })

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() =>
      expect(
        screen.getByRole('button', {
          name: /dismiss error and pick another snapshot/i,
        }),
      ).toBeInTheDocument(),
    )

    fireEvent.click(
      screen.getByRole('button', {
        name: /dismiss error and pick another snapshot/i,
      }),
    )

    expect(screen.queryByRole('alert')).toBeNull()
    // Headline card with restore button is visible again
    expect(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    ).toBeInTheDocument()
  })

  test('empty state shows emptyTitle and "Reveal logs" button', () => {
    renderScreen(makeReport({ recoverySnapshots: [], availableSnapshots: [] }))
    expect(screen.getByText(/no restore points yet/i)).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /open the pathkeep logs/i }),
    ).toBeInTheDocument()
  })

  test('"Reveal logs" calls backend.revealLogs', async () => {
    renderScreen(makeReport({ recoverySnapshots: [], availableSnapshots: [] }))
    fireEvent.click(
      screen.getByRole('button', { name: /open the pathkeep logs/i }),
    )
    await waitFor(() => expect(backend.revealLogs).toHaveBeenCalled())
  })

  test('focus trap: Tab from the last element wraps to the first', () => {
    renderScreen()
    const dialog = screen.getByRole('dialog')
    const buttons = Array.from(
      dialog.querySelectorAll<HTMLElement>('button,input,a[href],[tabindex]'),
    ).filter((el) => !el.hasAttribute('disabled') && el.tabIndex >= 0)

    const first = buttons[0]
    const last = buttons[buttons.length - 1]

    last.focus()
    expect(document.activeElement).toBe(last)
    fireEvent.keyDown(last, { key: 'Tab' })
    expect(document.activeElement).toBe(first)
  })

  test('focus trap: Shift+Tab from the first element wraps to the last', () => {
    renderScreen()
    const dialog = screen.getByRole('dialog')
    const buttons = Array.from(
      dialog.querySelectorAll<HTMLElement>('button,input,a[href],[tabindex]'),
    ).filter((el) => !el.hasAttribute('disabled') && el.tabIndex >= 0)

    const first = buttons[0]
    const last = buttons[buttons.length - 1]

    first.focus()
    expect(document.activeElement).toBe(first)
    fireEvent.keyDown(first, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(last)
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
          <ArchiveRecoveryScreen report={makeReport()} />
        </ShellDataContext.Provider>
      </I18nProvider>,
    )
    unmount()
    expect(document.activeElement).toBe(trigger)
    document.body.removeChild(trigger)
  })

  test('non-Tab, non-Escape key on dialog is ignored', () => {
    renderScreen()
    const dialog = screen.getByRole('dialog')
    // Should not throw or move focus
    fireEvent.keyDown(dialog, { key: 'ArrowDown' })
    expect(dialog).toBeInTheDocument()
  })

  test('Tab in the middle does not force-wrap', () => {
    // Expand the list so the dialog has 3+ focusable buttons and a genuine "middle".
    const report = makeReport({
      recoverySnapshots: [
        makeSnapshot(),
        makeSnapshot({ id: 'snap-2', path: '/snap2.sqlite' }),
      ],
    })
    renderScreen(report)
    fireEvent.click(screen.getByRole('button', { name: /show all available/i }))

    const dialog = screen.getByRole('dialog')
    const buttons = Array.from(
      dialog.querySelectorAll<HTMLElement>('button'),
    ).filter((el) => !el.hasAttribute('disabled') && el.tabIndex >= 0)
    expect(buttons.length).toBeGreaterThan(2)

    const middle = buttons[1]
    middle.focus()
    expect(document.activeElement).toBe(middle)
    fireEvent.keyDown(dialog, { key: 'Tab' })
    // Focus is neither first nor last, so the handler does not wrap — focus stays put.
    expect(document.activeElement).toBe(middle)
  })

  test('snapshot with no date shows snapshotDateUnknown', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [makeSnapshot({ createdAt: null })],
      }),
    )
    expect(screen.getByText(/date unknown/i)).toBeInTheDocument()
  })

  test('unverified snapshot shows "Not verified" badge', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [makeSnapshot({ verifiedOpenable: false })],
      }),
    )
    expect(screen.getByText(/not verified/i)).toBeInTheDocument()
  })

  test('confirm step with no date uses confirmBodyDateUnknown copy', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [makeSnapshot({ createdAt: null })],
      }),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    // confirmBodyDateUnknown contains "selected snapshot"
    expect(screen.getByText(/selected snapshot/i)).toBeInTheDocument()
  })

  test('clicking "Restore" on an expanded-list card opens confirm step', () => {
    const snap2 = makeSnapshot({ id: 'snap-2', path: '/snap2.sqlite' })
    const report = makeReport({
      recoverySnapshots: [makeSnapshot(), snap2],
    })
    renderScreen(report)

    // Expand the list
    fireEvent.click(screen.getByRole('button', { name: /show all available/i }))

    // There are now 2 "Restore from this snapshot" buttons (headline + expanded)
    const restoreButtons = screen.getAllByRole('button', {
      name: /restore from this snapshot/i,
    })
    expect(restoreButtons.length).toBeGreaterThanOrEqual(2)

    // Click the second one (in the expanded list)
    fireEvent.click(restoreButtons[1])

    // Confirm step should appear
    expect(screen.getByText(/replace archive and restore/i)).toBeInTheDocument()
  })

  // ── Focus-trap branch coverage ──────────────────────────────────────────────

  test('Tab during restoring state does nothing (no focusable — line 102 branch)', () => {
    // Use a never-resolving restore so the component stays in "restoring" state
    const runFullArchiveRestore = vi.fn().mockReturnValue(new Promise(() => {}))
    renderScreen(makeReport(), { runFullArchiveRestore })

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    // Now in restoring state — dialog has no Tab-focusable elements (tabIndex={-1} excluded)
    const dialog = screen.getByRole('dialog')
    fireEvent.keyDown(dialog, { key: 'Tab' })
    // Dialog should still be present (no crash)
    expect(dialog).toBeInTheDocument()
  })

  test('Shift+Tab when focus is NOT on first element does not wrap (line 104 false branch)', () => {
    // Need 2+ snapshots so first ≠ last — only then can we focus the last button
    // while it truly differs from first, exercising the false branch at line 125.
    const report = makeReport({
      recoverySnapshots: [
        makeSnapshot(),
        makeSnapshot({ id: 'snap-2', path: '/snap2.sqlite' }),
      ],
    })
    renderScreen(report)
    // Expand so both Restore buttons are present
    fireEvent.click(screen.getByRole('button', { name: /show all available/i }))

    const dialog = screen.getByRole('dialog')
    const buttons = Array.from(
      dialog.querySelectorAll<HTMLElement>('button'),
    ).filter((el) => !el.hasAttribute('disabled') && el.tabIndex >= 0)

    // With an expanded list there are 3+ buttons: first ≠ last.
    expect(buttons.length).toBeGreaterThan(1)
    const last = buttons[buttons.length - 1]
    last.focus()
    expect(document.activeElement).toBe(last)

    // Shift+Tab: active element is last (≠ first) → handler does nothing → focus stays.
    fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(last)
  })

  test('Tab when focus is NOT on last element does not wrap (line 108 false branch)', () => {
    renderScreen()
    const dialog = screen.getByRole('dialog')
    const buttons = Array.from(
      dialog.querySelectorAll<HTMLElement>('button'),
    ).filter((el) => !el.hasAttribute('disabled'))

    // Focus on the FIRST element and fire Tab — focus is not on last so no wrap
    const first = buttons[0]
    first.focus()
    expect(document.activeElement).toBe(first)

    fireEvent.keyDown(dialog, { key: 'Tab' })
    // No wrap — document.activeElement stays on first (JSDOM doesn't move focus naturally)
    expect(document.activeElement).toBe(first)
  })

  // ── Success post-condition + body copy + diagnostics ────────────────────────

  test('successful restore leaves no error and no stuck "Restoring…" state', async () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    renderScreen(makeReport(), { runFullArchiveRestore })

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() => expect(runFullArchiveRestore).toHaveBeenCalledTimes(1))
    // No error surfaced and the screen is not wedged in the restoring state.
    expect(screen.queryByRole('alert')).toBeNull()
    await waitFor(() => expect(screen.queryByText('Restoring…')).toBeNull())
    // Back to the guided state — the headline Restore button is present again.
    expect(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    ).toBeInTheDocument()
  })

  test('body copy uses the singular string for exactly one verified snapshot', () => {
    renderScreen(makeReport({ recoverySnapshots: [makeSnapshot()] }))
    expect(screen.getByText(/found 1 verified snapshot/i)).toBeInTheDocument()
  })

  test('body copy uses the plural string with the count for multiple verified snapshots', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [
          makeSnapshot({ id: 'snap-a', path: '/a.sqlite' }),
          makeSnapshot({ id: 'snap-b', path: '/b.sqlite' }),
        ],
      }),
    )
    expect(screen.getByText(/found 2 verified snapshots/i)).toBeInTheDocument()
  })

  test('empty state renders the diagnostic report detail', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [],
        availableSnapshots: [],
        detail: 'atRestDriftUnresolved: mode mismatch',
      }),
    )
    expect(
      screen.getByText(/atRestDriftUnresolved: mode mismatch/),
    ).toBeInTheDocument()
  })

  test('expand toggle exposes aria-controls for the snapshot list', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [
          makeSnapshot(),
          makeSnapshot({ id: 'snap-2', path: '/snap2.sqlite' }),
        ],
      }),
    )
    const toggle = screen.getByRole('button', { name: /show all available/i })
    expect(toggle).toHaveAttribute('aria-controls', 'recovery-snapshot-list')
  })

  // ── FIX 2: bodyUnverifiedOnly ────────────────────────────────────────────────

  test('body copy uses bodyUnverifiedOnly when snapshots exist but none are verified', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [makeSnapshot({ verifiedOpenable: false })],
      }),
    )
    // bodyUnverifiedOnly contains a unique phrase about trying older snapshots
    expect(
      screen.getByText(/older snapshots can still be tried/i),
    ).toBeInTheDocument()
  })

  // ── FIX 3: sourceOp labels ───────────────────────────────────────────────────

  test('reconcile sourceOp displays "Encryption maintenance" label', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [makeSnapshot({ sourceOp: 'reconcile' })],
      }),
    )
    expect(screen.getByText(/encryption maintenance/i)).toBeInTheDocument()
  })

  test('periodic sourceOp displays "Periodic snapshot" label', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [makeSnapshot({ sourceOp: 'periodic' })],
      }),
    )
    expect(screen.getByText(/periodic snapshot/i)).toBeInTheDocument()
  })

  // ── FIX 4: confirm button aria-describedby ───────────────────────────────────

  test('confirm "Restore now" button has aria-describedby pointing to the confirm body', () => {
    renderScreen()
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    const confirmBtn = screen.getByRole('button', {
      name: /confirm and restore from this snapshot/i,
    })
    expect(confirmBtn).toHaveAttribute(
      'aria-describedby',
      'recovery-confirm-body',
    )
    // The referenced element must exist in the DOM
    expect(document.getElementById('recovery-confirm-body')).toBeInTheDocument()
  })

  // ── FIX 5: restoring state focus ─────────────────────────────────────────────

  test('entering restoring state moves focus to the status region', async () => {
    const runFullArchiveRestore = vi.fn().mockReturnValue(new Promise(() => {}))
    renderScreen(makeReport(), { runFullArchiveRestore })

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    // useEffect fires after render; waitFor flushes pending React work
    await waitFor(() => {
      const statusEl = screen.queryByText(/restoring/i)
      expect(statusEl).toBeInTheDocument()
      expect(statusEl).toHaveFocus()
    })
  })

  // ── FIX 8: seeAll without count ──────────────────────────────────────────────

  test('expand button shows "See all snapshots" without a count in the text', () => {
    const snap2 = makeSnapshot({ id: 'snap-2', path: '/snap2.sqlite' })
    const report = makeReport({
      recoverySnapshots: [makeSnapshot(), snap2],
    })
    renderScreen(report)

    const seeAllBtn = screen.getByRole('button', {
      name: /show all available/i,
    })
    // Visible text should be plain "See all snapshots" — no digit
    expect(seeAllBtn.textContent).not.toMatch(/\d/)
    expect(seeAllBtn).toHaveTextContent('See all snapshots')
  })

  // ── FIX 9: empty state reassurance ──────────────────────────────────────────

  test('empty state shows the emptyReassurance paragraph', () => {
    renderScreen(makeReport({ recoverySnapshots: [], availableSnapshots: [] }))
    // emptyReassurance contains "quarantine" and "not deleted"
    expect(screen.getByText(/quarantine/i)).toBeInTheDocument()
    expect(screen.getByText(/not deleted/i)).toBeInTheDocument()
  })

  // ── Encrypted-snapshot recovery gap: honest badge + key entry + keyed restore ──

  test('encrypted headline snapshot shows the honest "needs your key" badge', () => {
    renderScreen(
      makeReport({
        recoverySnapshots: [makeSnapshot({ encrypted: true })],
      }),
    )
    expect(screen.getByText(/needs your key/i)).toBeInTheDocument()
    expect(screen.queryByText('Verified')).toBeNull()
  })

  test('encrypted snapshot confirm shows the ArchiveKeyField and threads the entered key', async () => {
    const runFullArchiveRestore = vi.fn().mockResolvedValue({})
    renderScreen(
      makeReport({ recoverySnapshots: [makeSnapshot({ encrypted: true })] }),
      { runFullArchiveRestore },
    )

    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    const keyInput = screen.getByLabelText('Archive key')
    expect(keyInput).toBeInTheDocument()
    fireEvent.change(keyInput, { target: { value: 'the-key' } })

    fireEvent.click(
      screen.getByRole('button', {
        name: /confirm and restore from this snapshot/i,
      }),
    )

    await waitFor(() =>
      expect(runFullArchiveRestore).toHaveBeenCalledWith(
        '/snap.sqlite',
        'the-key',
      ),
    )
  })

  test('plaintext snapshot confirm shows no key field', () => {
    renderScreen()
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    expect(screen.queryByLabelText('Archive key')).toBeNull()
  })

  test('encrypted confirm moves focus to the key field, not the destructive button', () => {
    renderScreen(
      makeReport({ recoverySnapshots: [makeSnapshot({ encrypted: true })] }),
    )
    fireEvent.click(
      screen.getByRole('button', { name: /restore from this snapshot/i }),
    )
    // Focus lands on the field the user must fill, not the "Restore now" button.
    expect(document.activeElement).toBe(screen.getByLabelText('Archive key'))
  })

  test('main panel exposes a persistent "Reveal logs" forward path', async () => {
    renderScreen()
    const reveal = screen.getByRole('button', {
      name: /open the pathkeep logs/i,
    })
    expect(reveal).toBeInTheDocument()
    fireEvent.click(reveal)
    await waitFor(() => expect(backend.revealLogs).toHaveBeenCalled())
  })
})
