/**
 * @file upgrade-bootstrap.test.tsx
 * @description Shell-data bootstrap coverage for the one-time archive-upgrade gate.
 * @module app/index-tests
 *
 * ## What this suite covers
 * - When the cheap `assess_archive_upgrade` pre-check reports `pending: true` for a healthy,
 *   unlocked, initialized archive, the shell mounts the blocking `ArchiveUpgradeScreen`.
 * - When it reports `pending: false`, the shell latches and renders the normal dashboard.
 *
 * The best-effort catch branch (assess throwing) is already exercised by the fixture-based
 * suites, whose browser-preview backend throws on the unknown `assess_archive_upgrade` command.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { createMemoryRouter } from 'react-router-dom'
import App from '../index'
import { appRoutes } from '../router'
import { backend } from '../../lib/backend-client'
import type { ArchiveUpgradeAssessment } from '../../lib/types'
import { resetAppShellHarness, seedArchiveRun } from './test-helpers'

// Keep the mounted upgrade screen off the real IPC bus.
vi.mock('../../lib/ipc/archive-upgrade-progress', () => ({
  subscribeToArchiveUpgradeProgress: vi.fn().mockResolvedValue(() => {}),
}))

function pendingAssessment(): ArchiveUpgradeAssessment {
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
        phase: 'intelligence',
        phaseLabel: 'archiveUpgrade.phase.intelligence',
        pending: true,
        streamed: false,
        estimatedTotal: 0,
      },
    ],
  }
}

function notPendingAssessment(): ArchiveUpgradeAssessment {
  return {
    pending: false,
    currentSchemaVersion: 16,
    targetSchemaVersion: 16,
    phases: [],
  }
}

describe('App shell archive-upgrade bootstrap', () => {
  beforeEach(() => {
    resetAppShellHarness()
  })

  test('mounts the upgrade screen when the pre-check reports a pending migration', async () => {
    await seedArchiveRun()
    vi.spyOn(backend, 'assessArchiveUpgrade').mockResolvedValue(
      pendingAssessment(),
    )
    // The screen drives initialize_archive; keep it pending so the gate holds
    // (and does not re-assess in a loop).
    vi.spyOn(backend, 'initializeArchive').mockReturnValue(
      new Promise(() => {}),
    )

    const router = createMemoryRouter(appRoutes, { initialEntries: ['/'] })
    render(<App router={router} />)

    expect(
      await screen.findByTestId('archive-upgrade-screen'),
    ).toBeInTheDocument()
    expect(screen.queryByTestId('dashboard-page')).not.toBeInTheDocument()
  })

  test('renders the normal shell when the pre-check reports no pending migration', async () => {
    await seedArchiveRun()
    vi.spyOn(backend, 'assessArchiveUpgrade').mockResolvedValue(
      notPendingAssessment(),
    )

    const router = createMemoryRouter(appRoutes, { initialEntries: ['/'] })
    render(<App router={router} />)

    expect(await screen.findByTestId('dashboard-page')).toBeInTheDocument()
    expect(
      screen.queryByTestId('archive-upgrade-screen'),
    ).not.toBeInTheDocument()
  })

  test('finishArchiveUpgrade re-bootstraps the shell once initialize_archive resolves', async () => {
    await seedArchiveRun()
    const snapshot = await backend.getAppSnapshot()
    // First pre-check is pending (shows the gate); after the gate's
    // initialize_archive resolves, finishArchiveUpgrade re-assesses as done.
    vi.spyOn(backend, 'assessArchiveUpgrade')
      .mockResolvedValueOnce(pendingAssessment())
      .mockResolvedValue(notPendingAssessment())
    vi.spyOn(backend, 'initializeArchive').mockResolvedValue(snapshot)

    const router = createMemoryRouter(appRoutes, { initialEntries: ['/'] })
    render(<App router={router} />)

    // The gate hands off and the normal dashboard takes over.
    expect(await screen.findByTestId('dashboard-page')).toBeInTheDocument()
    expect(
      screen.queryByTestId('archive-upgrade-screen'),
    ).not.toBeInTheDocument()
  })
})
