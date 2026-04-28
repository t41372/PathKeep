/**
 * Import follow-through review panels.
 *
 * ## 職責
 * - 渲染目前匯入結果、recent batches、doctor/repair review 三塊 follow-through UI。
 * - 把「剛完成的導入」和「歷史批次 / 維護工具」分成不同層級，避免舊批次搶走首屏焦點。
 * - 維持 audit-path support actions、batch revert/restore、與 health callout 的現有展示契約。
 *
 * ## 不負責
 * - 不持有 batch deep-link、backend doctor/repair calls、或 revert/restore mutation。
 * - 不決定 import wizard 的 step / progress / source selection。
 * - 不新增新的 support-action contract；沿用既有 review-layer owner。
 *
 * ## 依賴關係
 * - 依賴 `src/components/review/ImportBatchReview` 的 batch review contract。
 * - 依賴 `src/components/primitives/status-callout.tsx` 的共享 callout grammar。
 * - 依賴 route owner 傳入的 callbacks 來觸發 doctor、repair、batch mutation、copy、與 open-path。
 *
 * ## 性能備注
 * - 只渲染 route owner 已經拿到的 recent batches / selected batch / doctor result；不做額外 fan-out。
 */

import {
  ImportBatchReview,
  type ReviewCopyFeedback,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { useI18n } from '../../lib/i18n'
import type { ResolvedLanguage } from '../../lib/i18n'
import {
  healthCheckStatusKey,
  healthCheckStatusTone,
  importBatchStatusKey,
} from '../../lib/trust-review'
import type {
  HealthReport,
  ImportBatchDetail,
  ImportBatchOverview,
} from '../../lib/types'
import { formatTakeoutLocaleLabel, formatTakeoutPreviewRange } from './shared'

/**
 * Props for the extracted Import review panels.
 */
export interface ImportReviewPanelsProps {
  activeBatchDetail: ImportBatchDetail | null
  healthReport: HealthReport | null
  historyExpanded: boolean
  language: ResolvedLanguage
  loadingBatch: boolean
  recentImportBatches: ImportBatchOverview[] | null | undefined
  repairNotice: string | null
  selectedBatchId: number | null
  supportCopyFeedback: ReviewCopyFeedback | null
  onBatchMutation: (
    batch: ImportBatchOverview,
    action: 'revert' | 'restore',
  ) => void | Promise<void>
  onCopyPath: (key: string, value: string) => Promise<void>
  onHistoryExpandedChange: (expanded: boolean) => void
  onOpenPath: (path: string) => void
  onRepairHealth: () => void | Promise<void>
  onRunDoctor: () => void | Promise<void>
  onSelectBatch: (batchId: number) => void
}

/**
 * Renders the Import route's batch review and health follow-through panels.
 */
export function ImportReviewPanels({
  activeBatchDetail,
  healthReport,
  historyExpanded,
  language,
  loadingBatch,
  recentImportBatches,
  repairNotice,
  selectedBatchId,
  supportCopyFeedback,
  onBatchMutation,
  onCopyPath,
  onHistoryExpandedChange,
  onOpenPath,
  onRepairHealth,
  onRunDoctor,
  onSelectBatch,
}: ImportReviewPanelsProps) {
  const { t } = useI18n()
  const recentBatches = recentImportBatches ?? []

  return (
    <div className="import-review-stack">
      {activeBatchDetail ? (
        <div className="panel import-review-primary">
          <div className="panel-header">
            <span className="panel-title">{t('import.batchReviewTitle')}</span>
          </div>
          <div className="panel-body">
            <p className="dashboard-next-action">
              {t('import.batchReviewBody')}
            </p>
            <div className="import-batch-meta">
              <span className="status-badge">
                {t('import.detectedLocaleLabel')}:{' '}
                {formatTakeoutLocaleLabel(activeBatchDetail.detectedLocale, t)}
              </span>
              <span className="mono-support">
                {t('import.timeRangeLabel')}:{' '}
                {formatTakeoutPreviewRange(
                  activeBatchDetail.previewRangeStart,
                  activeBatchDetail.previewRangeEnd,
                  language,
                  t,
                )}
              </span>
            </div>
            <ImportBatchReview
              auditPathActions={{
                copyFeedback: supportCopyFeedback,
                copyKey: `import:audit:${activeBatchDetail.batch.id}`,
                copyLabel: t('common.copyAction'),
                errorMessage: t('audit.copyFailed'),
                label: t('audit.manifestPath'),
                onCopy: (key, value) => {
                  void onCopyPath(key, value)
                },
                onOpenPath,
                openPathLabel: t('common.openAction'),
                successMessage: t('common.copiedNotice'),
              }}
              batchDetail={activeBatchDetail}
              language={language}
              metricLabels={{
                candidateRows: t('import.candidateRows'),
                duplicateRows: t('import.duplicateRows'),
                importedRows: t('import.importedRows'),
                visibleRows: t('import.visibleRows'),
              }}
              noPreviewEntriesLabel={t('import.noPreviewRows')}
              previewStatusLabel={(status) => t(importBatchStatusKey(status))}
            />
            <div className="wizard-actions">
              <button
                className="btn-secondary"
                type="button"
                onClick={() => {
                  void onBatchMutation(activeBatchDetail.batch, 'revert')
                }}
                disabled={activeBatchDetail.batch.status === 'reverted'}
                aria-disabled={activeBatchDetail.batch.status === 'reverted'}
              >
                {t('import.revertBatch')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={() => {
                  void onBatchMutation(activeBatchDetail.batch, 'restore')
                }}
                disabled={activeBatchDetail.batch.status !== 'reverted'}
                aria-disabled={activeBatchDetail.batch.status !== 'reverted'}
              >
                {t('import.restoreBatch')}
              </button>
            </div>
          </div>
        </div>
      ) : loadingBatch ? (
        <div className="panel import-review-primary">
          <div className="panel-header">
            <span className="panel-title">{t('import.batchReviewTitle')}</span>
          </div>
          <div className="panel-body panel-body--compact">
            <p className="dashboard-next-action">{t('common.loading')}</p>
          </div>
        </div>
      ) : null}

      <div className="panel import-review-secondary">
        <div className="panel-header panel-header--toggle">
          <span className="panel-title">{t('import.historyToolsTitle')}</span>
          <button
            aria-expanded={historyExpanded}
            className="btn-ghost"
            type="button"
            onClick={() => onHistoryExpandedChange(!historyExpanded)}
          >
            {historyExpanded
              ? t('import.hideHistoryTools')
              : t('import.showHistoryTools')}
          </button>
        </div>
        {historyExpanded ? (
          <div className="panel-body">
            <p className="dashboard-next-action">
              {t('import.historyToolsBody')}
            </p>
            <div className="dashboard-grid">
              <div className="dashboard-left">
                <div className="panel panel--nested">
                  <div className="panel-header">
                    <span className="panel-title">
                      {t('import.recentBatches')}
                    </span>
                  </div>
                  <div className="panel-body">
                    <p className="dashboard-next-action">
                      {t('import.recentBatchesBody')}
                    </p>
                    {recentBatches.length === 0 ? (
                      <p className="dim">{t('import.noImportBatches')}</p>
                    ) : (
                      <div className="result-list">
                        {recentBatches.map((batch) => (
                          <button
                            key={batch.id}
                            className={`result-row ${
                              selectedBatchId === batch.id
                                ? 'result-row--active'
                                : ''
                            }`}
                            type="button"
                            onClick={() => onSelectBatch(batch.id)}
                          >
                            <div className="result-row__header">
                              <strong>
                                {t('import.batchIdLabel', {
                                  id: String(batch.id),
                                })}
                              </strong>
                              <span className="status-badge">
                                {t(importBatchStatusKey(batch.status))}
                              </span>
                            </div>
                            <p
                              className="mono"
                              style={{
                                overflow: 'hidden',
                                textOverflow: 'ellipsis',
                                whiteSpace: 'nowrap',
                              }}
                              title={batch.sourcePath}
                            >
                              {batch.sourcePath}
                            </p>
                            <div className="result-row__meta dim">
                              <span>
                                {t('import.importedRows')}:{' '}
                                {batch.importedItems}
                              </span>
                              <span>
                                {t('import.visibleRows')}: {batch.visibleItems}
                              </span>
                            </div>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              </div>

              <div className="dashboard-right">
                <div className="panel panel--nested">
                  <div className="panel-header">
                    <span className="panel-title">
                      {t('import.healthReport')}
                    </span>
                  </div>
                  <div className="panel-body">
                    <p className="dashboard-next-action">
                      {t('import.healthReportBody')}
                    </p>
                    <div className="wizard-actions">
                      <button
                        className="btn-secondary"
                        type="button"
                        onClick={() => {
                          void onRunDoctor()
                        }}
                      >
                        {t('import.runHealthCheckAction')}
                      </button>
                      <button
                        className="btn-secondary"
                        type="button"
                        disabled={!healthReport}
                        aria-disabled={!healthReport}
                        onClick={() => {
                          void onRepairHealth()
                        }}
                      >
                        {t('common.repairAction')}
                      </button>
                    </div>
                    <p className="dim" style={{ marginTop: 'var(--space-2)' }}>
                      {t('import.repairDescription')}
                    </p>
                    {healthReport ? (
                      <div
                        className="manual-steps"
                        style={{ marginTop: 'var(--space-4)' }}
                      >
                        {healthReport.checks.map((check) => (
                          <StatusCallout
                            key={check.name}
                            tone={healthCheckStatusTone(check.status)}
                            title={`${t(healthCheckStatusKey(check.status))} — ${check.name}`}
                            body={check.message}
                          />
                        ))}
                      </div>
                    ) : (
                      <p className="dim">{t('import.noHealthChecks')}</p>
                    )}
                    {repairNotice ? (
                      <p className="mono-support" role="status">
                        {repairNotice}
                      </p>
                    ) : null}
                  </div>
                </div>
              </div>
            </div>
          </div>
        ) : (
          <div className="panel-body panel-body--compact">
            <p className="dashboard-next-action">
              {t('import.historyToolsCollapsedHint')}
            </p>
          </div>
        )}
      </div>
    </div>
  )
}
