/**
 * @file audit-history.test.tsx
 * @description Protects the audit-history trust flows that filter past runs and keep surviving detail cache entries visible after partial failures.
 * @module pages/trust-flows
 *
 * ## Responsibilities
 * - Preserve the shipped audit-run filtering interactions across source, run-type, and artifact filters.
 * - Verify that audit detail tabs still show delta, artifacts, and warnings for the visible run.
 * - Ensure one failed audit detail fetch does not erase already-loaded successful detail state.
 *
 * ## Non-Responsibilities
 * - Does not own the shared trust-flow render harness.
 * - Does not redefine audit fixtures used by other trust-flow suites.
 * - Does not modify the original mega-suite cutover; it only carries the extracted audit-history coverage.
 *
 * ## Dependencies
 * - Depends on the shared trust-flow test helpers for canonical route rendering and per-test reset behavior.
 * - Depends on the backend client test harness because audit history is driven by the production-shaped snapshot and detail loaders.
 *
 * ## Performance Notes
 * - Keeps route setup minimal and reuses the shared harness so splitting the mega-suite does not add duplicate bootstrap work.
 */

import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { createNamespaceTranslator } from '../../lib/i18n'
import type { AuditRunDetail } from '../../lib/types'
import { AuditPage } from '../audit'
import { renderTrustPage, resetTrustFlowHarness } from './test-helpers'

const { invoke, isTauri, subscribeToImportProgress } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
  subscribeToImportProgress: vi.fn(() => Promise.resolve(vi.fn())),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

vi.mock('../../lib/ipc/import-progress', () => ({
  subscribeToImportProgress,
}))

describe('Audit trust flows', () => {
  beforeEach(() => {
    resetTrustFlowHarness({ invoke, isTauri, subscribeToImportProgress })
  })

  test('filters audit runs and shows delta against the previous visible run', async () => {
    const user = userEvent.setup()
    const auditT = createNamespaceTranslator('en', 'audit')
    const snapshot = await backend.getAppSnapshot()
    snapshot.config.initialized = true
    snapshot.recentRuns = [
      {
        id: 11,
        startedAt: '2026-04-07T10:00:00.000Z',
        finishedAt: '2026-04-07T10:05:00.000Z',
        status: 'success',
        runType: 'import',
        trigger: 'manual',
        profileScope: ['takeout::browser-history'],
        manifestHash: 'hash-11',
        profilesProcessed: 2,
        newVisits: 12,
        newUrls: 7,
        newDownloads: 3,
      },
      {
        id: 10,
        startedAt: '2026-04-06T10:00:00.000Z',
        finishedAt: '2026-04-06T10:05:00.000Z',
        status: 'success',
        runType: 'backup',
        trigger: 'schedule',
        profileScope: ['chrome:Default'],
        manifestHash: 'hash-10',
        profilesProcessed: 1,
        newVisits: 8,
        newUrls: 5,
        newDownloads: 2,
      },
      {
        id: 9,
        startedAt: '2026-04-05T10:00:00.000Z',
        finishedAt: '2026-04-05T10:05:00.000Z',
        status: 'success',
        runType: 'doctor',
        trigger: 'manual',
        profileScope: [],
        manifestHash: 'hash-9',
        profilesProcessed: 1,
        newVisits: 6,
        newUrls: 4,
        newDownloads: 1,
      },
    ]

    const detailMap: Record<number, AuditRunDetail> = {
      11: {
        run: snapshot.recentRuns[0],
        trigger: 'manual',
        timezone: 'America/Phoenix',
        dueOnly: false,
        profileScope: ['takeout::browser-history'],
        warnings: [],
        errorMessage: null,
        stats: {},
        manifestPath: '/tmp/run-11.json',
        manifestHash: 'hash-11',
        artifacts: [
          {
            kind: 'manifest',
            path: '/tmp/run-11.json',
            createdAt: '2026-04-07T10:05:00.000Z',
          },
        ],
      },
      10: {
        run: snapshot.recentRuns[1],
        trigger: 'schedule',
        timezone: 'America/Phoenix',
        dueOnly: false,
        profileScope: ['chrome:Default'],
        warnings: ['Schedule drift detected'],
        errorMessage: null,
        stats: {},
        manifestPath: '/tmp/run-10.json',
        manifestHash: 'hash-10',
        artifacts: [
          {
            kind: 'manifest',
            path: '/tmp/run-10.json',
            createdAt: '2026-04-06T10:05:00.000Z',
          },
          {
            kind: 'snapshot',
            path: '/tmp/run-10.snapshot',
            createdAt: '2026-04-06T10:05:00.000Z',
          },
        ],
      },
      9: {
        run: snapshot.recentRuns[2],
        trigger: 'manual',
        timezone: 'America/Phoenix',
        dueOnly: false,
        profileScope: [],
        warnings: [],
        errorMessage: null,
        stats: {},
        manifestPath: '/tmp/run-9.json',
        manifestHash: 'hash-9',
        artifacts: [
          {
            kind: 'snapshot',
            path: '/tmp/run-9.snapshot',
            createdAt: '2026-04-05T10:05:00.000Z',
          },
        ],
      },
    }

    const loadAuditRunDetailSpy = vi
      .spyOn(backend, 'loadAuditRunDetail')
      .mockImplementation((runId: number) => Promise.resolve(detailMap[runId]))

    renderTrustPage(<AuditPage />, {
      route: '/audit?run=11',
      snapshot,
    })

    expect(await screen.findByText('FILTERS')).toBeVisible()
    expect(
      await screen.findByRole('option', { name: 'snapshot' }),
    ).toBeVisible()
    await user.selectOptions(
      screen.getByLabelText('Source scope'),
      screen.getByRole('option', { name: 'Google Takeout' }),
    )
    expect(screen.getByRole('button', { name: /#11/ })).toBeVisible()
    expect(
      screen.queryByRole('button', { name: /#10/ }),
    ).not.toBeInTheDocument()
    await user.selectOptions(
      screen.getByLabelText('Source scope'),
      screen.getByRole('option', { name: 'All sources' }),
    )
    await user.selectOptions(
      screen.getByLabelText('Run type'),
      screen.getByRole('option', { name: 'Backup' }),
    )
    expect(screen.getByRole('button', { name: /#10/ })).toBeVisible()
    expect(
      screen.queryByRole('button', { name: /#11/ }),
    ).not.toBeInTheDocument()
    await user.selectOptions(
      screen.getByLabelText('Run type'),
      screen.getByRole('option', { name: 'All run types' }),
    )
    await user.selectOptions(
      screen.getByLabelText('Artifact type'),
      screen.getByRole('option', { name: 'snapshot' }),
    )

    await waitFor(() =>
      expect(
        screen.queryByRole('button', { name: /#11/ }),
      ).not.toBeInTheDocument(),
    )
    expect(screen.getByRole('button', { name: /#10/ })).toBeVisible()
    expect(await screen.findByText('Compared to run #9')).toBeVisible()
    expect(screen.getByText('+2')).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: auditT('artifactsTab') }),
    )
    expect(await screen.findByText(/\/tmp\/run-10\.snapshot/)).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: auditT('warningsTab') }),
    )
    expect(await screen.findByText('Schedule drift detected')).toBeVisible()

    loadAuditRunDetailSpy.mockRestore()
  })

  test('keeps successful audit detail cache entries when one run detail fails', async () => {
    const auditT = createNamespaceTranslator('en', 'audit')
    const snapshot = await backend.getAppSnapshot()
    snapshot.config.initialized = true
    snapshot.recentRuns = [
      {
        id: 21,
        startedAt: '2026-04-07T10:00:00.000Z',
        finishedAt: '2026-04-07T10:05:00.000Z',
        status: 'success',
        runType: 'import',
        trigger: 'manual',
        profileScope: ['takeout::browser-history'],
        manifestHash: 'hash-21',
        profilesProcessed: 1,
        newVisits: 3,
        newUrls: 2,
        newDownloads: 0,
      },
      {
        id: 20,
        startedAt: '2026-04-06T10:00:00.000Z',
        finishedAt: '2026-04-06T10:05:00.000Z',
        status: 'success',
        runType: 'backup',
        trigger: 'manual',
        profileScope: ['chrome:Default'],
        manifestHash: 'hash-20',
        profilesProcessed: 1,
        newVisits: 4,
        newUrls: 3,
        newDownloads: 0,
      },
    ]

    const detailMap: Record<number, AuditRunDetail> = {
      21: {
        run: snapshot.recentRuns[0],
        trigger: 'manual',
        timezone: 'America/Phoenix',
        dueOnly: false,
        profileScope: ['takeout::browser-history'],
        warnings: [],
        errorMessage: null,
        stats: {},
        manifestPath: '/tmp/run-21.json',
        manifestHash: 'hash-21',
        artifacts: [
          {
            kind: 'manifest',
            path: '/tmp/run-21.json',
            createdAt: '2026-04-07T10:05:00.000Z',
          },
        ],
      },
    }

    vi.spyOn(backend, 'loadAuditRunDetail').mockImplementation((runId) => {
      const detail = detailMap[runId]
      if (detail) {
        return Promise.resolve(detail)
      }
      return Promise.reject(new Error(`Run ${runId} detail unavailable`))
    })

    renderTrustPage(<AuditPage />, {
      language: 'en',
      route: '/audit?run=21',
      snapshot,
    })

    await waitFor(() =>
      expect(
        within(screen.getByLabelText(auditT('filterArtifactType'))).getByRole(
          'option',
          { name: 'manifest' },
        ),
      ).toBeInTheDocument(),
    )
  })
})
