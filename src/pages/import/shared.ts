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

import type { ImportProgressEvent } from '../../lib/types'

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
