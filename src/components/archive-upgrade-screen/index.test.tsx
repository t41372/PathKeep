/**
 * @file index.test.tsx
 * @description Coverage for the ArchiveUpgradeScreen one-time upgrade gate.
 * @module components/archive-upgrade-screen
 *
 * ## What this suite covers
 * - Renders the screen (headline / body / testid) in the initial working state.
 * - A `registrableDomainBackfill` tick with total>0 renders a DETERMINATE bar + human count.
 * - A `searchReprojection` tick with total>0 renders determinate.
 * - A `schemaMigration` tick with total===0 renders the canonical indeterminate sweep.
 * - The Intelligence phase renders an INFORMATIONAL line when its entry is streamed:false + pending:true.
 * - `initialize_archive` RESOLVING calls `finishArchiveUpgrade`; it does NOT dismiss before resolve.
 * - `initialize_archive` REJECTING shows a retryable error; Retry re-invokes `initialize_archive`.
 * - The aria-live region announces phase/milestone changes without spamming (same-bucket ticks are stable).
 * - The init is driven exactly once per attempt even under a StrictMode double-invoke.
 * - The step indicator is honest: it counts STREAMED pending phases only and clamps the terminal
 *   `finalizing` tick to the last step (never regresses to "Step 1").
 * - The finalizing/terminal state shows the `finishing` label; a `done:true` tick alone never dismisses.
 * - Retry re-focuses the status region; a re-bootstrap rejection is swallowed (no error surface).
 * - A teardown before the subscription resolves aborts the drive and unsubscribes (no dead-component work).
 */

import { StrictMode } from 'react'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { backend } from '../../lib/backend-client'
import type * as BackendClient from '../../lib/backend-client'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from '../../app/shell-data-context'
import type {
  AppConfig,
  ArchiveUpgradeAssessment,
  ArchiveUpgradeProgress,
} from '../../lib/types'
import { ArchiveUpgradeScreen } from './index'

// ──────────────────────────────────────────────────────────────────────────────
// Mock the progress subscribe helper so tests can push events, and the backend
// so `initialize_archive` never fires real IPC.
// ──────────────────────────────────────────────────────────────────────────────
const { subscribeMock, pushEvent, unsubscribeSpy } = vi.hoisted(() => {
  const state: { listener: ((event: unknown) => void) | null } = {
    listener: null,
  }
  const unsubscribeSpy = vi.fn()
  return {
    unsubscribeSpy,
    subscribeMock: vi.fn((listener: (event: unknown) => void) => {
      state.listener = listener
      return Promise.resolve(unsubscribeSpy)
    }),
    pushEvent: (event: ArchiveUpgradeProgress) => {
      state.listener?.(event)
    },
  }
})

vi.mock('../../lib/ipc/archive-upgrade-progress', () => ({
  subscribeToArchiveUpgradeProgress: subscribeMock,
}))

vi.mock('../../lib/backend-client', async (importOriginal) => {
  const actual = await importOriginal<typeof BackendClient>()
  return {
    ...actual,
    backend: {
      ...actual.backend,
      initializeArchive: vi.fn(),
    },
  }
})

// ──────────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ──────────────────────────────────────────────────────────────────────────────

function makeAssessment(
  overrides: Partial<ArchiveUpgradeAssessment> = {},
): ArchiveUpgradeAssessment {
  return {
    pending: true,
    currentSchemaVersion: 14,
    targetSchemaVersion: 16,
    phases: [
      {
        phase: 'registrableDomainBackfill',
        phaseLabel: 'archiveUpgrade.phase.registrableDomainBackfill',
        pending: true,
        streamed: true,
        estimatedTotal: 12000,
      },
      {
        phase: 'searchReprojection',
        phaseLabel: 'archiveUpgrade.phase.searchReprojection',
        pending: true,
        streamed: true,
        estimatedTotal: 8000,
      },
    ],
    ...overrides,
  }
}

const config = { initialized: true } as AppConfig

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
    recovery: null,
    archiveUpgrade: null,
    finishArchiveUpgrade: vi.fn().mockResolvedValue(undefined),
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
    runFullArchiveRestore: vi.fn().mockResolvedValue({}),
    ...overrides,
  } as ShellDataContextValue
}

function renderScreen(
  assessment: ArchiveUpgradeAssessment = makeAssessment(),
  shellOverrides: Partial<ShellDataContextValue> = {},
) {
  const shell = shellContextValue(shellOverrides)
  return {
    shell,
    ...render(
      <I18nProvider>
        <ShellDataContext.Provider value={shell}>
          <ArchiveUpgradeScreen assessment={assessment} config={config} />
        </ShellDataContext.Provider>
      </I18nProvider>,
    ),
  }
}

/** Renders and waits for the progress subscription to be established. */
async function renderReady(
  assessment: ArchiveUpgradeAssessment = makeAssessment(),
  shellOverrides: Partial<ShellDataContextValue> = {},
) {
  const utils = renderScreen(assessment, shellOverrides)
  await waitFor(() => expect(subscribeMock).toHaveBeenCalled())
  return utils
}

/** Pushes a progress tick inside act() so React state settles. */
function emit(event: ArchiveUpgradeProgress) {
  act(() => {
    pushEvent(event)
  })
}

/** Reads the prominent phase-name paragraph (distinct from the aria-live copy). */
function phaseName() {
  return screen
    .getByTestId('archive-upgrade-screen')
    .querySelector('.archive-upgrade-screen__phase')
}

// ──────────────────────────────────────────────────────────────────────────────
beforeEach(() => {
  vi.mocked(backend.initializeArchive).mockReset()
  unsubscribeSpy.mockClear()
  // Default: never resolves, so the screen holds its working state.
  vi.mocked(backend.initializeArchive).mockReturnValue(new Promise(() => {}))
})

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────
describe('ArchiveUpgradeScreen', () => {
  test('renders the dialog, headline and body in the initial working state', async () => {
    await renderReady()
    expect(
      screen.getByRole('dialog', { name: /upgrading your archive/i }),
    ).toBeInTheDocument()
    expect(screen.getByTestId('archive-upgrade-screen')).toBeInTheDocument()
    // The one-time reassurance body is visible immediately (~100ms feedback).
    expect(
      screen.getByText(/one-time step runs only after this update/i),
    ).toBeInTheDocument()
    // Before any event the count line shows the preparing copy.
    expect(screen.getByText(/getting things ready/i)).toBeInTheDocument()
  })

  test('a registrableDomainBackfill tick with total>0 renders a determinate bar + human count', async () => {
    await renderReady()
    emit({
      phase: 'registrableDomainBackfill',
      phaseLabel: 'archiveUpgrade.phase.registrableDomainBackfill',
      processed: 500,
      total: 12000,
      done: false,
    })

    const fill = screen.getByTestId('archive-upgrade-fill')
    expect(fill).not.toHaveClass('pk-indeterminate-bar')
    expect(fill).toHaveStyle({ width: '4%' })
    // Human count via toLocaleString.
    expect(screen.getByText('500 of 12,000')).toBeInTheDocument()
    // Determinate progressbar exposes aria-valuenow.
    expect(screen.getByRole('progressbar')).toHaveAttribute(
      'aria-valuenow',
      '4',
    )
    // Phase name is shown prominently.
    expect(phaseName()).toHaveTextContent(/grouping sites by domain/i)
  })

  test('a searchReprojection tick with total>0 renders determinate', async () => {
    await renderReady()
    emit({
      phase: 'searchReprojection',
      phaseLabel: 'archiveUpgrade.phase.searchReprojection',
      processed: 4000,
      total: 8000,
      done: false,
    })

    const fill = screen.getByTestId('archive-upgrade-fill')
    expect(fill).not.toHaveClass('pk-indeterminate-bar')
    expect(fill).toHaveStyle({ width: '50%' })
    expect(screen.getByText('4,000 of 8,000')).toBeInTheDocument()
    expect(phaseName()).toHaveTextContent(/rebuilding search/i)
  })

  test('a schemaMigration tick with total===0 renders the indeterminate sweep', async () => {
    await renderReady(
      makeAssessment({
        phases: [
          {
            phase: 'schemaMigration',
            phaseLabel: 'archiveUpgrade.phase.schemaMigration',
            pending: true,
            streamed: true,
            estimatedTotal: 0,
          },
        ],
      }),
    )
    emit({
      phase: 'schemaMigration',
      phaseLabel: 'archiveUpgrade.phase.schemaMigration',
      processed: 0,
      total: 0,
      done: false,
    })

    const fill = screen.getByTestId('archive-upgrade-fill')
    expect(fill).toHaveClass('pk-indeterminate-bar')
    // Indeterminate: no fabricated aria-valuenow.
    expect(screen.getByRole('progressbar')).not.toHaveAttribute('aria-valuenow')
    // The generic working label is shown (no count).
    expect(screen.getByText(/^working/i)).toBeInTheDocument()
  })

  test('the Intelligence phase renders an informational line (no bar) when streamed:false + pending:true', async () => {
    await renderReady(
      makeAssessment({
        phases: [
          {
            phase: 'intelligence',
            phaseLabel: 'archiveUpgrade.phase.intelligence',
            pending: true,
            streamed: false,
            estimatedTotal: 0,
          },
        ],
      }),
    )
    // Informational line present.
    expect(
      screen.getByText(/insights will refresh quietly in the background/i),
    ).toBeInTheDocument()
    // Falls back to the first phase name when nothing is streamed yet.
    expect(phaseName()).toHaveTextContent(/refreshing insights/i)
  })

  test('does not render the Intelligence info line when no non-streamed intelligence phase is pending', async () => {
    await renderReady()
    expect(
      screen.queryByText(/insights will refresh quietly in the background/i),
    ).not.toBeInTheDocument()
  })

  test('initialize_archive resolving hands off to the shell via finishArchiveUpgrade', async () => {
    const finishArchiveUpgrade = vi.fn().mockResolvedValue(undefined)
    vi.mocked(backend.initializeArchive).mockResolvedValue(
      {} as Awaited<ReturnType<typeof backend.initializeArchive>>,
    )
    await renderReady(makeAssessment(), { finishArchiveUpgrade })

    await waitFor(() => expect(finishArchiveUpgrade).toHaveBeenCalledTimes(1))
    // No error surface on the happy path.
    expect(screen.queryByRole('alert')).toBeNull()
  })

  test('does not dismiss (call finishArchiveUpgrade) before initialize_archive resolves', async () => {
    const finishArchiveUpgrade = vi.fn().mockResolvedValue(undefined)
    // Default init never resolves.
    await renderReady(makeAssessment(), { finishArchiveUpgrade })
    emit({
      phase: 'registrableDomainBackfill',
      phaseLabel: 'archiveUpgrade.phase.registrableDomainBackfill',
      processed: 6000,
      total: 12000,
      done: false,
    })
    // A mid-flight progress event (even at 50%) must NOT trigger the handoff.
    await Promise.resolve()
    expect(finishArchiveUpgrade).not.toHaveBeenCalled()
    expect(screen.getByTestId('archive-upgrade-screen')).toBeInTheDocument()
  })

  test('initialize_archive rejecting shows a retryable error and Retry re-invokes it', async () => {
    vi.mocked(backend.initializeArchive)
      .mockRejectedValueOnce(new Error('migration boom'))
      .mockReturnValue(new Promise(() => {}))
    await renderReady()

    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: /retry the archive upgrade/i }),
      ).toBeInTheDocument(),
    )
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(screen.getByText(/migration boom/)).toBeInTheDocument()
    expect(backend.initializeArchive).toHaveBeenCalledTimes(1)

    // Retry bumps the attempt and re-drives the migration.
    fireEvent.click(
      screen.getByRole('button', { name: /retry the archive upgrade/i }),
    )
    await waitFor(() =>
      expect(backend.initializeArchive).toHaveBeenCalledTimes(2),
    )
    // Back to the working state (second attempt never resolves).
    expect(screen.queryByRole('alert')).toBeNull()
    expect(screen.getByText(/getting things ready/i)).toBeInTheDocument()
  })

  test('the aria-live region announces phase/milestone changes without spamming', async () => {
    await renderReady()
    const live = screen.getByTestId('archive-upgrade-live')

    emit({
      phase: 'registrableDomainBackfill',
      phaseLabel: 'archiveUpgrade.phase.registrableDomainBackfill',
      processed: 500,
      total: 12000,
      done: false,
    })
    const firstAnnouncement = live.textContent
    expect(firstAnnouncement).toMatch(/grouping sites by domain/i)
    expect(firstAnnouncement).toMatch(/0%/)

    // Second tick in the SAME 25% bucket → announcement is unchanged (no spam).
    emit({
      phase: 'registrableDomainBackfill',
      phaseLabel: 'archiveUpgrade.phase.registrableDomainBackfill',
      processed: 1000,
      total: 12000,
      done: false,
    })
    expect(live.textContent).toBe(firstAnnouncement)

    // Crossing into the 50% bucket → the announcement updates.
    emit({
      phase: 'registrableDomainBackfill',
      phaseLabel: 'archiveUpgrade.phase.registrableDomainBackfill',
      processed: 6000,
      total: 12000,
      done: false,
    })
    expect(live.textContent).not.toBe(firstAnnouncement)
    expect(live.textContent).toMatch(/50%/)
  })

  test('the step indicator reflects the current phase position among pending phases', async () => {
    await renderReady()
    // Before any tick the first streamed pending phase (step 1 of 2) is current.
    expect(screen.getByText(/step 1 of 2/i)).toBeInTheDocument()

    emit({
      phase: 'searchReprojection',
      phaseLabel: 'archiveUpgrade.phase.searchReprojection',
      processed: 100,
      total: 8000,
      done: false,
    })
    expect(screen.getByText(/step 2 of 2/i)).toBeInTheDocument()
  })

  test('moves focus into the status region on mount', async () => {
    await renderReady()
    const status = screen
      .getByTestId('archive-upgrade-screen')
      .querySelector<HTMLElement>('.archive-upgrade-screen__status')
    expect(status).toHaveFocus()
  })

  test('counts only streamed pending phases and clamps the terminal tick to the last step', async () => {
    await renderReady(
      makeAssessment({
        phases: [
          {
            phase: 'schemaMigration',
            phaseLabel: 'archiveUpgrade.phase.schemaMigration',
            pending: true,
            streamed: true,
            estimatedTotal: 0,
          },
          {
            phase: 'registrableDomainBackfill',
            phaseLabel: 'archiveUpgrade.phase.registrableDomainBackfill',
            pending: true,
            streamed: true,
            estimatedTotal: 12000,
          },
          {
            phase: 'searchReprojection',
            phaseLabel: 'archiveUpgrade.phase.searchReprojection',
            pending: true,
            streamed: true,
            estimatedTotal: 8000,
          },
          {
            phase: 'intelligence',
            phaseLabel: 'archiveUpgrade.phase.intelligence',
            pending: true,
            streamed: false,
            estimatedTotal: 0,
          },
        ],
      }),
    )
    // The non-streamed Intelligence phase must NOT inflate the total.
    expect(screen.getByText(/step 1 of 3/i)).toBeInTheDocument()
    expect(screen.queryByText(/of 4/i)).not.toBeInTheDocument()

    // Advance to the last streamed phase.
    emit({
      phase: 'searchReprojection',
      phaseLabel: 'archiveUpgrade.phase.searchReprojection',
      processed: 100,
      total: 8000,
      done: false,
    })
    expect(screen.getByText(/step 3 of 3/i)).toBeInTheDocument()

    // The terminal `finalizing` tick is not among the assessment phases; it must
    // clamp to the LAST step, never regress to "Step 1".
    emit({
      phase: 'finalizing',
      phaseLabel: 'archiveUpgrade.phase.finalizing',
      processed: 0,
      total: 0,
      done: true,
    })
    expect(screen.getByText(/step 3 of 3/i)).toBeInTheDocument()
    expect(screen.queryByText(/step 1 of 3/i)).not.toBeInTheDocument()
  })

  test('shows the finishing label (and announces it) for the finalizing state', async () => {
    await renderReady()
    emit({
      phase: 'finalizing',
      phaseLabel: 'archiveUpgrade.phase.finalizing',
      processed: 0,
      total: 0,
      done: false,
    })
    // The count/status line reads the honest "Almost done…" copy, not "Working…".
    const count = screen
      .getByTestId('archive-upgrade-screen')
      .querySelector('.archive-upgrade-screen__count')
    expect(count).toHaveTextContent(/almost done/i)
    // The aria-live detail reflects finishing too.
    expect(screen.getByTestId('archive-upgrade-live')).toHaveTextContent(
      /almost done/i,
    )
  })

  test('a done:true terminal tick alone never dismisses the gate', async () => {
    const finishArchiveUpgrade = vi.fn().mockResolvedValue(undefined)
    // Default init never resolves, so only the tick could (wrongly) dismiss.
    await renderReady(makeAssessment(), { finishArchiveUpgrade })
    emit({
      phase: 'finalizing',
      phaseLabel: 'archiveUpgrade.phase.finalizing',
      processed: 0,
      total: 0,
      done: true,
    })
    await Promise.resolve()
    expect(finishArchiveUpgrade).not.toHaveBeenCalled()
    expect(screen.getByTestId('archive-upgrade-screen')).toBeInTheDocument()
  })

  test('swallows a re-bootstrap (finishArchiveUpgrade) rejection without a retry surface', async () => {
    vi.mocked(backend.initializeArchive).mockResolvedValue(
      {} as Awaited<ReturnType<typeof backend.initializeArchive>>,
    )
    const finishArchiveUpgrade = vi
      .fn()
      .mockRejectedValue(new Error('re-bootstrap boom'))
    await renderReady(makeAssessment(), { finishArchiveUpgrade })

    await waitFor(() => expect(finishArchiveUpgrade).toHaveBeenCalledTimes(1))
    // The re-bootstrap owns its own error surfacing (the gate is unmounting);
    // the gate must NOT flip into its retryable error state.
    expect(screen.queryByRole('alert')).toBeNull()
  })

  test('re-focuses the status region after Retry', async () => {
    vi.mocked(backend.initializeArchive)
      .mockRejectedValueOnce(new Error('migration boom'))
      .mockReturnValue(new Promise(() => {}))
    await renderReady()

    const retryButton = await screen.findByRole('button', {
      name: /retry the archive upgrade/i,
    })
    fireEvent.click(retryButton)

    const status = screen
      .getByTestId('archive-upgrade-screen')
      .querySelector<HTMLElement>('.archive-upgrade-screen__status')
    await waitFor(() => expect(status).toHaveFocus())
  })

  test('aborts the drive and unsubscribes when torn down before the subscription resolves', async () => {
    // Render WITHOUT awaiting the subscription, then unmount before the pending
    // subscribe promise resolves.
    const { unmount } = renderScreen()
    unmount()
    // Now let the still-pending subscribe resolve — post-teardown.
    await act(async () => {
      await Promise.resolve()
    })
    // The migration must not start on a dead component, and the eventually
    // registered listener is cleaned up.
    expect(backend.initializeArchive).not.toHaveBeenCalled()
    expect(unsubscribeSpy).toHaveBeenCalledTimes(1)
  })

  test('drives initialize_archive exactly once under a StrictMode double-invoke', async () => {
    vi.mocked(backend.initializeArchive).mockReturnValue(new Promise(() => {}))
    render(
      <StrictMode>
        <I18nProvider>
          <ShellDataContext.Provider value={shellContextValue()}>
            <ArchiveUpgradeScreen
              assessment={makeAssessment()}
              config={config}
            />
          </ShellDataContext.Provider>
        </I18nProvider>
      </StrictMode>,
    )
    await waitFor(() => expect(backend.initializeArchive).toHaveBeenCalled())
    await act(async () => {
      await Promise.resolve()
    })
    expect(backend.initializeArchive).toHaveBeenCalledTimes(1)
  })
})
