/**
 * This module renders a focused panel inside the Audit route.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `AuditRunDetailPanel`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { Link } from 'react-router-dom'
import {
  ReviewPathActionRow,
  ReviewSection,
  type ReviewCopyFeedback,
} from '../../../components/review'
import { PreviewEntryList } from '../../../components/ui'
import { StatusCallout } from '../../../components/primitives/status-callout'
import { backend } from '../../../lib/backend-client'
import { formatBytes, formatDateTime } from '../../../lib/format'
import type { ResolvedLanguage } from '../../../lib/i18n'
import {
  auditSeverityKey,
  importBatchStatusKey,
  importBatchStatusTone,
  runStatusKey,
  runTriggerKey,
  runTypeKey,
} from '../../../lib/trust-review'
import type {
  AuditRunDetail,
  ImportBatchDetail,
  ImportBatchOverview,
  SnapshotRestorePreview,
} from '../../../lib/types'
import type { AuditDetailTab, Translator } from '../types'

/**
 * Describes the props accepted by `AuditRunDetailPanel`.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface AuditRunDetailPanelProps {
  batchActionError: string | null
  batchActionNotice: string | null
  copyFeedback: ReviewCopyFeedback | null
  detail: AuditRunDetail
  detailSeverity: 'clear' | 'warning' | 'blocked' | null
  detailTab: AuditDetailTab
  handleCopyPath: (path: string) => Promise<void>
  handleExecuteRestore: () => Promise<void>
  handlePreviewRestore: (snapshotPath: string) => Promise<void>
  handleRelatedBatchMutation: (action: 'revert' | 'restore') => Promise<void>
  language: ResolvedLanguage
  loadingRelatedBatch: boolean
  relatedBatchDetail: ImportBatchDetail | null
  relatedBatchError: string | null
  relatedImportBatch: ImportBatchOverview | null
  restoreBusy: boolean
  restoreError: string | null
  restoreKindLabel: (kind: string) => string
  restoreNotice: string | null
  restorePreview: SnapshotRestorePreview | null
  setDetailTab: (tab: AuditDetailTab) => void
  t: Translator
}

/**
 * Renders the audit run detail panel.
 *
 * Keeping this as a named declaration makes the Audit surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function AuditRunDetailPanel({
  batchActionError,
  batchActionNotice,
  copyFeedback,
  detail,
  detailSeverity,
  detailTab,
  handleCopyPath,
  handleExecuteRestore,
  handlePreviewRestore,
  handleRelatedBatchMutation,
  language,
  loadingRelatedBatch,
  relatedBatchDetail,
  relatedBatchError,
  relatedImportBatch,
  restoreBusy,
  restoreError,
  restoreKindLabel,
  restoreNotice,
  restorePreview,
  setDetailTab,
  t,
}: AuditRunDetailPanelProps) {
  return (
    <div className="panel">
      <div className="panel-header">
        <span className="panel-title">
          {t('audit.manifestDetail', { runId: detail.run.id })}
        </span>
        {detailSeverity ? (
          <span className="panel-action">
            {t(auditSeverityKey(detailSeverity))}
          </span>
        ) : null}
      </div>
      <div className="panel-body">
        <div className="pme-tabs">
          {(
            [
              ['summary', t('audit.summaryTab')],
              ['artifacts', t('audit.artifactsTab')],
              ['warnings', t('audit.warningsTab')],
            ] as const
          ).map(([tab, label]) => (
            <button
              key={tab}
              className={`pme-tab ${detailTab === tab ? 'active' : ''}`}
              type="button"
              onClick={() => setDetailTab(tab)}
            >
              {label}
            </button>
          ))}
        </div>

        {detailTab === 'summary' ? (
          <>
            <StatusCallout
              tone="info"
              title={t('audit.reviewGuideTitle')}
              body={t('audit.reviewGuideBody')}
            />
            <div className="manifest-grid">
              <div className="manifest-field">
                <span className="field-label">{t('audit.runId')}</span>
                <span className="field-value mono">#{detail.run.id}</span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.runType')}</span>
                <span className="field-value">
                  {t(runTypeKey(detail.run.runType ?? 'backup'))}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('common.status')}</span>
                <span className="field-value">
                  {t(runStatusKey(detail.run.status))}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.runSource')}</span>
                <span className="field-value">
                  {detail.profileScope.join(' · ') || t('audit.archiveWide')}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.executedAt')}</span>
                <span className="field-value mono">
                  {formatDateTime(detail.run.startedAt, language) ??
                    detail.run.startedAt}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.triggerLabel')}</span>
                <span className="field-value">
                  {t(runTriggerKey(detail.trigger ?? detail.run.trigger))}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.manifestHash')}</span>
                <span className="field-value mono">
                  {detail.manifestHash ?? t('common.notAvailable')}
                </span>
              </div>
              <div className="manifest-field">
                <span className="field-label">{t('audit.manifestPath')}</span>
                <span className="field-value mono">
                  {detail.manifestPath ?? t('common.notAvailable')}
                </span>
              </div>
            </div>
            <div className="detail-divider" />
            <div className="manifest-stats">
              <div className="manifest-stat">
                <span className="dim">{t('audit.newVisits')}</span>
                <span className="mono accent">+{detail.run.newVisits}</span>
              </div>
              <div className="manifest-stat">
                <span className="dim">{t('audit.newUrls')}</span>
                <span className="mono">{detail.run.newUrls}</span>
              </div>
              <div className="manifest-stat">
                <span className="dim">{t('audit.downloads')}</span>
                <span className="mono">{detail.run.newDownloads}</span>
              </div>
              <div className="manifest-stat">
                <span className="dim">{t('audit.profiles')}</span>
                <span className="mono">{detail.run.profilesProcessed}</span>
              </div>
              {relatedBatchDetail ? (
                <>
                  <div className="manifest-stat">
                    <span className="dim">{t('audit.visibleRecords')}</span>
                    <span className="mono">
                      {relatedBatchDetail.batch.visibleItems}
                    </span>
                  </div>
                  <div className="manifest-stat">
                    <span className="dim">{t('audit.revertedRecords')}</span>
                    <span className="mono">
                      {Math.max(
                        0,
                        relatedBatchDetail.batch.importedItems -
                          relatedBatchDetail.batch.visibleItems,
                      )}
                    </span>
                  </div>
                </>
              ) : null}
            </div>
            <div className="detail-divider" />
            <div className="audit-review-section">
              <div className="audit-review-header">
                <span className="mono-kicker">
                  {t('audit.changedRecordsTitle')}
                </span>
                <span className="panel-action">
                  {relatedImportBatch
                    ? t('audit.importBatchLabel', {
                        id: String(relatedImportBatch.id),
                      })
                    : t('audit.changePreviewUnavailableShort')}
                </span>
              </div>
              <p className="dashboard-next-action">
                {t('audit.changedRecordsBody')}
              </p>
              {loadingRelatedBatch ? (
                <p className="dim">{t('common.loading')}</p>
              ) : relatedBatchError ? (
                <StatusCallout
                  tone="warning"
                  title={t('audit.importPreviewUnavailable')}
                  body={relatedBatchError}
                />
              ) : relatedBatchDetail ? (
                <>
                  <div className="manifest-grid">
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.candidateRows')}
                      </span>
                      <span className="field-value mono">
                        {relatedBatchDetail.batch.candidateItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.importedRows')}
                      </span>
                      <span className="field-value mono">
                        {relatedBatchDetail.batch.importedItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.duplicateRows')}
                      </span>
                      <span className="field-value mono">
                        {relatedBatchDetail.batch.duplicateItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('import.visibleRows')}
                      </span>
                      <span className="field-value mono">
                        {relatedBatchDetail.batch.visibleItems.toLocaleString(
                          language,
                        )}
                      </span>
                    </div>
                  </div>
                  <div className="detail-divider" />
                  <PreviewEntryList
                    entries={relatedBatchDetail.previewEntries}
                    language={language}
                    statusLabel={(status) => t(importBatchStatusKey(status))}
                    statusTone={importBatchStatusTone}
                  />
                  <div className="wizard-actions">
                    <Link
                      className="btn-secondary"
                      to={`/import?batch=${relatedBatchDetail.batch.id}`}
                    >
                      {t('audit.openImportReview')}
                    </Link>
                    {relatedBatchDetail.batch.auditPath ? (
                      <button
                        className="btn-secondary"
                        type="button"
                        onClick={() => {
                          void backend.openPathInFileManager(
                            relatedBatchDetail.batch.auditPath ?? '',
                          )
                        }}
                      >
                        {t('audit.openImportArtifact')}
                      </button>
                    ) : null}
                    <button
                      className="btn-secondary"
                      type="button"
                      onClick={() => {
                        void handleRelatedBatchMutation('revert')
                      }}
                      disabled={relatedBatchDetail.batch.status === 'reverted'}
                    >
                      {t('import.revertBatch')}
                    </button>
                    <button
                      className="btn-secondary"
                      type="button"
                      onClick={() => {
                        void handleRelatedBatchMutation('restore')
                      }}
                      disabled={relatedBatchDetail.batch.status !== 'reverted'}
                    >
                      {t('import.restoreBatch')}
                    </button>
                  </div>
                  <p className="mono-support">
                    {t(importBatchStatusKey(relatedBatchDetail.batch.status))}
                  </p>
                  {batchActionNotice ? (
                    <p className="mono-support" role="status">
                      {batchActionNotice}
                    </p>
                  ) : null}
                  {batchActionError ? (
                    <p className="inline-error" role="alert">
                      {batchActionError}
                    </p>
                  ) : null}
                </>
              ) : (
                <StatusCallout
                  tone="info"
                  title={t('audit.changePreviewUnavailableTitle')}
                  body={t('audit.changePreviewUnavailableBody')}
                />
              )}
            </div>
          </>
        ) : null}

        {detailTab === 'artifacts' ? (
          <div style={{ marginTop: 'var(--space-3)' }}>
            <span
              className="mono-kicker"
              style={{ marginBottom: 'var(--space-2)', display: 'block' }}
            >
              {t('audit.artifacts', { count: detail.artifacts.length })}
            </span>
            {detail.artifacts.length > 0 ? (
              detail.artifacts.map((artifact) => (
                <ReviewSection
                  key={`${artifact.kind}:${artifact.path}`}
                  className="audit-artifact-row"
                  headerMeta={
                    <span className="dim mono" style={{ fontSize: '10px' }}>
                      {formatBytes(artifact.sizeBytes ?? 0, language)}
                    </span>
                  }
                  title={
                    <span className="mono" style={{ fontSize: '11px' }}>
                      {artifact.kind}
                    </span>
                  }
                >
                  <ReviewPathActionRow
                    copyFeedback={copyFeedback}
                    copyKey={artifact.path}
                    copyLabel={t('common.copyAction')}
                    errorMessage={t('audit.copyFailed')}
                    label={t('common.filesLabel')}
                    onCopy={(_, value) => {
                      void handleCopyPath(value)
                    }}
                    onOpenPath={(path) => {
                      void backend.openPathInFileManager(path)
                    }}
                    openPathLabel={t('common.openAction')}
                    secondaryAction={
                      artifact.kind === 'snapshot' ? (
                        <button
                          className="btn-tiny"
                          type="button"
                          onClick={() => {
                            void handlePreviewRestore(artifact.path)
                          }}
                        >
                          {restoreBusy &&
                          restorePreview?.snapshotPath === artifact.path
                            ? t('common.loading')
                            : t('audit.previewRestore')}
                        </button>
                      ) : undefined
                    }
                    successMessage={t('audit.copied')}
                    value={artifact.path}
                  />
                </ReviewSection>
              ))
            ) : (
              <p className="dashboard-next-action">
                {t('common.notAvailable')}
              </p>
            )}
            {restorePreview ? (
              <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
                <div className="panel-header">
                  <span className="panel-title">
                    {t('audit.restorePreviewTitle')}
                  </span>
                  <span className="panel-action">
                    {restorePreview.executeSupported
                      ? t('audit.restoreReady')
                      : t('audit.restoreManualOnly')}
                  </span>
                </div>
                <div className="panel-body">
                  <p className="dashboard-next-action">
                    {t('audit.restorePreviewBody')}
                  </p>
                  <div className="manifest-grid">
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('audit.restoreKind')}
                      </span>
                      <span className="field-value">
                        {restoreKindLabel(restorePreview.snapshotKind)}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('audit.runSource')}
                      </span>
                      <span className="field-value mono">
                        {restorePreview.sourceProfileId ??
                          t('audit.archiveWide')}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('audit.executedAt')}
                      </span>
                      <span className="field-value mono">
                        {restorePreview.createdAt
                          ? (formatDateTime(
                              restorePreview.createdAt,
                              language,
                            ) ?? restorePreview.createdAt)
                          : t('common.notAvailable')}
                      </span>
                    </div>
                    <div className="manifest-field">
                      <span className="field-label">
                        {t('audit.restoreSnapshotPath')}
                      </span>
                      <span className="field-value mono">
                        {restorePreview.snapshotPath}
                      </span>
                    </div>
                  </div>
                  <div className="detail-divider" />
                  <div className="manifest-stats">
                    <div className="manifest-stat">
                      <span className="dim">{t('audit.estimatedVisits')}</span>
                      <span className="mono accent">
                        {restorePreview.estimatedVisits}
                      </span>
                    </div>
                    <div className="manifest-stat">
                      <span className="dim">{t('audit.estimatedUrls')}</span>
                      <span className="mono">
                        {restorePreview.estimatedUrls}
                      </span>
                    </div>
                    <div className="manifest-stat">
                      <span className="dim">
                        {t('audit.estimatedDownloads')}
                      </span>
                      <span className="mono">
                        {restorePreview.estimatedDownloads}
                      </span>
                    </div>
                  </div>
                  {restorePreview.warnings.length > 0 ? (
                    <div
                      className="warning-box"
                      style={{ marginTop: 'var(--space-3)' }}
                    >
                      <div className="warning-icon">⚠</div>
                      <div className="warning-text">
                        {restorePreview.warnings.map((warning) => (
                          <div key={warning}>{warning}</div>
                        ))}
                      </div>
                    </div>
                  ) : null}
                  <div
                    className="wizard-actions"
                    style={{ marginTop: 'var(--space-3)' }}
                  >
                    <button
                      className="btn-secondary"
                      type="button"
                      onClick={() => {
                        void backend.openPathInFileManager(
                          restorePreview.snapshotPath,
                        )
                      }}
                    >
                      {t('common.openAction')}
                    </button>
                    <button
                      className="btn-primary"
                      type="button"
                      disabled={!restorePreview.executeSupported || restoreBusy}
                      onClick={() => {
                        void handleExecuteRestore()
                      }}
                    >
                      {restoreBusy
                        ? t('common.loading')
                        : t('audit.executeRestore')}
                    </button>
                  </div>
                  {restoreNotice ? (
                    <p className="mono-support" role="status">
                      {restoreNotice}
                    </p>
                  ) : null}
                </div>
              </div>
            ) : null}
            {restoreError ? (
              <p className="inline-error" role="alert">
                {restoreError}
              </p>
            ) : null}
          </div>
        ) : null}

        {detailTab === 'warnings' ? (
          detail.warnings.length > 0 ? (
            <div
              className="warning-box"
              style={{ marginTop: 'var(--space-3)' }}
            >
              <div className="warning-icon">⚠</div>
              <div className="warning-text">
                {detail.warnings.map((warning) => (
                  <div key={warning}>{warning}</div>
                ))}
              </div>
            </div>
          ) : (
            <p
              className="dashboard-next-action"
              style={{ marginTop: 'var(--space-3)' }}
            >
              {t('audit.noWarnings')}
            </p>
          )
        ) : null}

        {detail.manifestPath ? (
          <div style={{ marginTop: 'var(--space-4)' }}>
            <ReviewPathActionRow
              copyFeedback={copyFeedback}
              copyKey={detail.manifestPath}
              copyLabel={t('audit.copyPath')}
              errorMessage={t('audit.copyFailed')}
              label={t('audit.manifestPath')}
              onCopy={(_, value) => {
                void handleCopyPath(value)
              }}
              onOpenPath={(path) => {
                void backend.openPathInFileManager(path)
              }}
              openPathLabel={t('audit.viewManifest')}
              successMessage={t('audit.copied')}
              value={detail.manifestPath}
            />
          </div>
        ) : null}
      </div>
    </div>
  )
}
