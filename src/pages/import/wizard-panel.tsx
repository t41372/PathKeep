/**
 * Import wizard render module.
 *
 * ## 職責
 * - 渲染 Import route 的 stepper 與 scan/preview/confirm/done 步驟內容。
 * - 組合專門的 select-step render module，避免同一檔案再次長成 mega-route。
 * - 接收 route owner 提供的 state / handlers，保持 wizard 呈現與 mutation 邏輯分離。
 *
 * ## 不負責
 * - 不持有 workflow explainer 的展開狀態或 OperationWorkflow content。
 * - 不管理 batch review、doctor/repair、或 selected batch follow-through。
 * - 不直接做 backend import / inspect / subscribe side effects。
 *
 * ## 依賴關係
 * - 依賴 `src/components/review/preview-entry-list.tsx` 的 `PreviewEntryList`。
 * - 依賴 `src/components/primitives/busy-overlay.tsx` 的 progress overlay。
 * - 依賴 `./select-step.tsx` 與 `./shared.ts` 的 route-local wizard helpers。
 *
 * ## 性能備注
 * - 只渲染 route owner 已有的 wizard / preview state；重工作仍留在 backend 與 progress stream。
 */

import { BusyOverlay } from '../../components/primitives/busy-overlay'
import { PreviewEntryList } from '../../components/review'
import { useI18n } from '../../lib/i18n'
import type { ResolvedLanguage } from '../../lib/i18n'
import {
  importBatchStatusKey,
  importBatchStatusTone,
} from '../../lib/trust-review'
import type {
  BrowserProfile,
  ImportProgressEvent,
  TakeoutInspection,
} from '../../lib/types'
import { ImportSelectStep } from './select-step'
import {
  type ImportMethod,
  type ImportWizardStepDefinition,
  localizedImportProgressDetail,
  type WizardStep,
} from './shared'

/**
 * Props for the extracted Import wizard panel.
 */
export interface ImportWizardPanelProps {
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
  onBrowseSource: (options: { directory: boolean }) => void | Promise<void>
  onImport: () => void | Promise<void>
  onImportAnother: () => void
  onManualPathExpandedChange: (expanded: boolean) => void
  onMethodChange: (method: ImportMethod) => void
  onScan: () => void | Promise<void>
  onSelectBrowserProfile: (profile: BrowserProfile) => void
  onSourcePathChange: (path: string) => void
  onStepChange: (step: WizardStep) => void
}

/**
 * Renders the Import route's wizard steps.
 */
export function ImportWizardPanel({
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
  onBrowseSource,
  onImport,
  onImportAnother,
  onManualPathExpandedChange,
  onMethodChange,
  onScan,
  onSelectBrowserProfile,
  onSourcePathChange,
  onStepChange,
}: ImportWizardPanelProps) {
  const { t } = useI18n()

  return (
    <div className="import-container">
      <div className="wizard-panel">
        <div className="wizard-steps">
          {wizardSteps.map((wizardStep, index) => (
            <div key={wizardStep.key} style={{ display: 'contents' }}>
              {index > 0 && (
                <div
                  className={`wizard-step-line ${
                    index <= stepIndex
                      ? 'completed'
                      : index === stepIndex + 1
                        ? 'active'
                        : ''
                  }`}
                />
              )}
              <div
                aria-current={index === stepIndex ? 'step' : undefined}
                className={`wizard-step ${
                  index < stepIndex
                    ? 'completed'
                    : index === stepIndex
                      ? 'active-step'
                      : ''
                }`}
              >
                <div className="step-number">{index + 1}</div>
                <div className="step-label">{wizardStep.label}</div>
              </div>
            </div>
          ))}
        </div>

        <div className="wizard-body">
          {step === 'select' && (
            <ImportSelectStep
              detectedBrowserProfiles={detectedBrowserProfiles}
              language={language}
              manualPathExpanded={manualPathExpanded}
              method={method}
              selectedBrowserProfile={selectedBrowserProfile}
              selectedBrowserProfileId={selectedBrowserProfileId}
              sourcePath={sourcePath}
              onBrowseSource={onBrowseSource}
              onManualPathExpandedChange={onManualPathExpandedChange}
              onMethodChange={onMethodChange}
              onScan={onScan}
              onSelectBrowserProfile={onSelectBrowserProfile}
              onSourcePathChange={onSourcePathChange}
            />
          )}

          {step === 'scan' && (
            <div style={{ position: 'relative', minHeight: '120px' }}>
              <BusyOverlay
                label={t('import.scanningTitle')}
                detail={t('import.workflowPreviewReason')}
                progressLabel={`2 / ${wizardSteps.length.toLocaleString(language)}`}
                progressValue={(2 / wizardSteps.length) * 100}
                steps={wizardSteps
                  .slice(0, 3)
                  .map((wizardStep) => wizardStep.label)}
                activeStep={1}
              />
            </div>
          )}

          {step === 'preview' && inspection && (
            <>
              <div className="wizard-title">{t('import.previewTitle')}</div>
              <div className="wizard-description dim">
                {t('import.previewBody')}
              </div>

              <div className="preview-stats">
                <div className="preview-stat">
                  <div className="preview-stat-label">
                    {t('import.recordsFound')}
                  </div>
                  <div className="preview-stat-value mono">
                    {inspection.candidateItems.toLocaleString(language)}
                  </div>
                </div>
                <div className="preview-stat">
                  <div className="preview-stat-label">
                    {t('import.duplicates')}
                  </div>
                  <div className="preview-stat-value mono">
                    {inspection.duplicateItems.toLocaleString(language)}
                  </div>
                </div>
                <div className="preview-stat">
                  <div className="preview-stat-label">
                    {t('import.newRecords')}
                  </div>
                  <div className="preview-stat-value mono accent">
                    {(
                      inspection.candidateItems - inspection.duplicateItems
                    ).toLocaleString(language)}
                  </div>
                </div>
              </div>

              {inspection.recognizedFiles.length > 0 && (
                <div className="preview-files">
                  <div
                    className="panel-header"
                    style={{ marginTop: 'var(--space-4)' }}
                  >
                    <span className="panel-title">
                      {t('import.detectedFiles')}
                    </span>
                  </div>
                  {inspection.recognizedFiles.map((file) => (
                    <div key={file.path} className="file-item">
                      <span
                        className="file-status ok"
                        aria-label={t('common.statusSuccess')}
                      >
                        ✓
                      </span>
                      <span
                        className="file-name mono"
                        title={file.path}
                        style={{
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        {file.path}
                      </span>
                      <span className="file-detail dim">
                        {file.records.toLocaleString(language)} · {file.kind}
                      </span>
                    </div>
                  ))}
                </div>
              )}

              {inspection.quarantinedFiles.length > 0 && (
                <div className="preview-files">
                  <div
                    className="panel-header"
                    style={{ marginTop: 'var(--space-4)' }}
                  >
                    <span className="panel-title">
                      {t('import.quarantinedFiles')}
                    </span>
                  </div>
                  {inspection.quarantinedFiles.map((file) => (
                    <div key={file.path} className="file-item">
                      <span
                        className="file-status warn"
                        aria-label={t('common.warning')}
                      >
                        ⚠
                      </span>
                      <span
                        className="file-name mono"
                        title={file.path}
                        style={{
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        {file.path}
                      </span>
                      <span className="file-detail dim">{file.status}</span>
                    </div>
                  ))}
                </div>
              )}

              {inspection.previewEntries.length > 0 ? (
                <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
                  <div className="panel-header">
                    <span className="panel-title">
                      {t('import.previewRows')}
                    </span>
                  </div>
                  <div className="panel-body">
                    <PreviewEntryList
                      entries={inspection.previewEntries}
                      language={language}
                      statusLabel={(status) => t(importBatchStatusKey(status))}
                      statusTone={importBatchStatusTone}
                    />
                  </div>
                </div>
              ) : null}

              {inspection.notes.length > 0 && (
                <div className="inline-note-list dim">
                  {inspection.notes.map((note) => (
                    <div key={note}>{note}</div>
                  ))}
                </div>
              )}

              <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
                <div className="panel-header">
                  <span className="panel-title">
                    {t('import.confirmSummaryTitle')}
                  </span>
                </div>
                <div className="panel-body">
                  <p className="dim">{t('import.confirmSummaryBody')}</p>
                  <div className="manifest-grid">
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.confirmSummaryNewRecords')}
                      </span>
                      <span className="field-value mono accent">
                        {(
                          inspection.candidateItems - inspection.duplicateItems
                        ).toLocaleString(language)}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.confirmSummaryDuplicates')}
                      </span>
                      <span className="field-value mono">
                        {inspection.duplicateItems.toLocaleString(language)}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.confirmSummaryFiles')}
                      </span>
                      <span className="field-value mono">
                        {inspection.recognizedFiles.length.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                  </div>
                </div>
              </div>

              <div className="wizard-actions">
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => onStepChange('select')}
                  disabled={importing}
                >
                  {t('import.backAction')}
                </button>
                <button
                  className="btn-primary"
                  type="button"
                  onClick={() => {
                    void onImport()
                  }}
                  disabled={importing}
                >
                  {t('import.confirmImport')}
                </button>
              </div>
            </>
          )}

          {step === 'confirm' && importing && (
            <div style={{ position: 'relative', minHeight: '120px' }}>
              <BusyOverlay
                label={t('import.importingTitle')}
                detail={
                  importProgress
                    ? localizedImportProgressDetail(importProgress, t, language)
                    : t('import.importingProgressDetail', {
                        records: (
                          inspection?.candidateItems ?? 0
                        ).toLocaleString(language),
                        files: (
                          inspection?.recognizedFiles.length ?? 0
                        ).toLocaleString(language),
                      })
                }
                logLines={
                  importProgress
                    ? [
                        localizedImportProgressDetail(
                          importProgress,
                          t,
                          language,
                        ),
                      ]
                    : []
                }
                progressLabel={
                  importProgress
                    ? `${importProgress.current.toLocaleString(language)} / ${importProgress.total.toLocaleString(language)}`
                    : `4 / ${wizardSteps.length.toLocaleString(language)}`
                }
                progressValue={
                  importProgress?.progressPercent ??
                  (4 / wizardSteps.length) * 100
                }
                steps={wizardSteps
                  .slice(2)
                  .map((wizardStep) => wizardStep.label)}
                activeStep={1}
              />
            </div>
          )}

          {step === 'done' && importResult && (
            <>
              <div className="wizard-title">{t('import.completeTitle')}</div>
              <div className="wizard-description dim">
                {t('import.completeBody')}
              </div>
              <div className="preview-stats">
                <div className="preview-stat">
                  <div className="preview-stat-label">
                    {t('import.imported')}
                  </div>
                  <div className="preview-stat-value mono accent">
                    {importResult.importedItems.toLocaleString(language)}
                  </div>
                </div>
                <div className="preview-stat">
                  <div className="preview-stat-label">
                    {t('import.duplicatesSkipped')}
                  </div>
                  <div className="preview-stat-value mono">
                    {importResult.duplicateItems.toLocaleString(language)}
                  </div>
                </div>
              </div>
              <div className="wizard-actions">
                <button
                  className="btn-primary"
                  type="button"
                  onClick={onImportAnother}
                >
                  {t('import.importAnother')}
                </button>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
