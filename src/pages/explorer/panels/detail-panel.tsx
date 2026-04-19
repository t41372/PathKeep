/**
 * Shared Explorer detail rail for selected history records across time, session, and trail views.
 *
 * Why this file exists:
 * - The Explorer route should show one consistent detail workflow even when the left-side grouping changes.
 * - Keeping the detail rail separate avoids duplicating visit metadata, open actions, and navigation tracing across panels.
 *
 * Main declarations:
 * - `ExplorerDetailPanel`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for Explorer deep-link behavior.
 * - Keep loading and trust grammar aligned with `docs/design/ux-principles.md`.
 */

import type { ReactNode } from 'react'
import { formatDateTime } from '../../../lib/format'
import { type ResolvedLanguage } from '../../../lib/i18n'
import { sanitizeExplorerDisplayText } from '../helpers'
import { NavigationTracer } from './navigation-tracer'
import type { ExplorerVisitSelection, Translator } from '../types'

interface ExplorerDetailPanelProps {
  commonT: Translator
  explorerT: Translator
  footer?: ReactNode
  handleVisit: (url: string) => Promise<void>
  intelligenceT: Translator
  language: ResolvedLanguage
  selectedVisit: ExplorerVisitSelection | null
}

/**
 * Renders the shared detail rail for a selected visit.
 */
export function ExplorerDetailPanel({
  commonT,
  explorerT,
  footer,
  handleVisit,
  intelligenceT,
  language,
  selectedVisit,
}: ExplorerDetailPanelProps) {
  return (
    <div className="detail-panel">
      <div className="detail-header">
        <span className="crosshair-mark small">+</span>
        <span className="detail-label">{explorerT('recordDetail')}</span>
      </div>
      {selectedVisit ? (
        <div className="detail-body">
          <div className="detail-section">
            <div className="detail-field">
              <span className="field-label">{explorerT('fieldTitle')}</span>
              <span className="field-value">
                {sanitizeExplorerDisplayText(
                  selectedVisit.title ?? selectedVisit.url,
                )}
              </span>
            </div>
            <div className="detail-field">
              <span className="field-label">{explorerT('fieldUrl')}</span>
              <span className="field-value" style={{ wordBreak: 'break-all' }}>
                {selectedVisit.url}
              </span>
            </div>
          </div>
          <div className="detail-divider" />
          <div className="detail-row">
            <div className="detail-field half">
              <span className="field-label">{explorerT('visitedAt')}</span>
              <span className="field-value">
                {formatDateTime(selectedVisit.visitedAt, language) ??
                  selectedVisit.visitedAt}
              </span>
            </div>
            <div className="detail-field half">
              <span className="field-label">{explorerT('fieldProfile')}</span>
              <span className="field-value">
                {selectedVisit.profileId ?? commonT('notAvailable')}
              </span>
            </div>
          </div>
          <div className="detail-row">
            <div className="detail-field half">
              <span className="field-label">{explorerT('transition')}</span>
              <span className="field-value">
                {selectedVisit.transition ?? commonT('notAvailable')}
              </span>
            </div>
            <div className="detail-field half" />
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
                void handleVisit(selectedVisit.url)
              }}
            >
              {explorerT('visitRecord')}
            </button>
          </div>
          <NavigationTracer
            intelligenceT={intelligenceT}
            visitId={selectedVisit.visitId}
            onSelectVisitUrl={(url) => {
              void handleVisit(url)
            }}
          />
          {footer}
        </div>
      ) : (
        <div className="detail-body">
          <p className="dim">{explorerT('waitingForQuery')}</p>
        </div>
      )}
    </div>
  )
}
