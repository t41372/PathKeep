/**
 * @file zero-state.test.tsx
 * @description Focused render coverage for Dashboard zero-state copy branches.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Verify zero-state next-action copy maps backend hints to locale-owned UI strings.
 * - Keep the zero-state composition covered without loading the full Dashboard route.
 *
 * ## Not responsible for
 * - Re-testing populated dashboard panels or route fallback selection.
 * - Re-testing backend dashboard snapshot creation.
 *
 * ## Dependencies
 * - Uses MemoryRouter because the zero-state primary action is a route link.
 *
 * ## Performance notes
 * - Pure render tests with tiny dashboard fixtures keep this branch cheap.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import type { DashboardSnapshot } from '../../lib/types'
import { DashboardZeroState } from './zero-state'

vi.mock('./panels', () => ({
  DashboardArchiveBoundaryPanel: () => <div>archive boundary</div>,
  DashboardStatsRow: () => <div>stats row</div>,
  DashboardZeroStateChecklistPanel: ({
    snapshotInitialized,
  }: {
    snapshotInitialized: boolean
  }) => <div>checklist:{String(snapshotInitialized)}</div>,
}))

describe('DashboardZeroState', () => {
  test('uses locale copy when the backend has no next action', () => {
    renderZeroState({ nextAction: null, snapshotInitialized: false })

    expect(screen.getByText('dashboard.zeroStateBody')).toBeVisible()
    expect(screen.getByText('checklist:false')).toBeVisible()
  })

  test('localizes known backend next-action hints and preserves unknown hints', () => {
    const { rerender } = renderZeroState({
      nextAction: 'Initialize the archive before backup.',
      snapshotInitialized: true,
    })

    expect(
      screen.getByText('dashboard.nextActionInitializeArchive'),
    ).toBeVisible()
    expect(screen.getByText('checklist:true')).toBeVisible()

    rerender(
      <MemoryRouter>
        <DashboardZeroState
          commonT={commonT}
          dashboard={dashboardFixture({
            nextAction:
              'Run a manual backup to create the first manifest and snapshot artifacts.',
          })}
          selectedProfiles={[]}
          snapshotInitialized={true}
          stats={[]}
          t={dashboardT}
        />
      </MemoryRouter>,
    )
    expect(screen.getByText('dashboard.nextActionRunFirstBackup')).toBeVisible()

    rerender(
      <MemoryRouter>
        <DashboardZeroState
          commonT={commonT}
          dashboard={dashboardFixture({
            nextAction: 'Review the browser source permissions.',
          })}
          selectedProfiles={[]}
          snapshotInitialized={true}
          stats={[]}
          t={dashboardT}
        />
      </MemoryRouter>,
    )
    expect(
      screen.getByText('Review the browser source permissions.'),
    ).toBeVisible()
  })
})

const dashboardT = (key: string) => key
const commonT = (key: string) => key

function renderZeroState({
  nextAction,
  snapshotInitialized,
}: {
  nextAction: string | null
  snapshotInitialized: boolean
}) {
  return render(
    <MemoryRouter>
      <DashboardZeroState
        commonT={commonT}
        dashboard={dashboardFixture({ nextAction })}
        selectedProfiles={[]}
        snapshotInitialized={snapshotInitialized}
        stats={[]}
        t={dashboardT}
      />
    </MemoryRouter>,
  )
}

function dashboardFixture(
  overrides: Partial<DashboardSnapshot> = {},
): DashboardSnapshot {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    lastSuccessfulBackupAt: null,
    nextAction: null,
    recentRuns: [],
    storage: {
      archiveDatabaseBytes: 0,
      exportBytes: 0,
      intelligenceBlobBytes: 0,
      intelligenceDatabaseBytes: 0,
      manifestBytes: 0,
      quarantineBytes: 0,
      searchDatabaseBytes: 0,
      semanticSidecarBytes: 0,
      snapshotBytes: 0,
      sourceEvidenceDatabaseBytes: 0,
      stagingBytes: 0,
    },
    totalDownloads: 0,
    totalProfiles: 0,
    totalUrls: 0,
    totalVisits: 0,
    ...overrides,
  }
}
