/**
 * @file preview-regressions.test.ts
 * @description Browser-preview regression coverage for fallback and edge-case backend facade branches.
 * @module lib/backend-tests/preview-regressions
 *
 * ## Responsibilities
 * - Protect preview fallback branches that are easy to miss once the backend facade is split across modules.
 * - Cover export pagination, seeded mock-state oddities, implicit snapshot/retention fallback args, and unknown command handling.
 * - Keep these lower-frequency branches separate from the main preview smoke path so failures point at the right domain faster.
 *
 * ## Not responsible for
 * - Main browser-preview command smoke coverage.
 * - App-lock behavioral detail.
 * - Tauri passthrough invocation assertions.
 *
 * ## Dependencies
 * - Depends on `../backend` and `./test-helpers` for the shared preview config fixture.
 * - Mocks `@tauri-apps/api/core` locally so the suite stays on the browser-preview path.
 *
 * ## Performance notes
 * - These tests mutate the in-memory preview fixture heavily, so each test resets the harness first to avoid cross-test drift.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'

const { invoke, isTauri } = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => false),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  isTauri,
}))

import { backend, backendTestHarness } from '../backend'
import type { RetentionPruneResult } from '../types'
import { previewConfigFixture as config } from './test-helpers'

describe('backend facade preview regressions', () => {
  beforeEach(() => {
    isTauri.mockReturnValue(false)
    invoke.mockReset()
    backendTestHarness.reset()
  })

  test('covers remaining preview fallback branches through seeded mock state', async () => {
    await expect(backend.securityStatus()).resolves.toMatchObject({
      mode: 'uninitialized',
    })
    await expect(backend.doctor()).resolves.toMatchObject({
      checks: [
        expect.objectContaining({
          name: 'import-artifacts',
          status: 'info',
        }),
        expect.objectContaining({
          name: 'visibility-state',
          status: 'ok',
        }),
      ],
    })
    await expect(backend.repairHealth()).resolves.toSatisfy((report) => {
      expect(report.runId).toBeGreaterThan(0)
      expect(report.repairedImportAudits).toBe(0)
      expect(report.repairedVisibilityRows).toBe(0)
      expect(report.clearedDerivedRows).toBe(0)
      return true
    })

    await backend.initializeArchive(
      {
        ...config,
        archiveMode: 'Plaintext',
        initialized: true,
      },
      null,
    )
    await expect(backend.securityStatus()).resolves.toMatchObject({
      mode: 'plaintext',
    })

    await expect(backend.scheduleStatus('windows')).resolves.toMatchObject({
      platform: 'windows',
      manualSteps: [
        expect.stringContaining('Task Scheduler'),
        expect.stringContaining('XML'),
      ],
      warnings: [expect.stringContaining('Task Scheduler')],
    })

    backendTestHarness.seedSchedule({
      platform: 'windows',
      label: 'dev.example.windows',
      executablePath: 'C:/PathKeep/pathkeep.exe',
      generatedFiles: [],
      manualSteps: ['Use the documented Task Scheduler import flow.'],
      applyCommands: [],
      rollbackCommands: [],
      applySupported: false,
    })
    await expect(backend.scheduleStatus('windows')).resolves.toMatchObject({
      installState: 'manual-review',
      manualSteps: ['Use the documented Task Scheduler import flow.'],
    })

    backendTestHarness.mutateState((state) => {
      state.snapshot.archiveStatus.warning = 'Preview archive warning.'
      state.snapshot.recentRuns = [
        {
          id: 900,
          startedAt: '2026-04-07T00:00:00Z',
          finishedAt: null,
          status: 'success',
          runType: 'backup',
          trigger: undefined,
          profileScope: undefined,
          manifestHash: null,
          profilesProcessed: 0,
          newVisits: 0,
          newUrls: 0,
          newDownloads: 0,
        },
      ]
      state.history.items = [
        ...state.history.items,
        {
          ...state.history.items[0],
          id: 99,
          profileId: 'standalone',
          url: 'https://example.test/unrelated-url',
          title: 'SQLite title match only',
          domain: 'example.test',
          visitTime: state.history.items[0].visitTime - 5000,
          visitedAt: new Date(
            state.history.items[0].visitTime - 5000,
          ).toISOString(),
        },
        {
          ...state.history.items[0],
          id: 100,
          profileId: 'chrome:NoTitle',
          url: 'https://example.test/no-title-row',
          title: null,
          domain: 'example.test',
          visitTime: state.history.items[0].visitTime - 10_000,
          visitedAt: new Date(
            state.history.items[0].visitTime - 10_000,
          ).toISOString(),
        },
      ]
      state.snapshot.config.ai.jobQueuePaused = true
    })

    await expect(backend.securityStatus()).resolves.toMatchObject({
      warnings: ['Preview archive warning.'],
    })

    await expect(backend.loadAuditRunDetail(900)).resolves.toMatchObject({
      trigger: 'manual',
      profileScope: ['chrome:Default'],
      artifacts: [
        expect.objectContaining({
          createdAt: '2026-04-07T00:00:00Z',
        }),
      ],
    })
    await expect(
      backendTestHarness.call('load_audit_run_detail'),
    ).resolves.toMatchObject({
      run: expect.objectContaining({ id: 900 }),
    })

    await expect(
      backend.queryHistory({
        q: 'title match only',
        domain: 'example.test',
        profileId: 'standalone',
        browserKind: 'standalone',
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 10,
        cursor: null,
      }),
    ).resolves.toMatchObject({
      total: 1,
      items: [expect.objectContaining({ id: 99 })],
    })
    await expect(
      backend.queryHistory({
        q: 'title-does-not-exist',
        domain: 'example.test',
        profileId: 'chrome:NoTitle',
        browserKind: 'chrome',
        startTimeMs: null,
        endTimeMs: null,
        sort: 'newest',
        limit: 10,
        cursor: null,
      }),
    ).resolves.toMatchObject({
      total: 0,
      items: [],
    })

    await expect(
      backend.searchAiHistory({
        query: 'history',
        profileId: null,
        domain: null,
      }),
    ).resolves.toMatchObject({
      total: expect.any(Number),
      nextCursor: null,
    })

    await expect(
      backendTestHarness.call('import_takeout'),
    ).resolves.toMatchObject({
      sourcePath: '/tmp/takeout.zip',
      dryRun: false,
    })
    await expect(
      backendTestHarness.call('preview_import_batch'),
    ).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1 }),
    })
    backendTestHarness.mutateState((state) => {
      state.snapshot.recentRuns = []
    })
    await expect(
      backendTestHarness.call('revert_import_batch'),
    ).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'reverted' }),
    })
    await expect(
      backendTestHarness.call('restore_import_batch'),
    ).resolves.toMatchObject({
      batch: expect.objectContaining({ id: 1, status: 'imported' }),
    })

    await expect(backend.replayAiJob(2)).resolves.toMatchObject({
      id: 2,
      state: 'paused',
    })

    backendTestHarness.mutateState((state) => {
      state.snapshot.recentRuns = []
    })
    await expect(
      backendTestHarness.call('load_audit_run_detail'),
    ).rejects.toThrow('1848')
  })

  test('exports all filtered preview rows instead of the current page only', async () => {
    backendTestHarness.mutateState((state) => {
      state.history.items = Array.from({ length: 75 }, (_, index) => ({
        id: index + 1,
        profileId: 'chrome:Default',
        url: `https://example.com/sqlite/${index + 1}`,
        title: `SQLite note ${index + 1}`,
        domain: 'example.com',
        visitedAt: new Date(Date.now() - index * 60_000).toISOString(),
        visitTime: Date.now() - index * 60_000,
        durationMs: 5_000,
        transition: 805306368,
        sourceVisitId: index + 1,
        appId: null,
      }))
    })

    const firstPage = await backend.queryHistory({
      q: 'sqlite',
      domain: null,
      profileId: null,
      browserKind: null,
      startTimeMs: null,
      endTimeMs: null,
      sort: 'newest',
      limit: 50,
      cursor: null,
    })
    const exportResult = await backend.exportHistory({
      query: {
        q: 'sqlite',
        limit: 50,
        cursor: firstPage.nextCursor,
      },
      format: 'jsonl',
    })
    const pagedExportResult = await backend.exportHistory({
      query: {
        q: 'sqlite',
        limit: 10,
        page: 3,
      },
      format: 'jsonl',
    })

    expect(firstPage.items).toHaveLength(50)
    expect(firstPage.total).toBe(75)
    expect(exportResult.count).toBe(75)
    expect(pagedExportResult.count).toBe(75)
  })

  test('covers mock fallback branches for snapshot restore and retention prune without explicit args', async () => {
    backendTestHarness.mutateState((state) => {
      state.snapshot.recentRuns = []
    })

    await expect(
      backend.previewSnapshotRestore({ snapshotPath: '' }),
    ).resolves.toMatchObject({
      snapshotPath: expect.stringContaining('/run-1'),
      snapshotKind: 'raw-source-checkpoint',
    })

    await expect(
      backendTestHarness.call('run_snapshot_restore'),
    ).resolves.toMatchObject({
      run: expect.objectContaining({
        id: 1848,
        runType: 'snapshot_restore',
        profileScope: ['chrome:Default'],
      }),
    })

    backendTestHarness.mutateState((state) => {
      state.snapshot.recentRuns = []
    })

    const prune = await backendTestHarness.call<RetentionPruneResult>(
      'run_retention_prune',
    )
    expect(prune).toMatchObject({
      runId: 1848,
      deletedBytes: 0,
      deletedFiles: 0,
      buckets: [],
    })
    await expect(
      backend.loadAuditRunDetail(Number(prune.runId)),
    ).resolves.toMatchObject({
      artifacts: [expect.objectContaining({ kind: 'retention' })],
    })
  })

  test('throws when a mock command is not implemented in browser preview mode', async () => {
    await expect(
      backendTestHarness.call('inspect_takeout'),
    ).resolves.toMatchObject({
      sourcePath: '/tmp/takeout.zip',
      dryRun: true,
    })
    await expect(
      backendTestHarness.call('totally_unknown_command'),
    ).rejects.toThrow('Mock backend does not implement totally_unknown_command')
  })
})
