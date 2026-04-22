/**
 * @file timeline-bar.tsx
 * @description Render-only timeline and pagination summary strip for the Explorer route.
 * @module pages/explorer
 *
 * ## Responsibilities
 * - Render the date shortcut controls at the top of Explorer.
 * - Render the current visible page summary without owning query state.
 * - Keep the clear-range affordance and staged-loading summary in one owner.
 *
 * ## Not responsible for
 * - Fetching Explorer data.
 * - Parsing URL state or deciding which shortcut is active.
 * - Rendering the main record list or detail rail.
 *
 * ## Dependencies
 * - Depends on Explorer translator copy and the shared date shortcut list.
 *
 * ## Performance notes
 * - Pure render-only chrome so Explorer can keep this strip mounted while
 *   results load in the background.
 */

import { dateShortcutWindows } from './helpers'
import type { Translator } from './types'

interface ExplorerTimelineBarProps {
  activeShortcutKey: string | null
  explorerT: Translator
  onApplyDateShortcut: (days: number) => void
  onClearDateRange: () => void
  summary: {
    currentPage: number
    loaded: number
    pageCount: number
    total: number
  } | null
  end: string | null
  start: string | null
}

/**
 * Keeps Explorer's top timeline strip mounted while results stage in.
 *
 * The route owns the actual query state; this component only renders the
 * current shortcut, date-range summary, and staged pagination feedback.
 */
export function ExplorerTimelineBar({
  activeShortcutKey,
  explorerT,
  onApplyDateShortcut,
  onClearDateRange,
  summary,
  end,
  start,
}: ExplorerTimelineBarProps) {
  return (
    <div className="timeline-bar">
      <div className="timeline-controls">
        {dateShortcutWindows.map((entry) => (
          <button
            key={entry.key}
            className={`tl-btn ${activeShortcutKey === entry.key ? 'active' : ''}`}
            type="button"
            onClick={() => onApplyDateShortcut(entry.days)}
          >
            {explorerT(entry.labelKey)}
          </button>
        ))}
      </div>
      <div className="timeline-track">
        {summary ? (
          <div className="timeline-page-summary">
            <span className="history-page-summary">
              {explorerT('pageCountSummary', {
                current: summary.currentPage,
                total: summary.pageCount,
              })}
            </span>
            <span className="timeline-page-summary__loaded">
              {explorerT('resultsSummary', {
                loaded: summary.loaded,
                total: summary.total,
              })}
            </span>
          </div>
        ) : (
          <span className="timeline-label">{explorerT('waitingForQuery')}</span>
        )}
        <span className="timeline-label">
          {start || end
            ? `${start ?? '…'} → ${end ?? '…'}`
            : explorerT('allRecordedTime')}
        </span>
        {(start || end) && (
          <button className="tl-today" type="button" onClick={onClearDateRange}>
            {explorerT('clearRange')}
          </button>
        )}
      </div>
    </div>
  )
}
