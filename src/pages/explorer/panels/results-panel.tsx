/**
 * This module renders a focused panel inside the Explorer route.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `ExplorerResultsPanel`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import {
  formatDateTime,
  formatDuration,
  formatRelativeTime,
} from '../../../lib/format'
import { EmptyState } from '../../../components/primitives/empty-state'
import type { ResolvedLanguage } from '../../../lib/i18n'
import type { ExportFormat, HistoryQueryResponse } from '../../../lib/types'
import { activateRecordSelection } from '../helpers'
import type { Translator } from '../types'

/**
 * Describes the props accepted by `ExplorerResultsPanel`.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface ExplorerResultsPanelProps {
  actionError: string | null
  commonT: Translator
  copiedExportPath: string | null
  explorerT: Translator
  exportResult: { path: string } | null
  handleCopyExportPath: (path: string) => Promise<void>
  handleExport: (format: ExportFormat) => Promise<void>
  handleOpenExportPath: (path: string) => Promise<void>
  handleVisit: (url: string) => Promise<void>
  handleHistoryPageJump: (historyPageCount: number) => void
  handleFirstHistoryPage: () => void
  handleLastHistoryPage: (historyPageCount: number) => void
  handleNextHistoryPage: () => void
  handlePreviousHistoryPage: () => void
  historyBlockedByInvalidRegex: boolean
  historyPage: number
  historyPageCount: number
  historyPageInput: string
  language: ResolvedLanguage
  onHistoryPageInputChange: (value: string) => void
  onSelectHistory: (id: number) => void
  results: HistoryQueryResponse
  selectedEntry: HistoryQueryResponse['items'][number] | null
}

/**
 * Renders the explorer results panel.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function ExplorerResultsPanel({
  actionError,
  commonT,
  copiedExportPath,
  explorerT,
  exportResult,
  handleCopyExportPath,
  handleExport,
  handleOpenExportPath,
  handleVisit,
  handleHistoryPageJump,
  handleFirstHistoryPage,
  handleLastHistoryPage,
  handleNextHistoryPage,
  handlePreviousHistoryPage,
  historyBlockedByInvalidRegex,
  historyPage,
  historyPageCount,
  historyPageInput,
  language,
  onHistoryPageInputChange,
  onSelectHistory,
  results,
  selectedEntry,
}: ExplorerResultsPanelProps) {
  return (
    <div className="explorer-grid">
      <div className="record-list">
        <div className="record-group">
          <div className="record-group-header">
            {explorerT('resultsSummary', {
              page: historyPage,
              loaded: results.items.length,
              total: results.total,
            })}
          </div>
          {results.items.map((item) => (
            <div
              key={item.id}
              className={`record-item ${selectedEntry?.id === item.id ? 'selected' : ''}`}
              role="button"
              tabIndex={0}
              aria-pressed={selectedEntry?.id === item.id}
              onClick={() => onSelectHistory(item.id)}
              onKeyDown={(event) =>
                activateRecordSelection(event, () => onSelectHistory(item.id))
              }
            >
              <div className="favicon-placeholder">
                {(item.domain ?? '?')[0].toUpperCase()}
              </div>
              <div className="record-main">
                <div className="record-title">{item.title || item.url}</div>
                <div className="record-url dim mono">{item.url}</div>
              </div>
              <div className="record-meta">
                <span className="dim mono" style={{ fontSize: '10px' }}>
                  {formatRelativeTime(item.visitedAt, language)}
                </span>
                <button
                  className="btn-tiny"
                  type="button"
                  onClick={(event) => {
                    event.stopPropagation()
                    void handleVisit(item.url)
                  }}
                >
                  {explorerT('visitRecord')}
                </button>
              </div>
            </div>
          ))}
          <div
            className="intelligence-actions"
            style={{ padding: 'var(--space-3) 0 0' }}
          >
            <button
              className="btn-secondary"
              type="button"
              onClick={handleFirstHistoryPage}
              disabled={!results.hasPrevious}
            >
              {explorerT('firstPage')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={handlePreviousHistoryPage}
              disabled={!results.hasPrevious}
            >
              {explorerT('previousPage')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={handleNextHistoryPage}
              disabled={!results.hasNext}
            >
              {explorerT('nextPage')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => handleLastHistoryPage(historyPageCount)}
              disabled={!results.hasNext}
            >
              {explorerT('lastPage')}
            </button>
            <label className="history-page-jump">
              <span className="history-page-jump__label">
                {explorerT('pageNumberLabel')}
              </span>
              <input
                className="history-page-jump__input"
                inputMode="numeric"
                min={1}
                type="number"
                value={historyPageInput}
                onChange={(event) =>
                  onHistoryPageInputChange(event.target.value)
                }
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault()
                    handleHistoryPageJump(historyPageCount)
                  }
                }}
              />
            </label>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => handleHistoryPageJump(historyPageCount)}
            >
              {explorerT('jumpToPage')}
            </button>
          </div>
        </div>
      </div>

      <div className="detail-panel">
        <div className="detail-header">
          <span className="crosshair-mark small">+</span>
          <span className="detail-label">{explorerT('recordDetail')}</span>
        </div>
        {selectedEntry ? (
          <div className="detail-body">
            <div className="detail-section">
              <div className="detail-field">
                <span className="field-label">{explorerT('fieldTitle')}</span>
                <span className="field-value">
                  {selectedEntry.title || selectedEntry.url}
                </span>
              </div>
              <div className="detail-field">
                <span className="field-label">{explorerT('fieldUrl')}</span>
                <span
                  className="field-value"
                  style={{ wordBreak: 'break-all' }}
                >
                  {selectedEntry.url}
                </span>
              </div>
            </div>
            <div className="detail-divider" />
            <div className="detail-row">
              <div className="detail-field half">
                <span className="field-label">{explorerT('visitedAt')}</span>
                <span className="field-value">
                  {formatDateTime(selectedEntry.visitedAt, language) ??
                    selectedEntry.visitedAt}
                </span>
              </div>
              <div className="detail-field half">
                <span className="field-label">{explorerT('duration')}</span>
                <span className="field-value">
                  {formatDuration(selectedEntry.durationMs)}
                </span>
              </div>
            </div>
            <div className="detail-row">
              <div className="detail-field half">
                <span className="field-label">{explorerT('fieldProfile')}</span>
                <span className="field-value">{selectedEntry.profileId}</span>
              </div>
              <div className="detail-field half">
                <span className="field-label">{explorerT('transition')}</span>
                <span className="field-value">
                  {selectedEntry.transition ?? commonT('notAvailable')}
                </span>
              </div>
            </div>
            <div className="detail-divider" />
            <div
              className="intelligence-actions"
              style={{ marginBottom: 'var(--space-3)' }}
            >
              <button
                className="btn-secondary"
                type="button"
                onClick={() => {
                  void handleVisit(selectedEntry.url)
                }}
              >
                {explorerT('visitRecord')}
              </button>
            </div>
            {actionError ? (
              <p className="inline-error" role="alert">
                {actionError}
              </p>
            ) : null}
            <div className="summary-label">
              {explorerT('exportVisibleQuery')}
            </div>
            <p className="dashboard-next-action">
              {explorerT('exportDescription')}
            </p>
            <div style={{ display: 'flex', gap: 'var(--space-2)' }}>
              {(['jsonl', 'markdown', 'html', 'text'] as ExportFormat[]).map(
                (format) => (
                  <button
                    key={format}
                    className="btn-tiny"
                    disabled={historyBlockedByInvalidRegex}
                    type="button"
                    onClick={() => {
                      void handleExport(format)
                    }}
                  >
                    {format}
                  </button>
                ),
              )}
            </div>
            {exportResult && (
              <div style={{ marginTop: 'var(--space-3)', fontSize: '11px' }}>
                <span className="dim mono">{exportResult.path}</span>
                <div
                  style={{
                    marginTop: 'var(--space-1)',
                    display: 'flex',
                    gap: 'var(--space-2)',
                  }}
                >
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => {
                      void handleOpenExportPath(exportResult.path)
                    }}
                  >
                    {commonT('openAction')}
                  </button>
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => {
                      void handleCopyExportPath(exportResult.path)
                    }}
                  >
                    {commonT('copyAction')}
                  </button>
                </div>
                {copiedExportPath === exportResult.path && (
                  <span className="dim mono" style={{ fontSize: '10px' }}>
                    {explorerT('copied')}
                  </span>
                )}
                {copiedExportPath === `error:${exportResult.path}` && (
                  <span className="dim mono" style={{ fontSize: '10px' }}>
                    {explorerT('copyFailed')}
                  </span>
                )}
              </div>
            )}
          </div>
        ) : (
          <div className="detail-body">
            <EmptyState
              description={explorerT('noResultDescription')}
              eyebrow={explorerT('noResultEyebrow')}
              title={explorerT('noResultTitle')}
            />
          </div>
        )}
      </div>
    </div>
  )
}
