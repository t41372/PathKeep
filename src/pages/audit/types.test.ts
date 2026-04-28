/**
 * @file types.test.ts
 * @description Focused regression coverage for Audit route helper decisions.
 * @module pages/audit
 *
 * ## Responsibilities
 * - Verify import-batch matching stays profile-aware and timestamp-based.
 * - Cover rollback/restore/import timestamp priority without mounting the Audit route.
 *
 * ## Not responsible for
 * - Re-testing Audit route rendering or panel interactions.
 * - Re-testing backend audit ledger persistence.
 *
 * ## Dependencies
 * - Uses only the typed Audit and Import overview contracts.
 *
 * ## Performance notes
 * - Pure helper tests keep strict coverage fast and deterministic.
 */

import { describe, expect, test } from 'vitest'
import type {
  AuditRunDetail,
  BackupRunOverview,
  ImportBatchOverview,
} from '../../lib/types'
import {
  parseAuditTimestamp,
  pickRelatedImportBatch,
  resolveBatchEventTime,
} from './types'

function batchFixture(
  id: number,
  overrides: Partial<ImportBatchOverview> = {},
): ImportBatchOverview {
  return {
    id,
    sourceKind: 'browser-direct',
    sourcePath: `/imports/${id}`,
    profileId: 'chrome:Default',
    createdAt: '2026-04-25T00:00:00.000Z',
    importedAt: '2026-04-25T00:10:00.000Z',
    revertedAt: null,
    status: 'visible',
    candidateItems: 10,
    importedItems: 9,
    duplicateItems: 1,
    visibleItems: 9,
    auditPath: null,
    gitCommit: null,
    ...overrides,
  }
}

function runFixture(
  runType: string,
  overrides: Partial<BackupRunOverview> = {},
): BackupRunOverview {
  return {
    id: 42,
    startedAt: '2026-04-25T00:12:00.000Z',
    finishedAt: '2026-04-25T00:12:30.000Z',
    status: 'success',
    runType,
    trigger: 'manual',
    profileScope: ['chrome:Default'],
    manifestHash: null,
    profilesProcessed: 1,
    newVisits: 0,
    newUrls: 0,
    newDownloads: 0,
    ...overrides,
  }
}

function detailFixture(
  runType: string,
  overrides: Partial<AuditRunDetail> = {},
): AuditRunDetail {
  const run = runFixture(runType)
  return {
    run,
    trigger: run.trigger ?? 'manual',
    timezone: 'UTC',
    dueOnly: false,
    profileScope: run.profileScope ?? [],
    warnings: [],
    errorMessage: null,
    stats: {},
    manifestPath: null,
    manifestHash: null,
    artifacts: [],
    ...overrides,
  }
}

describe('audit route helper types', () => {
  test('parses timestamps defensively', () => {
    expect(parseAuditTimestamp(null)).toBeNaN()
    expect(parseAuditTimestamp(undefined)).toBeNaN()
    expect(parseAuditTimestamp('not-a-date')).toBeNaN()
    expect(parseAuditTimestamp('2026-04-25T00:00:00.000Z')).toBe(
      Date.parse('2026-04-25T00:00:00.000Z'),
    )
  })

  test('resolves batch event time by audit run type priority', () => {
    const batch = batchFixture(1, {
      createdAt: '2026-04-25T00:00:00.000Z',
      importedAt: '2026-04-25T00:10:00.000Z',
      revertedAt: '2026-04-25T00:20:00.000Z',
    })

    expect(resolveBatchEventTime(batch, 'rollback')).toBe(
      Date.parse('2026-04-25T00:20:00.000Z'),
    )
    expect(
      resolveBatchEventTime(
        batchFixture(3, { importedAt: null, revertedAt: null }),
        'rollback',
      ),
    ).toBe(Date.parse('2026-04-25T00:00:00.000Z'))
    expect(resolveBatchEventTime(batch, 'restore')).toBe(
      Date.parse('2026-04-25T00:10:00.000Z'),
    )
    expect(
      resolveBatchEventTime(
        batchFixture(2, { importedAt: null, revertedAt: null }),
        'import',
      ),
    ).toBe(Date.parse('2026-04-25T00:00:00.000Z'))
  })

  test('picks the closest related import batch within the audit profile scope', () => {
    const nearbySameProfile = batchFixture(10, {
      importedAt: '2026-04-25T00:12:20.000Z',
    })
    const closerDifferentProfile = batchFixture(11, {
      profileId: 'safari:Work',
      importedAt: '2026-04-25T00:12:29.000Z',
    })
    const olderSameProfile = batchFixture(12, {
      importedAt: '2026-04-25T00:09:00.000Z',
    })

    expect(
      pickRelatedImportBatch(detailFixture('import'), [
        olderSameProfile,
        closerDifferentProfile,
        nearbySameProfile,
      ]),
    ).toBe(nearbySameProfile)
  })

  test('returns null for unsupported audit run types and missing details', () => {
    expect(pickRelatedImportBatch(null, [batchFixture(1)])).toBeNull()
    expect(
      pickRelatedImportBatch(detailFixture('backup'), [batchFixture(1)]),
    ).toBeNull()
    expect(
      pickRelatedImportBatch(
        detailFixture('import', {
          run: runFixture('import', {
            finishedAt: null,
            runType: null as unknown as string,
          }),
        }),
        [batchFixture(1)],
      ),
    ).toBeNull()
  })

  test('uses archive-wide batches when the run has no profile scope', () => {
    const batch = batchFixture(20, {
      profileId: 'safari:Work',
      revertedAt: '2026-04-25T00:12:29.000Z',
    })

    expect(
      pickRelatedImportBatch(
        detailFixture('rollback', {
          profileScope: [],
          run: runFixture('rollback', { profileScope: [] }),
        }),
        [batch],
      ),
    ).toBe(batch)
  })

  test('falls back to the run start time when matching unfinished audit details', () => {
    const batch = batchFixture(30, {
      importedAt: '2026-04-25T00:12:00.000Z',
    })

    expect(
      pickRelatedImportBatch(
        detailFixture('import', {
          run: runFixture('import', { finishedAt: null }),
        }),
        [batch],
      ),
    ).toBe(batch)
  })
})
