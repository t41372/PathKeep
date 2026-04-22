/**
 * @file import-batch-review.tsx
 * @description Shared loaded-state review body for import batches, reused by Import and Audit follow-through surfaces.
 * @module components/review
 *
 * ## Responsibilities
 * - Render import-batch manifest metrics in one canonical layout.
 * - Render preview-entry evidence and optional audit-path actions for an already loaded batch.
 * - Keep Import and Audit aligned on how import-batch review content is presented.
 *
 * ## Not responsible for
 * - Loading batch detail or deciding loading, empty, or error states.
 * - Executing revert, restore, or route-navigation mutations.
 * - Owning route-specific notices, status footers, or action rows outside the shared review body.
 *
 * ## Dependencies
 * - Depends on `PreviewEntryList` for imported row evidence.
 * - Depends on `ReviewPathActionRow` for copy/open-path grammar when an audit artifact path exists.
 * - Depends on callers to provide localized labels and route-specific callbacks.
 *
 * ## Performance notes
 * - Pure render-only composition for an already loaded batch detail.
 * - Centralizing this markup reduces cross-route drift without adding new reads or subscriptions.
 */

import type { ResolvedLanguage } from '../../lib/i18n'
import type { ImportBatchDetail } from '../../lib/types'
import { PreviewEntryList } from './preview-entry-list'
import type { ReviewCopyFeedback } from './review-surface'
import { ReviewPathActionRow } from './support-actions'

interface ImportBatchAuditPathActions {
  copyFeedback: ReviewCopyFeedback | null
  copyKey: string
  copyLabel: string
  errorMessage: string
  label: string
  onCopy: (key: string, value: string) => void | Promise<void>
  onOpenPath: (path: string) => void
  openPathLabel: string
  successMessage: string
}

/**
 * Props for the shared import-batch review body.
 *
 * The caller still owns route-specific loading/error state and follow-through
 * actions. This component only renders the stable batch-review content once a
 * batch detail is already available.
 */
export interface ImportBatchReviewProps {
  auditPathActions?: ImportBatchAuditPathActions
  batchDetail: ImportBatchDetail
  language: ResolvedLanguage
  metricLabels: {
    candidateRows: string
    duplicateRows: string
    importedRows: string
    visibleRows: string
  }
  noPreviewEntriesLabel: string
  previewStatusLabel?: (status: string) => string
  previewStatusTone?: (
    status: string,
  ) => 'info' | 'success' | 'danger' | 'neutral'
}

/**
 * Renders the canonical import-batch review body shared by Import and Audit.
 *
 * This keeps the manifest grid, preview rows, and audit-path actions in one
 * owner so route consumers only compose their own follow-through buttons and
 * notices.
 */
export function ImportBatchReview({
  auditPathActions,
  batchDetail,
  language,
  metricLabels,
  noPreviewEntriesLabel,
  previewStatusLabel,
  previewStatusTone,
}: ImportBatchReviewProps) {
  return (
    <>
      <div className="manifest-grid">
        <div className="manifest-field">
          <span className="field-label">{metricLabels.candidateRows}</span>
          <span className="field-value mono">
            {batchDetail.batch.candidateItems.toLocaleString(language)}
          </span>
        </div>
        <div className="manifest-field">
          <span className="field-label">{metricLabels.importedRows}</span>
          <span className="field-value mono">
            {batchDetail.batch.importedItems.toLocaleString(language)}
          </span>
        </div>
        <div className="manifest-field">
          <span className="field-label">{metricLabels.duplicateRows}</span>
          <span className="field-value mono">
            {batchDetail.batch.duplicateItems.toLocaleString(language)}
          </span>
        </div>
        <div className="manifest-field">
          <span className="field-label">{metricLabels.visibleRows}</span>
          <span className="field-value mono">
            {batchDetail.batch.visibleItems.toLocaleString(language)}
          </span>
        </div>
      </div>
      <div className="detail-divider" />
      {batchDetail.previewEntries.length > 0 ? (
        <PreviewEntryList
          entries={batchDetail.previewEntries}
          language={language}
          statusLabel={previewStatusLabel}
          statusTone={previewStatusTone}
        />
      ) : (
        <p className="dim">{noPreviewEntriesLabel}</p>
      )}
      {batchDetail.batch.auditPath && auditPathActions ? (
        <ReviewPathActionRow
          copyFeedback={auditPathActions.copyFeedback}
          copyKey={auditPathActions.copyKey}
          copyLabel={auditPathActions.copyLabel}
          errorMessage={auditPathActions.errorMessage}
          label={auditPathActions.label}
          onCopy={auditPathActions.onCopy}
          onOpenPath={auditPathActions.onOpenPath}
          openPathLabel={auditPathActions.openPathLabel}
          successMessage={auditPathActions.successMessage}
          value={batchDetail.batch.auditPath}
        />
      ) : null}
    </>
  )
}
