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
  BackupWarning,
  ImportBatchOverview,
} from '../../lib/types'
import {
  localizedBackupWarningText,
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

  test('localizes coded run warnings and passes opaque diagnostics through', () => {
    const t = (key: string, vars?: Record<string, string | number>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key
    const warning = (overrides: Partial<BackupWarning>): BackupWarning => ({
      code: 'og-cleanup-summary',
      message: 'English fallback.',
      ...overrides,
    })

    // No details at all (older backend, non-backup run type) keeps the raw
    // message so a recorded warning is never dropped.
    expect(localizedBackupWarningText(undefined, 'raw', t, 'en')).toBe('raw')
    expect(
      localizedBackupWarningText(warning({ code: '' }), 'raw', t, 'en'),
    ).toBe('raw')
    expect(
      localizedBackupWarningText(
        warning({ code: 'future-warning' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('raw')

    expect(
      localizedBackupWarningText(
        warning({ code: 'og-refetch-summary', count: 1200, succeeded: 400 }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningOgRefetchSummaryMany:{"count":"1,200","succeeded":"400"}',
    )
    expect(
      localizedBackupWarningText(
        warning({ code: 'og-refetch-failed', diagnostic: 'dns hiccup' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningOgRefetchFailed dns hiccup')
    expect(
      localizedBackupWarningText(
        warning({ code: 'og-prefetch-summary', count: 7, succeeded: 4 }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningOgPrefetchSummaryMany:{"count":"7","succeeded":"4"}')
    expect(
      localizedBackupWarningText(
        warning({ code: 'og-prefetch-failed' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningOgPrefetchFailed English fallback.')
    expect(
      localizedBackupWarningText(
        warning({ count: 4, blobs: 2, bytes: 1234 }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningOgCleanupSummaryMany:{"count":"4","blobs":"2","bytes":"1,234"}',
    )
    expect(
      localizedBackupWarningText(
        warning({ code: 'og-cleanup-failed', diagnostic: 'archive locked' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningOgCleanupFailed archive locked')
    expect(
      localizedBackupWarningText(
        warning({ code: 'intelligence-refresh-failed', diagnostic: 'locked' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningIntelligenceRefreshFailed locked')
    expect(
      localizedBackupWarningText(
        warning({ code: 'ai-autoindex-queued-while-paused', jobId: 42 }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningAiAutoIndexQueuedWhilePaused:{"jobId":42}')
    // A coded warning missing the value its copy names falls back to the
    // backend's own sentence. Interpolating a blank would ship a hole —
    // "AI auto-index queued job  while the queue is paused" — which reads like
    // a rendering bug rather than a warning.
    expect(
      localizedBackupWarningText(
        warning({ code: 'ai-autoindex-queued-while-paused' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('raw')
    expect(
      localizedBackupWarningText(
        warning({ code: 'ai-autoindex-enqueue-failed', diagnostic: 'offline' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningAiAutoIndexEnqueueFailed offline')
    expect(
      localizedBackupWarningText(
        warning({ code: 'ai-autoindex-provider-not-ready' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningAiAutoIndexProviderNotReady English fallback.')
    expect(
      localizedBackupWarningText(
        warning({
          code: 'safari-full-disk-access-skip',
          profileId: 'safari:default',
        }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningSafariFullDiskAccessSkip:{"profileId":"safari:default"}',
    )
    expect(
      localizedBackupWarningText(
        warning({ code: 'safari-full-disk-access-skip' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('raw')
    expect(
      localizedBackupWarningText(
        warning({ code: 'source-evidence-rebuild-needed', diagnostic: 'boom' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningSourceEvidenceRebuildNeeded boom')
    expect(
      localizedBackupWarningText(
        warning({ code: 'search-projection-rebuild-needed' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningSearchProjectionRebuildNeeded English fallback.')
    expect(
      localizedBackupWarningText(
        warning({
          code: 'profile-history-unreadable-skip',
          profileId: 'edge:Default',
          diagnostic: 'History is missing at /profiles/edge',
        }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningProfileHistoryUnreadableSkip:{"profileId":"edge:Default"} History is missing at /profiles/edge',
    )
    expect(
      localizedBackupWarningText(
        warning({
          code: 'profile-not-detected-skip',
          profileId: 'chrome:Gone',
        }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningProfileNotDetectedSkip:{"profileId":"chrome:Gone"}')
    expect(
      localizedBackupWarningText(
        warning({
          code: 'staging-fallback-recovered-copy',
          profileId: 'chrome:Default',
          diagnostic: 'History: database is locked',
        }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningStagingFallbackRecoveredCopy:{"profileId":"chrome:Default"} History: database is locked',
    )
    expect(
      localizedBackupWarningText(
        warning({ code: 'git-history-skipped', diagnostic: 'git not found' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningGitHistorySkipped git not found')
    expect(
      localizedBackupWarningText(
        warning({ code: 'missing-table', profileId: 'chrome:Default' }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningParserMissingTable:{"profileId":"chrome:Default"} English fallback.',
    )
    expect(
      localizedBackupWarningText(
        warning({
          code: 'missing-source',
          profileId: 'chrome:Default',
          diagnostic: 'favicons database was not provided',
        }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningParserMissingSource:{"profileId":"chrome:Default"} favicons database was not provided',
    )
    expect(
      localizedBackupWarningText(
        warning({ code: 'baseline-support', profileId: 'firefox:default' }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningParserBaselineSupport:{"profileId":"firefox:default"}')
    expect(
      localizedBackupWarningText(
        warning({
          code: 'profile-search-terms-captured',
          profileId: 'chrome:Default',
          count: 1234,
        }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningProfileSearchTermsCapturedMany:{"count":"1,234","profileId":"chrome:Default"}',
    )
  })

  // Copy fix: these four warnings used to say "row(s)" / "URL(s)". They now
  // ship a singular/plural pair, so the count=1 side needs its own coverage.
  test('picks the singular warning copy when exactly one item was affected', () => {
    const t = (key: string, vars?: Record<string, string | number>) =>
      vars ? `${key}:${JSON.stringify(vars)}` : key
    const warning = (overrides: Partial<BackupWarning>): BackupWarning => ({
      code: 'og-cleanup-summary',
      message: 'English fallback.',
      ...overrides,
    })

    expect(
      localizedBackupWarningText(
        warning({ code: 'og-refetch-summary', count: 1, succeeded: 1 }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningOgRefetchSummaryOne:{"count":"1","succeeded":"1"}')
    expect(
      localizedBackupWarningText(
        warning({ code: 'og-prefetch-summary', count: 1, succeeded: 1 }),
        'raw',
        t,
        'en',
      ),
    ).toBe('audit.warningOgPrefetchSummaryOne:{"count":"1","succeeded":"1"}')
    expect(
      localizedBackupWarningText(
        warning({ count: 1, blobs: 1, bytes: 12 }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningOgCleanupSummaryOne:{"count":"1","blobs":"1","bytes":"12"}',
    )
    expect(
      localizedBackupWarningText(
        warning({
          code: 'profile-search-terms-captured',
          profileId: 'chrome:Default',
          count: 1,
        }),
        'raw',
        t,
        'en',
      ),
    ).toBe(
      'audit.warningProfileSearchTermsCapturedOne:{"count":"1","profileId":"chrome:Default"}',
    )
  })
})
