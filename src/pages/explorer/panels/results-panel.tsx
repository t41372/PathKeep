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

import { formatRelativeTime } from '../../../lib/format'
import { HistoryFavicon } from '../../../components/primitives/history-favicon'
import type { ResolvedLanguage } from '../../../lib/i18n'
import type { ExportFormat, HistoryQueryResponse } from '../../../lib/types'
import { keywordPageSizeOptions } from '../helpers'
import {
  activateRecordSelection,
  sanitizeExplorerDisplayText,
} from '../helpers'
import { ExplorerDetailPanel } from './detail-panel'
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
  historyPageSize: number
  intelligenceT: Translator
  language: ResolvedLanguage
  onHistoryPageInputChange: (value: string) => void
  onHistoryPageSizeChange: (value: number) => void
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
  historyPageSize,
  intelligenceT,
  language,
  onHistoryPageInputChange,
  onHistoryPageSizeChange,
  onSelectHistory,
  results,
  selectedEntry,
}: ExplorerResultsPanelProps) {
  return (
    <div className="explorer-grid">
      <div className="record-list">
        <div className="record-group">
          <div className="record-group-header">
            <div className="record-group-header__summary">
              <span className="history-page-summary">
                {explorerT('pageCountSummary', {
                  current: historyPage,
                  total: historyPageCount,
                })}
              </span>
              <span className="record-group-header__loaded">
                {explorerT('resultsSummary', {
                  loaded: results.items.length,
                  total: results.total,
                })}
              </span>
            </div>
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
              <HistoryFavicon domain={item.domain} favicon={item.favicon} />
              <div className="record-main">
                <div className="record-title">
                  {sanitizeExplorerDisplayText(item.title || item.url)}
                </div>
                <div className="record-url dim mono">
                  {sanitizeExplorerDisplayText(item.url, 72)}
                </div>
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
          <div className="record-group-pagination">
            <div className="record-group-pagination__summary">
              <span className="history-page-summary">
                {explorerT('pageCountSummary', {
                  current: historyPage,
                  total: historyPageCount,
                })}
              </span>
              <span className="record-group-pagination__loaded">
                {explorerT('resultsSummary', {
                  loaded: results.items.length,
                  total: results.total,
                })}
              </span>
            </div>
            <div className="record-group-pagination__controls">
              <div className="record-group-pagination__nav">
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
              </div>
              <div className="record-group-pagination__jump">
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
                <label className="history-page-jump">
                  <span className="history-page-jump__label">
                    {explorerT('pageSizeLabel')}
                  </span>
                  <select
                    className="history-page-size__select"
                    value={historyPageSize}
                    onChange={(event) =>
                      onHistoryPageSizeChange(Number(event.target.value))
                    }
                  >
                    {keywordPageSizeOptions.map((option) => (
                      <option key={option} value={option}>
                        {explorerT('pageSizeOption', { count: option })}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
            </div>
          </div>
        </div>
      </div>
      <ExplorerDetailPanel
        commonT={commonT}
        explorerT={explorerT}
        footer={
          <>
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
            {exportResult ? (
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
                {copiedExportPath === exportResult.path ? (
                  <span className="dim mono" style={{ fontSize: '10px' }}>
                    {explorerT('copied')}
                  </span>
                ) : null}
                {copiedExportPath === `error:${exportResult.path}` ? (
                  <span className="dim mono" style={{ fontSize: '10px' }}>
                    {explorerT('copyFailed')}
                  </span>
                ) : null}
              </div>
            ) : null}
          </>
        }
        handleVisit={handleVisit}
        intelligenceT={intelligenceT}
        language={language}
        selectedVisit={
          selectedEntry
            ? {
                domain: selectedEntry.domain,
                profileId: selectedEntry.profileId,
                title: selectedEntry.title,
                transition: selectedEntry.transition,
                url: selectedEntry.url,
                visitId: selectedEntry.id,
                visitedAt: selectedEntry.visitedAt,
              }
            : null
        }
      />
    </div>
  )
}
