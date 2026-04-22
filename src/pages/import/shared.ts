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
 * A valid requested batch wins first, then an existing still-valid selection,
 * and only then the newest available fallback batch.
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

  return recentBatches[0]?.id ?? null
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
      files: inspection?.recognizedFiles.map((file) => file.path),
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
) {
  switch (progress.phase) {
    case 'prepare':
      return t('import.importProgressPrepareDetail', {
        files: progress.total.toLocaleString(language),
      })
    case 'import-file':
      return t('import.importProgressImportDetail', {
        current: progress.current.toLocaleString(language),
        total: progress.total.toLocaleString(language),
        source: progress.sourcePath ?? '',
      })
    case 'finalize':
      return t('import.importProgressFinalizeDetail')
    case 'complete':
      return t('import.importProgressCompleteDetail')
    default:
      return progress.detail
  }
}
