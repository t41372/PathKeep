/**
 * @file shared.test.ts
 * @description Guards the pure Import workflow and batch-selection helpers extracted from the route shell.
 * @module pages/import
 *
 * ## Responsibilities
 * - Verify Import batch deep-link parsing and selection fallback stay deterministic.
 * - Verify the workflow-step builder and active-batch fallback keep the same route contract.
 *
 * ## Not responsible for
 * - Re-testing route-level backend mutations or provider wiring.
 * - Rendering wizard or review panels.
 *
 * ## Dependencies
 * - Depends on the route-local helper module rather than the full Import route.
 *
 * ## Performance notes
 * - Pure helper coverage keeps the route-shell refactor honest without mounting the full route for every branch.
 */

import { describe, expect, test } from 'vitest'
import type {
  HealthReport,
  ImportBatchDetail,
  ImportBatchOverview,
  TakeoutInspection,
} from '../../lib/types'
import {
  buildImportWorkflowSteps,
  countTakeoutFilesByClassification,
  deriveActiveImportBatchDetail,
  groupTakeoutFileReports,
  parseImportBatchId,
  resolveSelectedImportBatchId,
} from './shared'

const recentBatches: ImportBatchOverview[] = [
  {
    id: 1,
    sourceKind: 'takeout',
    sourcePath: '/tmp/one',
    profileId: 'takeout::browser-history',
    createdAt: '2026-04-21T10:00:00.000Z',
    importedAt: '2026-04-21T10:01:00.000Z',
    revertedAt: null,
    status: 'imported',
    candidateItems: 2,
    importedItems: 2,
    duplicateItems: 0,
    visibleItems: 2,
    auditPath: '/tmp/one-audit.json',
    gitCommit: null,
  },
  {
    id: 2,
    sourceKind: 'takeout',
    sourcePath: '/tmp/two',
    profileId: 'takeout::browser-history',
    createdAt: '2026-04-21T11:00:00.000Z',
    importedAt: '2026-04-21T11:01:00.000Z',
    revertedAt: null,
    status: 'imported',
    candidateItems: 4,
    importedItems: 3,
    duplicateItems: 1,
    visibleItems: 3,
    auditPath: '/tmp/two-audit.json',
    gitCommit: null,
  },
]

const selectedBatchDetail: ImportBatchDetail = {
  batch: recentBatches[0],
  previewEntries: [],
  recognizedFiles: [],
  quarantinedFiles: [],
  notes: [],
  detectedLocale: 'en',
  previewRangeStart: '2026-04-21T09:00:00.000Z',
  previewRangeEnd: '2026-04-21T11:00:00.000Z',
}

const importResult: TakeoutInspection = {
  dryRun: false,
  sourcePath: '/tmp/three',
  recognizedFiles: [],
  quarantinedFiles: [],
  previewEntries: [],
  candidateItems: 5,
  importedItems: 4,
  duplicateItems: 1,
  notes: [],
  detectedLocale: 'en',
  previewRangeStart: '2026-04-21T09:00:00.000Z',
  previewRangeEnd: '2026-04-21T11:00:00.000Z',
  importBatch: {
    ...recentBatches[1],
    id: 3,
    sourcePath: '/tmp/three',
  },
}

const healthReport: HealthReport = {
  generatedAt: '2026-04-21T12:00:00.000Z',
  checks: [],
}

const t = (key: string) => key

describe('Import shared helpers', () => {
  test('parses import batch ids from search params safely', () => {
    expect(parseImportBatchId(new URLSearchParams('batch=7'))).toBe(7)
    expect(parseImportBatchId(new URLSearchParams('batch=0'))).toBeNull()
    expect(parseImportBatchId(new URLSearchParams('batch=nope'))).toBeNull()
    expect(parseImportBatchId(new URLSearchParams('range=month'))).toBeNull()
  })

  test('resolves the next selected batch id from deep links, current state, and recents', () => {
    expect(resolveSelectedImportBatchId(recentBatches, 2, null)).toBe(2)
    expect(resolveSelectedImportBatchId(recentBatches, 99, 1)).toBe(1)
    expect(resolveSelectedImportBatchId(recentBatches, null, 99)).toBe(1)
    expect(resolveSelectedImportBatchId([], 2, 1)).toBeNull()
  })

  test('prefers loaded batch detail but can fall back to a fresh import result', () => {
    expect(
      deriveActiveImportBatchDetail(selectedBatchDetail, importResult)?.batch
        .id,
    ).toBe(1)
    expect(deriveActiveImportBatchDetail(null, importResult)?.batch.id).toBe(3)
    expect(deriveActiveImportBatchDetail(null, null)).toBeNull()
  })

  test('builds workflow steps from the current import route state', () => {
    const steps = buildImportWorkflowSteps({
      activeBatchDetail: selectedBatchDetail,
      healthReport,
      importResult,
      inspection: {
        ...importResult,
        dryRun: true,
        importBatch: null,
        recognizedFiles: [
          {
            path: '/tmp/BrowserHistory.json',
            kind: 'browser-json',
            status: 'previewed',
            records: 4,
            classification: 'will-import',
            reasonCode: 'chrome-history-json',
            reasonDetail: null,
            detectedLocale: 'en',
          },
        ],
      },
      step: 'done',
      t,
    })

    expect(steps).toHaveLength(5)
    expect(steps[0]).toMatchObject({
      id: 'preview',
      status: 'complete',
      files: ['/tmp/BrowserHistory.json'],
    })
    expect(steps[1]).toMatchObject({
      id: 'manual',
      status: 'complete',
      checklist: [
        'import.manualLocateStep',
        'import.manualInspectStep',
        'import.manualContinueStep',
      ],
    })
    expect(steps[4]).toMatchObject({
      id: 'finish',
      status: 'complete',
    })
  })

  test('groups file reports by import disposition', () => {
    const groups = groupTakeoutFileReports([
      {
        path: '/tmp/BrowserHistory.json',
        kind: 'browser-json',
        status: 'previewed',
        records: 4,
        classification: 'will-import',
        reasonCode: 'chrome-history-json',
        reasonDetail: null,
        detectedLocale: 'en',
      },
      {
        path: '/tmp/My Activity.json',
        kind: 'chrome-activity',
        status: 'needs-review',
        records: 1,
        classification: 'needs-review',
        reasonCode: 'chrome-activity-outside-scope',
        reasonDetail: null,
        detectedLocale: 'en',
      },
    ])

    expect(groups).toHaveLength(2)
    expect(groups[0]).toMatchObject({ classification: 'will-import' })
    expect(groups[1]).toMatchObject({ classification: 'needs-review' })
    expect(
      countTakeoutFilesByClassification(
        groups.flatMap((group) => group.files),
        'needs-review',
      ),
    ).toBe(1)
  })
})
