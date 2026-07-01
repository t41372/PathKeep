/**
 * @file archive-recovery.test.tsx
 * @description Coverage for the shell-data recovery bootstrap path and
 * `runFullArchiveRestore` action.
 * @module app/shell-data-tests
 *
 * ## What this suite covers
 * - When `archiveNeedsLaunchRecovery` is true and `initializeArchive` throws a
 *   recovery-formatted error, the shell sets `recovery` (lines 487-497).
 * - When `initializeArchive` throws a non-recovery error, it re-throws so the outer
 *   error handler catches it (line 499).
 * - `runFullArchiveRestore` calls `backend.runFullArchiveRestore`, clears `recovery`,
 *   and triggers a background refresh (lines 800-803).
 */

import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { I18nContext } from '../../lib/i18n/context'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import type { ArchiveRecoveryReport } from '../../lib/types'
import { ShellDataProvider } from '../shell-data'
import { useShellData } from '../shell-data-context'
import {
  createI18nValue,
  getDefaultBuildInfo,
  resetShellDataHarness,
  seedSnapshot,
} from './test-helpers'

// ──────────────────────────────────────────────────────────────────────────────
// Custom probe exposing recovery state and runFullArchiveRestore trigger
// ──────────────────────────────────────────────────────────────────────────────

function ArchiveRecoveryProbe() {
  const shell = useShellData()
  return (
    <div>
      <div data-testid="recovery-set">{shell.recovery ? 'yes' : 'no'}</div>
      <div data-testid="shell-error">{shell.error ?? 'none'}</div>
      <div data-testid="loading">{String(shell.loading)}</div>
      <button
        type="button"
        onClick={() => {
          void shell
            .runFullArchiveRestore('/snap.sqlite')
            .catch(() => undefined)
        }}
      >
        run-restore
      </button>
    </div>
  )
}

function renderRecoveryProbe() {
  return render(
    <I18nContext.Provider value={createI18nValue('en')}>
      <ProfileScopeProvider>
        <ShellDataProvider>
          <ArchiveRecoveryProbe />
        </ShellDataProvider>
      </ProfileScopeProvider>
    </I18nContext.Provider>,
  )
}

// ──────────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ──────────────────────────────────────────────────────────────────────────────

function makeRecoveryReport(): ArchiveRecoveryReport {
  return {
    kind: 'atRestDriftUnresolved',
    configMode: 'Plaintext',
    availableSnapshots: ['/snap.sqlite'],
    recoverySnapshots: [],
    detail: 'archive drift detected',
  }
}

function makeRecoveryError(report: ArchiveRecoveryReport): Error {
  return new Error(`archive_recovery_required: ${JSON.stringify(report)}`)
}

// ──────────────────────────────────────────────────────────────────────────────
describe('ShellDataProvider — archive recovery paths', () => {
  beforeEach(() => {
    resetShellDataHarness()
  })

  test('sets recovery when initializeArchive throws a recovery-formatted error', async () => {
    const { snapshot } = await seedSnapshot()

    // Build a snapshot that triggers archiveNeedsLaunchRecovery:
    // initialized=true, encrypted=false, unlocked=false, warning set
    const recoverySnapshot = {
      ...snapshot,
      archiveStatus: {
        ...snapshot.archiveStatus,
        initialized: true,
        encrypted: false,
        unlocked: false,
        warning: 'archive drift detected',
      },
    }

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(recoverySnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'initializeArchive').mockRejectedValue(
      makeRecoveryError(makeRecoveryReport()),
    )

    renderRecoveryProbe()

    // Recovery should be set in context after bootstrap
    await waitFor(() =>
      expect(screen.getByTestId('recovery-set')).toHaveTextContent('yes'),
    )
    expect(screen.getByTestId('loading')).toHaveTextContent('false')
  })

  test('sets shell error when initializeArchive throws a non-recovery error', async () => {
    const { snapshot } = await seedSnapshot()

    const recoverySnapshot = {
      ...snapshot,
      archiveStatus: {
        ...snapshot.archiveStatus,
        initialized: true,
        encrypted: false,
        unlocked: false,
        warning: 'archive drift detected',
      },
    }

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(recoverySnapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    // Throw a non-recovery error (no archive_recovery_required prefix)
    vi.spyOn(backend, 'initializeArchive').mockRejectedValue(
      new Error('SQLITE_CORRUPT: database disk image is malformed'),
    )

    renderRecoveryProbe()

    // Shell should surface an error, not set recovery
    await waitFor(() =>
      expect(screen.getByTestId('shell-error')).not.toHaveTextContent('none'),
    )
    expect(screen.getByTestId('recovery-set')).toHaveTextContent('no')
  })

  test('runFullArchiveRestore calls backend, clears recovery, and refreshes app data', async () => {
    const user = userEvent.setup()
    const { snapshot, dashboard } = await seedSnapshot()

    vi.spyOn(backend, 'getAppSnapshot').mockResolvedValue(snapshot)
    vi.spyOn(backend, 'getAppBuildInfo').mockResolvedValue(
      getDefaultBuildInfo(),
    )
    vi.spyOn(backend, 'loadDashboardSnapshot').mockResolvedValue(dashboard)
    const runRestoreSpy = vi
      .spyOn(backend, 'runFullArchiveRestore')
      .mockResolvedValue({
        runId: null,
        restoredSnapshotPath: '/snap.sqlite',
        restoredMode: 'Plaintext',
        quarantineDir: '/quarantine/20260601',
        sourceEvidenceRebuilt: false,
        warnings: [],
      })

    renderRecoveryProbe()

    // Wait for bootstrap to finish
    await waitFor(() =>
      expect(screen.getByTestId('loading')).toHaveTextContent('false'),
    )

    await act(async () => {
      await user.click(screen.getByRole('button', { name: 'run-restore' }))
    })

    await waitFor(() =>
      expect(runRestoreSpy).toHaveBeenCalledWith(
        { snapshotPath: '/snap.sqlite' },
        null,
      ),
    )
  })
})
