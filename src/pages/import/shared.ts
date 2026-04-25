/**
 * Shared Import-route workflow types and small presentation helpers.
 *
 * ## 職責
 * - 定義 Import route 內部共用的 wizard / method 型別。
 * - 提供 workflow / progress UI 會共用的小型文案 helper。
 * - 讓 route owner 與 extracted render modules 共用同一套 route-local contract。
 *
 * ## 不負責
 * - 不持有 route state、effects、deep-link、或 backend mutation。
 * - 不渲染 Import route 的實際 UI。
 * - 不定義跨 route 的 shared primitive 或 transport contract。
 *
 * ## 依賴關係
 * - 依賴 `src/lib/types` 的 `ImportProgressEvent` 來對齊 progress payload shape。
 * - 由 Import route 或其 render modules 傳入翻譯函數與 locale。
 *
 * ## 性能備注
 * - helper 只做小型字串映射，不做資料查詢或大規模轉換。
 */

import type { WorkflowStep } from '../../components/review'
import type {
  HealthReport,
  ImportBatchDetail,
  ImportBatchOverview,
  ImportProgressEvent,
  TakeoutFileReport,
  TakeoutInspection,
} from '../../lib/types'

/**
 * Names the two Import entry flows that share one route but not one initial
 * selection experience.
 */
export type ImportMethod = 'takeout' | 'browser'

/**
 * Names the user-facing wizard steps used by the Import route.
 */
export type WizardStep = 'select' | 'scan' | 'preview' | 'confirm' | 'done'

/**
 * Defines the minimal step descriptor shared by the Import route and its
 * extracted workflow panel.
 */
export interface ImportWizardStepDefinition {
  key: WizardStep
  label: string
}

export type TakeoutFileClassification =
  | 'will-import'
  | 'known-but-ignored'
  | 'needs-review'
  | 'parse-error'

export interface TakeoutFileGroup {
  classification: TakeoutFileClassification
  files: TakeoutFileReport[]
}

/**
 * Translator shape shared by Import route helpers so workflow and review logic
 * can move out of the route shell without inventing a second text contract.
 */
export type ImportTranslate = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Parses the selected import-batch id from the route search params.
 *
 * Import follow-through uses `?batch=` deep links, so this helper keeps the
 * route shell and focused tests aligned on the same parsing rules.
 */
export function parseImportBatchId(searchParams: URLSearchParams) {
  const raw = searchParams.get('batch')
  if (!raw) {
    return null
  }

  const parsed = Number.parseInt(raw, 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null
}

/**
 * Chooses which recent import batch the route should review next.
 *
 * A valid requested batch wins first, followed by an existing still-valid
 * selection. The route no longer auto-picks the newest batch on first load,
 * so historical review only steals focus after an explicit deep link or click.
 */
export function resolveSelectedImportBatchId(
  recentBatches: ImportBatchOverview[],
  requestedBatchId: number | null,
  currentBatchId: number | null,
) {
  if (!recentBatches.length) {
    return null
  }

  if (requestedBatchId !== null) {
    const requestedBatch = recentBatches.find(
      (batch) => batch.id === requestedBatchId,
    )
    if (requestedBatch) {
      return requestedBatch.id
    }
  }

  if (currentBatchId !== null) {
    const currentBatch = recentBatches.find(
      (batch) => batch.id === currentBatchId,
    )
    if (currentBatch) {
      return currentBatch.id
    }
  }

  return null
}

/**
 * Builds the effective batch review detail shown by the follow-through panel.
 *
 * The route prefers the fully loaded persisted batch detail, but can fall back
 * to the just-finished import result so the UI stays honest before the fresh
 * preview round-trip completes.
 */
export function deriveActiveImportBatchDetail(
  selectedBatchDetail: ImportBatchDetail | null,
  importResult: TakeoutInspection | null,
) {
  if (selectedBatchDetail) {
    return selectedBatchDetail
  }

  if (!importResult?.importBatch) {
    return null
  }

  return {
    batch: importResult.importBatch,
    previewEntries: importResult.previewEntries,
    recognizedFiles: importResult.recognizedFiles,
    quarantinedFiles: importResult.quarantinedFiles,
    notes: importResult.notes,
  } satisfies ImportBatchDetail
}

/**
 * Builds the workflow explainer rows from the current Import route state.
 *
 * Keeping this pure prevents the route shell from re-embedding the same step
 * contract every time we adjust follow-through behavior.
 */
export function buildImportWorkflowSteps(args: {
  activeBatchDetail: ImportBatchDetail | null
  healthReport: HealthReport | null
  importResult: TakeoutInspection | null
  inspection: TakeoutInspection | null
  step: WizardStep
  t: ImportTranslate
}): WorkflowStep[] {
  const { activeBatchDetail, healthReport, importResult, inspection, step, t } =
    args

  return [
    {
      id: 'preview',
      title: t('import.workflowPreviewTitle'),
      status:
        step === 'preview' || step === 'confirm' || step === 'done'
          ? ('complete' as const)
          : ('pending' as const),
      summary: t('import.workflowPreviewSummary'),
      reason: t('import.workflowPreviewReason'),
      files: inspection?.recognizedFiles
        .filter(
          (file) =>
            normalizeTakeoutFileClassification(file.classification) ===
            'will-import',
        )
        .map((file) => file.path),
    },
    {
      id: 'manual',
      title: t('import.workflowManualTitle'),
      status:
        inspection !== null ? ('complete' as const) : ('pending' as const),
      summary: t('import.workflowManualSummary'),
      reason: t('import.workflowManualReason'),
      checklist: [
        t('import.manualLocateStep'),
        t('import.manualInspectStep'),
        t('import.manualContinueStep'),
      ],
    },
    {
      id: 'execute',
      title: t('import.workflowExecuteTitle'),
      status: step === 'done' ? ('complete' as const) : ('pending' as const),
      summary: t('import.workflowExecuteSummary'),
      reason: t('import.workflowExecuteReason'),
    },
    {
      id: 'verify',
      title: t('import.workflowVerifyTitle'),
      status:
        activeBatchDetail !== null || importResult !== null
          ? ('complete' as const)
          : ('pending' as const),
      summary: t('import.workflowVerifySummary'),
      reason: t('import.workflowVerifyReason'),
    },
    {
      id: 'finish',
      title: t('import.workflowFinishTitle'),
      status:
        activeBatchDetail !== null && healthReport !== null
          ? ('complete' as const)
          : ('pending' as const),
      summary: t('import.workflowFinishSummary'),
      reason: t('import.workflowFinishReason'),
    },
  ]
}

/**
 * Localizes the current import progress detail from the runtime progress
 * payload.
 *
 * The Import route shows the same underlying progress in multiple places, so
 * this helper keeps phase-specific wording stable across the workflow UI.
 */
export function localizedImportProgressDetail(
  progress: ImportProgressEvent,
  t: (key: string, vars?: Record<string, string | number>) => string,
  language: string,
  options: { expectedRecords?: number | null } = {},
) {
  const source = progress.sourceLabel ?? progress.sourcePath ?? ''
  const processedRecords = normalizedOptionalCount(progress.processedRecords)
  const totalRecords =
    normalizedOptionalCount(progress.totalRecords) ??
    normalizedOptionalCount(options.expectedRecords)

  switch (progress.phase) {
    case 'prepare':
      return t('import.importProgressPrepareDetail', {
        files: progress.total.toLocaleString(language),
      })
    case 'import-file':
      if (processedRecords !== null) {
        return totalRecords !== null
          ? t('import.importProgressRecordDetail', {
              processed: processedRecords.toLocaleString(language),
              total: totalRecords.toLocaleString(language),
              source,
            })
          : t('import.importProgressRecordActiveDetail', {
              processed: processedRecords.toLocaleString(language),
              source,
            })
      }

      return progress.progressPercent === null
        ? t('import.importProgressImportActiveDetail', {
            current: progress.current.toLocaleString(language),
            total: progress.total.toLocaleString(language),
            source,
          })
        : t('import.importProgressImportDetail', {
            current: progress.current.toLocaleString(language),
            total: progress.total.toLocaleString(language),
            source,
          })
    case 'finalize':
      return t('import.importProgressFinalizeDetail')
    case 'complete':
      return t('import.importProgressCompleteDetail')
    default:
      return progress.detail
  }
}

export function localizedImportProgressLabel(
  progress: ImportProgressEvent,
  t: (key: string, vars?: Record<string, string | number>) => string,
  language: string,
  options: { expectedRecords?: number | null } = {},
) {
  const processedRecords = normalizedOptionalCount(progress.processedRecords)
  const totalRecords =
    normalizedOptionalCount(progress.totalRecords) ??
    normalizedOptionalCount(options.expectedRecords)

  if (processedRecords !== null) {
    return totalRecords !== null
      ? t('import.importProgressRecordLabel', {
          processed: processedRecords.toLocaleString(language),
          total: totalRecords.toLocaleString(language),
        })
      : t('import.importProgressRecordActiveLabel', {
          processed: processedRecords.toLocaleString(language),
        })
  }

  if (progress.phase === 'import-file' && progress.progressPercent === null) {
    return t('import.importProgressActiveLabel', {
      current: progress.current.toLocaleString(language),
      total: progress.total.toLocaleString(language),
    })
  }

  return `${progress.current.toLocaleString(language)} / ${progress.total.toLocaleString(language)}`
}

export function localizedImportProgressLogLines(
  progress: ImportProgressEvent,
  t: (key: string, vars?: Record<string, string | number>) => string,
  language: string,
  options: { expectedRecords?: number | null } = {},
) {
  const lines = [localizedImportProgressDetail(progress, t, language, options)]
  const importedRecords = normalizedOptionalCount(progress.importedRecords)
  const duplicateRecords = normalizedOptionalCount(progress.duplicateRecords)
  const skippedRecords = normalizedOptionalCount(progress.skippedRecords)

  if (importedRecords !== null || duplicateRecords !== null) {
    lines.push(
      t('import.importProgressRecordStats', {
        imported: (importedRecords ?? 0).toLocaleString(language),
        duplicates: (duplicateRecords ?? 0).toLocaleString(language),
      }),
    )
  }

  if (skippedRecords !== null && skippedRecords > 0) {
    lines.push(
      t('import.importProgressSkippedRecords', {
        count: skippedRecords.toLocaleString(language),
      }),
    )
  }

  return lines
}

export function importProgressValue(
  progress: ImportProgressEvent | null,
  options: { expectedRecords?: number | null } = {},
) {
  if (!progress) {
    return null
  }

  if (
    progress.progressPercent !== null &&
    progress.progressPercent !== undefined
  ) {
    return progress.progressPercent
  }

  const processedRecords = normalizedOptionalCount(progress.processedRecords)
  const totalRecords =
    normalizedOptionalCount(progress.totalRecords) ??
    normalizedOptionalCount(options.expectedRecords)

  if (processedRecords === null || totalRecords === null) {
    return null
  }

  return Math.min(100, (processedRecords / totalRecords) * 100)
}

export function localizedImportNoteSummary(
  noteCount: number,
  t: (key: string, vars?: Record<string, string | number>) => string,
  language: string,
) {
  return t('import.technicalNotesRecorded', {
    count: noteCount.toLocaleString(language),
  })
}

function normalizedOptionalCount(value: number | null | undefined) {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
    ? value
    : null
}

export function groupTakeoutFileReports(
  reports: TakeoutFileReport[],
): TakeoutFileGroup[] {
  const groups = new Map<TakeoutFileClassification, TakeoutFileReport[]>()

  for (const report of reports) {
    const classification = normalizeTakeoutFileClassification(
      report.classification,
    )
    const existing = groups.get(classification) ?? []
    existing.push(report)
    groups.set(classification, existing)
  }

  return (
    ['will-import', 'needs-review', 'parse-error', 'known-but-ignored'] as const
  )
    .map((classification) => ({
      classification,
      files: [...(groups.get(classification) ?? [])].sort((left, right) =>
        left.path.localeCompare(right.path),
      ),
    }))
    .filter((group) => group.files.length > 0)
}

export function countTakeoutFilesByClassification(
  reports: TakeoutFileReport[],
  classification: TakeoutFileClassification,
) {
  return reports.filter(
    (report) =>
      normalizeTakeoutFileClassification(report.classification) ===
      classification,
  ).length
}

export function takeoutFileGroupTitleKey(
  classification: TakeoutFileClassification,
) {
  switch (classification) {
    case 'will-import':
      return 'import.groupWillImportTitle'
    case 'known-but-ignored':
      return 'import.groupIgnoredTitle'
    case 'needs-review':
      return 'import.groupNeedsReviewTitle'
    case 'parse-error':
      return 'import.groupParseErrorTitle'
  }
}

export function takeoutFileGroupBodyKey(
  classification: TakeoutFileClassification,
) {
  switch (classification) {
    case 'will-import':
      return 'import.groupWillImportBody'
    case 'known-but-ignored':
      return 'import.groupIgnoredBody'
    case 'needs-review':
      return 'import.groupNeedsReviewBody'
    case 'parse-error':
      return 'import.groupParseErrorBody'
  }
}

export function takeoutFileKindLabel(
  report: TakeoutFileReport,
  t: ImportTranslate,
) {
  switch (report.kind) {
    case 'jsonl':
      return t('import.kindJsonl')
    case 'browser-json':
      return t('import.kindBrowserHistory')
    case 'typed-url-json':
      return t('import.kindTypedUrl')
    case 'session-json':
      return t('import.kindSession')
    case 'takeout-index':
      return t('import.kindTakeoutIndex')
    case 'chrome-activity':
      return t('import.kindChromeActivity')
    case 'chrome-supporting-file':
      return t('import.kindChromeSupportingFile')
    case 'unknown-history-like':
      return t('import.kindHistoryLikeFile')
    case 'outside-scope':
      return t('import.kindOutsideScope')
    default:
      return report.kind
  }
}

export function takeoutFileReasonLabel(
  report: TakeoutFileReport,
  t: ImportTranslate,
) {
  switch (report.reasonCode) {
    case 'chrome-history-json':
      return t('import.reasonChromeHistoryJson')
    case 'jsonl-history-fixture':
      return t('import.reasonJsonlHistoryFixture')
    case 'source-evidence-only':
      return t('import.reasonSourceEvidenceOnly')
    case 'takeout-index':
      return t('import.reasonTakeoutIndex')
    case 'chrome-activity-outside-scope':
      return t('import.reasonChromeActivityOutsideScope')
    case 'chrome-my-activity-json':
      return t('import.reasonChromeMyActivityJson')
    case 'chrome-my-activity-html':
      return t('import.reasonChromeMyActivityHtml')
    case 'activity-outside-scope':
      return t('import.reasonActivityOutsideScope')
    case 'outside-chrome-scope':
      return t('import.reasonOutsideChromeScope')
    case 'chrome-supporting-file':
      return t('import.reasonChromeSupportingFile')
    case 'unrecognized-history-file':
      return t('import.reasonUnrecognizedHistoryFile')
    case 'parse-error':
      return report.reasonDetail ?? t('import.reasonParseError')
    default:
      return report.reasonDetail ?? ''
  }
}

export function formatTakeoutLocaleLabel(
  locale: string | null | undefined,
  t: ImportTranslate,
) {
  switch (locale) {
    case 'en':
      return t('import.localeEnglish')
    case 'de':
      return t('import.localeGerman')
    case 'zh-cn':
      return t('import.localeChineseSimplified')
    case 'zh-tw':
      return t('import.localeChineseTraditional')
    case 'mixed':
      return t('import.localeMixed')
    default:
      return t('import.localeUnknown')
  }
}

export function hasTakeoutReasonCode(
  reports: TakeoutFileReport[],
  reasonCode: string,
) {
  return reports.some((report) => report.reasonCode === reasonCode)
}

export function formatTakeoutPreviewRange(
  start: string | null | undefined,
  end: string | null | undefined,
  language: string,
  t: ImportTranslate,
) {
  if (!start || !end) {
    return t('import.rangeUnavailable')
  }

  const formatter = new Intl.DateTimeFormat(language, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  })
  return `${formatter.format(new Date(start))} - ${formatter.format(
    new Date(end),
  )}`
}

function normalizeTakeoutFileClassification(
  classification: string | null | undefined,
): TakeoutFileClassification {
  switch (classification) {
    case 'will-import':
    case 'known-but-ignored':
    case 'needs-review':
    case 'parse-error':
      return classification
    default:
      return 'known-but-ignored'
  }
}
