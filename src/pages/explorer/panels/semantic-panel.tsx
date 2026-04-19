/**
 * This module renders a focused panel inside the Explorer route.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `ExplorerSemanticPanel`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { Link } from 'react-router-dom'
import { EmptyState } from '../../../components/primitives/empty-state'
import { ErrorState } from '../../../components/primitives/error-state'
import { LoadingState } from '../../../components/primitives/loading-state'
import {
  assistantHref,
  evidenceHref,
  scoreBand,
} from '../../../lib/intelligence'
import { formatDateTime } from '../../../lib/format'
import type { ResolvedLanguage } from '../../../lib/i18n'
import type { AiSearchResponse } from '../../../lib/types'
import { sanitizeExplorerDisplayText } from '../helpers'
import type { Translator } from '../types'

/**
 * Describes the props accepted by `ExplorerSemanticPanel`.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
interface ExplorerSemanticPanelProps {
  explorerT: Translator
  intelligenceT: Translator
  language: ResolvedLanguage
  mode: 'semantic' | 'hybrid'
  onNextPage: (nextCursor: string | null) => void
  onPreviousPage: () => void
  onSelectHistory: (historyId: number) => void
  semanticError: string | null
  semanticLoading: boolean
  semanticQuery: {
    query: string
  }
  semanticResults: AiSearchResponse | null
  semanticTrailLength: number
}

/**
 * Renders the explorer semantic panel.
 *
 * Keeping this as a named declaration makes the Explorer surface easier to review and test than burying the behavior inside another anonymous callback.
 */
export function ExplorerSemanticPanel({
  explorerT,
  intelligenceT,
  language,
  mode,
  onNextPage,
  onPreviousPage,
  onSelectHistory,
  semanticError,
  semanticLoading,
  semanticQuery,
  semanticResults,
  semanticTrailLength,
}: ExplorerSemanticPanelProps) {
  return (
    <div className="panel intelligence-panel">
      <div className="panel-header">
        <span className="panel-title">{explorerT('semanticRecallTitle')}</span>
        <span className="panel-action">
          {semanticResults
            ? explorerT('semanticPageSummary', {
                page: semanticTrailLength + 1,
                loaded: semanticResults.items.length,
                total: semanticResults.total,
              })
            : semanticQuery.query
              ? explorerT('preparingRecall')
              : explorerT('enterQueryToRank')}
        </span>
      </div>
      <div className="panel-body intelligence-stack">
        {!semanticQuery.query ? (
          <p className="dashboard-next-action">{explorerT('semanticPrompt')}</p>
        ) : semanticLoading ? (
          <LoadingState
            compact
            label={explorerT('rankingSemanticEvidence')}
            detail={explorerT('preparingRecall')}
            progressLabel={`1 / ${mode === 'hybrid' ? 3 : 2}`}
            progressValue={mode === 'hybrid' ? 33 : 50}
          />
        ) : semanticError ? (
          <ErrorState
            title={explorerT('semanticRecallDegradedTitle')}
            description={semanticError}
          />
        ) : semanticResults && semanticResults.total > 0 ? (
          <>
            {semanticResults.notes.length > 0 && (
              <div className="intelligence-note-list">
                {semanticResults.notes.map((note) => (
                  <p key={note} className="mono-support">
                    {note}
                  </p>
                ))}
              </div>
            )}
            <div className="intelligence-result-list">
              {semanticResults.items.map((item) => {
                const band = scoreBand(item.score, intelligenceT)
                const displayTitle = sanitizeExplorerDisplayText(
                  item.title ?? item.url,
                )
                const displayUrl = sanitizeExplorerDisplayText(item.url, 96)
                return (
                  <div key={item.historyId} className="result-row">
                    <div className="result-row__header">
                      <strong>{displayTitle}</strong>
                      <span className={`status-badge status-${band.tone}`}>
                        {band.label}
                      </span>
                    </div>
                    <p>{item.matchReason}</p>
                    <div className="result-row__meta">
                      <span className="mono-support">
                        {item.profileId} ·{' '}
                        {formatDateTime(item.visitedAt, language) ??
                          item.visitedAt}
                      </span>
                      <span className="mono-support">{displayUrl}</span>
                    </div>
                    <div className="intelligence-actions">
                      <button
                        className="btn-tiny"
                        type="button"
                        onClick={() => onSelectHistory(item.historyId)}
                      >
                        {explorerT('jumpToRecord')}
                      </button>
                      <Link className="btn-tiny" to={evidenceHref(item)}>
                        {explorerT('openEvidence')}
                      </Link>
                      <Link
                        className="btn-tiny"
                        to={assistantHref(
                          explorerT('assistantExplainPrompt', {
                            item: displayTitle,
                            query: semanticQuery.query,
                          }),
                          item.profileId,
                        )}
                      >
                        {explorerT('askAssistant')}
                      </Link>
                    </div>
                  </div>
                )
              })}
            </div>
            <div className="intelligence-actions">
              <button
                className="btn-secondary"
                type="button"
                onClick={onPreviousPage}
                disabled={semanticTrailLength === 0}
              >
                {explorerT('previousEvidencePage')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={() => onNextPage(semanticResults.nextCursor ?? null)}
                disabled={!semanticResults.nextCursor}
              >
                {explorerT('nextEvidencePage')}
              </button>
            </div>
          </>
        ) : (
          <EmptyState
            description={explorerT('noSemanticDescription')}
            eyebrow={explorerT('noSemanticEyebrow')}
            title={explorerT('noSemanticTitle')}
          />
        )}
      </div>
    </div>
  )
}
