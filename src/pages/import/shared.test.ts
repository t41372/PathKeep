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
  ImportProgressEvent,
  TakeoutFileReport,
  TakeoutInspection,
} from '../../lib/types'
import {
  buildImportWorkflowSteps,
  countTakeoutFilesByClassification,
  formatTakeoutPreviewRange,
  formatTakeoutLocaleLabel,
  deriveActiveImportBatchDetail,
  groupTakeoutFileReports,
  hasTakeoutReasonCode,
  importProgressValue,
  localizedImportNoteSummary,
  localizedImportProgressDetail,
  localizedImportProgressLabel,
  localizedImportProgressLogLines,
  parseImportBatchId,
  resolveSelectedImportBatchId,
  takeoutFileGroupBodyKey,
  takeoutFileGroupTitleKey,
  takeoutFileKindLabel,
  takeoutFileReasonLabel,
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

const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

const baseProgress: ImportProgressEvent = {
  phase: 'import-file',
  label: 'Importing',
  detail: 'fallback detail',
  current: 1,
  total: 4,
  progressPercent: 25,
  logLines: [],
  sourcePath: '/tmp/BrowserHistory.json',
  sourceLabel: null,
  processedRecords: null,
  totalRecords: null,
  importedRecords: null,
  duplicateRecords: null,
  skippedRecords: null,
}

function fileReport(
  overrides: Partial<TakeoutFileReport> = {},
): TakeoutFileReport {
  return {
    path: '/tmp/BrowserHistory.json',
    kind: 'browser-json',
    status: 'previewed',
    records: 4,
    classification: 'will-import',
    reasonCode: 'chrome-history-json',
    reasonDetail: null,
    detectedLocale: 'en',
    ...overrides,
  }
}

describe('Import shared helpers', () => {
  test('parses import batch ids from search params safely', () => {
    expect(parseImportBatchId(new URLSearchParams('batch=7'))).toBe(7)
    expect(parseImportBatchId(new URLSearchParams('batch=0'))).toBeNull()
    expect(parseImportBatchId(new URLSearchParams('batch=nope'))).toBeNull()
    expect(parseImportBatchId(new URLSearchParams('range=month'))).toBeNull()
  })

  test('resolves the next selected batch id from deep links and current state without auto-selecting history', () => {
    expect(resolveSelectedImportBatchId(recentBatches, 2, null)).toBe(2)
    expect(resolveSelectedImportBatchId(recentBatches, 99, 1)).toBe(1)
    expect(resolveSelectedImportBatchId(recentBatches, null, 99)).toBeNull()
    expect(resolveSelectedImportBatchId(recentBatches, null, null)).toBeNull()
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
        path: '/tmp/z-BrowserHistory.json',
        kind: 'browser-json',
        status: 'previewed',
        records: 4,
        classification: 'will-import',
        reasonCode: 'chrome-history-json',
        reasonDetail: null,
        detectedLocale: 'en',
      },
      {
        path: '/tmp/a-BrowserHistory.json',
        kind: 'browser-json',
        status: 'previewed',
        records: 2,
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
      {
        path: '/tmp/Bookmarks.json',
        kind: 'outside-scope',
        status: 'ignored',
        records: 0,
        classification: 'unsupported-shape',
        reasonCode: 'outside-chrome-scope',
        reasonDetail: null,
        detectedLocale: 'en',
      },
    ])

    expect(groups).toHaveLength(3)
    expect(groups[0]).toMatchObject({ classification: 'will-import' })
    expect(groups[0]?.files.map((file) => file.path)).toEqual([
      '/tmp/a-BrowserHistory.json',
      '/tmp/z-BrowserHistory.json',
    ])
    expect(groups[1]).toMatchObject({ classification: 'needs-review' })
    expect(groups[2]).toMatchObject({ classification: 'known-but-ignored' })
    expect(
      countTakeoutFilesByClassification(
        groups.flatMap((group) => group.files),
        'needs-review',
      ),
    ).toBe(1)
    expect(
      hasTakeoutReasonCode(
        groups.flatMap((group) => group.files),
        'chrome-activity-outside-scope',
      ),
    ).toBe(true)
    expect(
      countTakeoutFilesByClassification(
        groups.flatMap((group) => group.files),
        'known-but-ignored',
      ),
    ).toBe(1)
  })

  test('formats detected takeout locales including Chinese variants', () => {
    expect(formatTakeoutLocaleLabel('en', t)).toBe('import.localeEnglish')
    expect(formatTakeoutLocaleLabel('de', t)).toBe('import.localeGerman')
    expect(formatTakeoutLocaleLabel('zh-cn', t)).toBe(
      'import.localeChineseSimplified',
    )
    expect(formatTakeoutLocaleLabel('zh-tw', t)).toBe(
      'import.localeChineseTraditional',
    )
    expect(formatTakeoutLocaleLabel('mixed', t)).toBe('import.localeMixed')
    expect(formatTakeoutLocaleLabel(null, t)).toBe('import.localeUnknown')
  })

  test('localizes import progress details, labels, logs, and numeric progress', () => {
    expect(
      localizedImportProgressDetail(
        { ...baseProgress, phase: 'prepare', total: 12 },
        t,
        'en',
      ),
    ).toBe('import.importProgressPrepareDetail:{"files":"12"}')
    expect(
      localizedImportProgressDetail(
        {
          ...baseProgress,
          processedRecords: 1000,
          totalRecords: 2000,
          sourceLabel: 'BrowserHistory.json',
        },
        t,
        'en',
      ),
    ).toBe(
      'import.importProgressRecordDetail:{"processed":"1,000","total":"2,000","source":"BrowserHistory.json"}',
    )
    expect(
      localizedImportProgressDetail(
        {
          ...baseProgress,
          processedRecords: 1000,
          totalRecords: null,
          sourcePath: null,
        },
        t,
        'en',
      ),
    ).toBe(
      'import.importProgressRecordActiveDetail:{"processed":"1,000","source":""}',
    )
    expect(
      localizedImportProgressDetail(
        {
          ...baseProgress,
          progressPercent: null,
          sourceLabel: 'BrowserHistory.json',
        },
        t,
        'en',
      ),
    ).toBe(
      'import.importProgressImportActiveDetail:{"current":"1","total":"4","source":"BrowserHistory.json"}',
    )
    expect(
      localizedImportProgressDetail(
        { ...baseProgress, phase: 'finalize' },
        t,
        'en',
      ),
    ).toBe('import.importProgressFinalizeDetail')
    expect(
      localizedImportProgressDetail(
        { ...baseProgress, phase: 'complete' },
        t,
        'en',
      ),
    ).toBe('import.importProgressCompleteDetail')
    expect(
      localizedImportProgressDetail(
        { ...baseProgress, phase: 'custom', detail: 'raw backend detail' },
        t,
        'en',
      ),
    ).toBe('raw backend detail')

    expect(
      localizedImportProgressLabel(
        { ...baseProgress, processedRecords: 3, totalRecords: 6 },
        t,
        'en',
      ),
    ).toBe('import.importProgressRecordLabel:{"processed":"3","total":"6"}')
    expect(
      localizedImportProgressLabel(
        {
          ...baseProgress,
          processedRecords: 3,
          totalRecords: null,
          progressPercent: null,
        },
        t,
        'en',
      ),
    ).toBe('import.importProgressRecordActiveLabel:{"processed":"3"}')
    expect(
      localizedImportProgressLabel(
        { ...baseProgress, progressPercent: null },
        t,
        'en',
      ),
    ).toBe('import.importProgressActiveLabel:{"current":"1","total":"4"}')
    expect(localizedImportProgressLabel(baseProgress, t, 'en')).toBe('1 / 4')

    expect(
      localizedImportProgressLogLines(
        {
          ...baseProgress,
          importedRecords: 10,
          duplicateRecords: 2,
          skippedRecords: 3,
        },
        t,
        'en',
      ),
    ).toEqual([
      'import.importProgressImportDetail:{"current":"1","total":"4","source":"/tmp/BrowserHistory.json"}',
      'import.importProgressRecordStats:{"imported":"10","duplicates":"2"}',
      'import.importProgressSkippedRecords:{"count":"3"}',
    ])
    expect(
      localizedImportProgressLogLines(
        {
          ...baseProgress,
          importedRecords: null,
          duplicateRecords: 2,
          skippedRecords: 0,
        },
        t,
        'en',
      ),
    ).toEqual([
      'import.importProgressImportDetail:{"current":"1","total":"4","source":"/tmp/BrowserHistory.json"}',
      'import.importProgressRecordStats:{"imported":"0","duplicates":"2"}',
    ])
    expect(
      localizedImportProgressLogLines(
        {
          ...baseProgress,
          importedRecords: 10,
          duplicateRecords: null,
          skippedRecords: null,
        },
        t,
        'en',
      ),
    ).toEqual([
      'import.importProgressImportDetail:{"current":"1","total":"4","source":"/tmp/BrowserHistory.json"}',
      'import.importProgressRecordStats:{"imported":"10","duplicates":"0"}',
    ])
    expect(localizedImportProgressLogLines(baseProgress, t, 'en')).toEqual([
      'import.importProgressImportDetail:{"current":"1","total":"4","source":"/tmp/BrowserHistory.json"}',
    ])
    expect(importProgressValue(null)).toBeNull()
    expect(importProgressValue(baseProgress)).toBe(25)
    expect(
      importProgressValue({
        ...baseProgress,
        progressPercent: null,
        processedRecords: 7,
        totalRecords: 5,
      }),
    ).toBe(100)
    expect(
      importProgressValue({
        ...baseProgress,
        progressPercent: null,
        processedRecords: null,
        totalRecords: null,
      }),
    ).toBeNull()
    expect(localizedImportNoteSummary(1200, t, 'en')).toBe(
      'import.technicalNotesRecorded:{"count":"1,200"}',
    )
  })

  test('maps takeout group labels, file kinds, reasons, and preview ranges', () => {
    expect(takeoutFileGroupTitleKey('will-import')).toBe(
      'import.groupWillImportTitle',
    )
    expect(takeoutFileGroupTitleKey('known-but-ignored')).toBe(
      'import.groupIgnoredTitle',
    )
    expect(takeoutFileGroupTitleKey('needs-review')).toBe(
      'import.groupNeedsReviewTitle',
    )
    expect(takeoutFileGroupTitleKey('parse-error')).toBe(
      'import.groupParseErrorTitle',
    )
    expect(takeoutFileGroupBodyKey('will-import')).toBe(
      'import.groupWillImportBody',
    )
    expect(takeoutFileGroupBodyKey('known-but-ignored')).toBe(
      'import.groupIgnoredBody',
    )
    expect(takeoutFileGroupBodyKey('needs-review')).toBe(
      'import.groupNeedsReviewBody',
    )
    expect(takeoutFileGroupBodyKey('parse-error')).toBe(
      'import.groupParseErrorBody',
    )

    expect(takeoutFileKindLabel(fileReport({ kind: 'jsonl' }), t)).toBe(
      'import.kindJsonl',
    )
    expect(
      takeoutFileKindLabel(fileReport({ kind: 'typed-url-json' }), t),
    ).toBe('import.kindTypedUrl')
    expect(takeoutFileKindLabel(fileReport({ kind: 'session-json' }), t)).toBe(
      'import.kindSession',
    )
    expect(takeoutFileKindLabel(fileReport({ kind: 'takeout-index' }), t)).toBe(
      'import.kindTakeoutIndex',
    )
    expect(
      takeoutFileKindLabel(fileReport({ kind: 'chrome-activity' }), t),
    ).toBe('import.kindChromeActivity')
    expect(
      takeoutFileKindLabel(fileReport({ kind: 'chrome-supporting-file' }), t),
    ).toBe('import.kindChromeSupportingFile')
    expect(
      takeoutFileKindLabel(fileReport({ kind: 'unknown-history-like' }), t),
    ).toBe('import.kindHistoryLikeFile')
    expect(takeoutFileKindLabel(fileReport({ kind: 'outside-scope' }), t)).toBe(
      'import.kindOutsideScope',
    )
    expect(takeoutFileKindLabel(fileReport({ kind: 'custom-kind' }), t)).toBe(
      'custom-kind',
    )

    expect(
      takeoutFileReasonLabel(
        fileReport({ reasonCode: 'jsonl-history-fixture' }),
        t,
      ),
    ).toBe('import.reasonJsonlHistoryFixture')
    expect(
      takeoutFileReasonLabel(
        fileReport({ reasonCode: 'source-evidence-only' }),
        t,
      ),
    ).toBe('import.reasonSourceEvidenceOnly')
    expect(
      takeoutFileReasonLabel(fileReport({ reasonCode: 'takeout-index' }), t),
    ).toBe('import.reasonTakeoutIndex')
    expect(
      takeoutFileReasonLabel(
        fileReport({ reasonCode: 'chrome-my-activity-json' }),
        t,
      ),
    ).toBe('import.reasonChromeMyActivityJson')
    expect(
      takeoutFileReasonLabel(
        fileReport({ reasonCode: 'chrome-my-activity-html' }),
        t,
      ),
    ).toBe('import.reasonChromeMyActivityHtml')
    expect(
      takeoutFileReasonLabel(
        fileReport({ reasonCode: 'activity-outside-scope' }),
        t,
      ),
    ).toBe('import.reasonActivityOutsideScope')
    expect(
      takeoutFileReasonLabel(
        fileReport({ reasonCode: 'outside-chrome-scope' }),
        t,
      ),
    ).toBe('import.reasonOutsideChromeScope')
    expect(
      takeoutFileReasonLabel(
        fileReport({ reasonCode: 'chrome-supporting-file' }),
        t,
      ),
    ).toBe('import.reasonChromeSupportingFile')
    expect(
      takeoutFileReasonLabel(
        fileReport({ reasonCode: 'unrecognized-history-file' }),
        t,
      ),
    ).toBe('import.reasonUnrecognizedHistoryFile')
    expect(
      takeoutFileReasonLabel(
        fileReport({ reasonCode: 'parse-error', reasonDetail: 'bad json' }),
        t,
      ),
    ).toBe('bad json')
    expect(
      takeoutFileReasonLabel(fileReport({ reasonCode: 'parse-error' }), t),
    ).toBe('import.reasonParseError')
    expect(
      takeoutFileReasonLabel(
        fileReport({
          reasonCode: 'custom-reason',
          reasonDetail: 'custom detail',
        }),
        t,
      ),
    ).toBe('custom detail')
    expect(
      takeoutFileReasonLabel(fileReport({ reasonCode: 'custom-reason' }), t),
    ).toBe('')

    expect(formatTakeoutPreviewRange(null, '2026-04-21', 'en', t)).toBe(
      'import.rangeUnavailable',
    )
    expect(
      formatTakeoutPreviewRange(
        '2026-04-20T00:00:00.000Z',
        '2026-04-21T00:00:00.000Z',
        'en',
        t,
      ),
    ).toContain('2026')
  })
})
