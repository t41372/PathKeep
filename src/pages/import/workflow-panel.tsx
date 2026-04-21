/**
 * Import workflow explainer composition.
 *
 * ## 職責
 * - 渲染 Import route 的 workflow explainer，並組合實際的 wizard render module。
 * - 使用 route owner 傳入的 state / handlers，讓 Import route 可以把 workflow render 從 effect 與 mutation 邏輯中拆開。
 * - 保持 workflow honesty 與 wizard composition 都落在 route-local modules，而不是留在 mega-route。
 *
 * ## 不負責
 * - 不持有 deep-link、backend mutation、import progress subscription、或 batch selection state。
 * - 不決定檢查 / 修復 / revert / restore 等 follow-through 行為。
 * - 不定義跨 route 的 shared primitive 或 stylesheet owner。
 *
 * ## 依賴關係
 * - 依賴 `src/components/ui.tsx` 的 `OperationWorkflow` primitive。
 * - 依賴 `./wizard-panel.tsx` 渲染實際的 wizard body。
 * - 依賴 `./shared.ts` 的 route-local wizard types。
 *
 * ## 性能備注
 * - workflow explainer 本身只消費 route owner 已經整理好的 step data。
 * - 重工作仍留在 route owner 觸發的 backend / progress path。
 */

import { OperationWorkflow, type WorkflowStep } from '../../components/ui'
import { useI18n } from '../../lib/i18n'
import type { ResolvedLanguage } from '../../lib/i18n'
import type {
  BrowserProfile,
  ImportProgressEvent,
  TakeoutInspection,
} from '../../lib/types'
import {
  type ImportMethod,
  type ImportWizardStepDefinition,
  type WizardStep,
} from './shared'
import { ImportWizardPanel } from './wizard-panel'

/**
 * Props for the extracted Import workflow composition.
 */
export interface ImportWorkflowPanelProps {
  detectedBrowserProfiles: BrowserProfile[]
  importing: boolean
  importProgress: ImportProgressEvent | null
  importResult: TakeoutInspection | null
  inspection: TakeoutInspection | null
  language: ResolvedLanguage
  manualPathExpanded: boolean
  method: ImportMethod
  selectedBrowserProfile: BrowserProfile | null
  selectedBrowserProfileId: string | null
  sourcePath: string
  step: WizardStep
  stepIndex: number
  wizardSteps: ImportWizardStepDefinition[]
  workflowExpanded: boolean
  workflowSteps: WorkflowStep[]
  onBrowseSource: (options: { directory: boolean }) => void | Promise<void>
  onCopyWorkflowValue: (value: string) => Promise<void>
  onImport: () => void | Promise<void>
  onImportAnother: () => void
  onManualPathExpandedChange: (expanded: boolean) => void
  onMethodChange: (method: ImportMethod) => void
  onScan: () => void | Promise<void>
  onSelectBrowserProfile: (profile: BrowserProfile) => void
  onSourcePathChange: (path: string) => void
  onStepChange: (step: WizardStep) => void
  onWorkflowExpandedChange: (expanded: boolean) => void
}

/**
 * Renders the Import route's workflow explainer plus wizard module.
 */
export function ImportWorkflowPanel({
  detectedBrowserProfiles,
  importing,
  importProgress,
  importResult,
  inspection,
  language,
  manualPathExpanded,
  method,
  selectedBrowserProfile,
  selectedBrowserProfileId,
  sourcePath,
  step,
  stepIndex,
  wizardSteps,
  workflowExpanded,
  workflowSteps,
  onBrowseSource,
  onCopyWorkflowValue,
  onImport,
  onImportAnother,
  onManualPathExpandedChange,
  onMethodChange,
  onScan,
  onSelectBrowserProfile,
  onSourcePathChange,
  onStepChange,
  onWorkflowExpandedChange,
}: ImportWorkflowPanelProps) {
  const { t } = useI18n()

  return (
    <>
      <div className="panel">
        <div className="panel-header panel-header--toggle">
          <span className="panel-title">{t('import.workflowLabel')}</span>
          <button
            aria-expanded={workflowExpanded}
            className="btn-ghost"
            type="button"
            onClick={() => onWorkflowExpandedChange(!workflowExpanded)}
          >
            {workflowExpanded
              ? t('import.hideWorkflow')
              : t('import.showWorkflow')}
          </button>
        </div>
        {workflowExpanded ? (
          <div className="panel-body">
            <OperationWorkflow
              actionLabel={t('import.workflowLabel')}
              labels={{
                why: t('common.whyThisStepMatters'),
                files: t('common.filesLabel'),
                commands: t('common.commandsLabel'),
                checklist: t('common.checklistLabel'),
                copy: t('common.copyAction'),
                current: t('common.current'),
                complete: t('common.complete'),
                pending: t('common.pending'),
                command: (index) => t('common.commandStepLabel', { index }),
              }}
              onCopy={onCopyWorkflowValue}
              steps={workflowSteps}
            />
          </div>
        ) : (
          <div className="panel-body panel-body--compact">
            <p className="dashboard-next-action">
              {t('import.workflowCollapsedHint')}
            </p>
          </div>
        )}
      </div>

      <ImportWizardPanel
        detectedBrowserProfiles={detectedBrowserProfiles}
        importing={importing}
        importProgress={importProgress}
        importResult={importResult}
        inspection={inspection}
        language={language}
        manualPathExpanded={manualPathExpanded}
        method={method}
        selectedBrowserProfile={selectedBrowserProfile}
        selectedBrowserProfileId={selectedBrowserProfileId}
        sourcePath={sourcePath}
        step={step}
        stepIndex={stepIndex}
        wizardSteps={wizardSteps}
        onBrowseSource={onBrowseSource}
        onImport={onImport}
        onImportAnother={onImportAnother}
        onManualPathExpandedChange={onManualPathExpandedChange}
        onMethodChange={onMethodChange}
        onScan={onScan}
        onSelectBrowserProfile={onSelectBrowserProfile}
        onSourcePathChange={onSourcePathChange}
        onStepChange={onStepChange}
      />
    </>
  )
}
