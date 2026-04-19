/**
 * Session Group panel — displays browsing history grouped by sessions.
 *
 * Why this file exists:
 * - Part of Core Intelligence P1-2b Explorer Session View.
 * - Renders sessions as collapsible cards with auto-generated titles.
 * - Each session shows visit count, search count, time range, and deep-dive badge.
 *
 * Source-of-truth:
 * - `docs/features/core-intelligence-ultimate-design.md` §2.3 & §3.1
 * - `docs/design/ux-principles.md`
 */

import { useState } from 'react'
import { ExplainabilityPanel } from '../../../components/intelligence/explainability-panel'
import { useAsyncData } from '../../../lib/core-intelligence/hooks'
import * as api from '../../../lib/core-intelligence/api'
import type {
  DateRange,
  SessionSummary,
  SessionVisit,
} from '../../../lib/core-intelligence/types'
import type { ResolvedLanguage } from '../../../lib/i18n'
import { sanitizeExplorerDisplayText } from '../helpers'
import type { ExplorerVisitSelection, Translator } from '../types'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface SessionGroupPanelProps {
  dateRange: DateRange
  profileId?: string | null
  language: ResolvedLanguage
  explorerT: Translator
  intelligenceT: Translator
  onSelectVisit?: (visit: ExplorerVisitSelection) => void
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function SessionGroupPanel({
  dateRange,
  profileId,
  language,
  explorerT,
  intelligenceT,
  onSelectVisit,
}: SessionGroupPanelProps) {
  const [page, setPage] = useState(0)

  const { data, loading, error } = useAsyncData(
    () => api.getSessions(dateRange, profileId, { page, pageSize: 20 }),
    [dateRange, profileId, page],
  )

  if (loading) {
    return (
      <div className="session-group-panel">
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      </div>
    )
  }

  if (error || !data || data.sessions.length === 0) {
    return (
      <div className="session-group-panel">
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || intelligenceT('sessionGroupEmpty')}
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="session-group-panel">
      <div className="session-group-panel__header">
        <span className="session-group-panel__summary">
          {intelligenceT('sessionGroupSummary', {
            count: data.total,
            page: data.page + 1,
          })}
        </span>
      </div>
      <div className="session-group-panel__list">
        {data.sessions.map((session) => (
          <SessionCard
            key={session.sessionId}
            profileId={profileId}
            session={session}
            language={language}
            intelligenceT={intelligenceT}
            onSelectVisit={onSelectVisit}
          />
        ))}
      </div>

      <div className="session-group-panel__pagination">
        <button
          className="btn-secondary"
          type="button"
          disabled={page <= 0}
          onClick={() => setPage((p) => Math.max(0, p - 1))}
        >
          {explorerT('previousPage')}
        </button>
        <span className="session-group-panel__page-label">
          {page + 1} / {Math.max(1, Math.ceil(data.total / 20))}
        </span>
        <button
          className="btn-secondary"
          type="button"
          disabled={(page + 1) * 20 >= data.total}
          onClick={() => setPage((p) => p + 1)}
        >
          {explorerT('nextPage')}
        </button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Session Card
// ---------------------------------------------------------------------------

function SessionCard({
  profileId,
  session,
  language,
  intelligenceT,
  onSelectVisit,
}: {
  profileId?: string | null
  session: SessionSummary
  language: ResolvedLanguage
  intelligenceT: Translator
  onSelectVisit?: (visit: ExplorerVisitSelection) => void
}) {
  const [expanded, setExpanded] = useState(false)
  const [detail, setDetail] = useState<SessionVisit[] | null>(null)
  const [detailLoading, setDetailLoading] = useState(false)

  const startTime = new Date(session.firstVisitMs).toLocaleTimeString(
    language === 'en' ? 'en-US' : language === 'zh-CN' ? 'zh-CN' : 'zh-TW',
    { hour: '2-digit', minute: '2-digit' },
  )
  const endTime = new Date(session.lastVisitMs).toLocaleTimeString(
    language === 'en' ? 'en-US' : language === 'zh-CN' ? 'zh-CN' : 'zh-TW',
    { hour: '2-digit', minute: '2-digit' },
  )
  const dateStr = new Date(session.firstVisitMs).toLocaleDateString(
    language === 'en' ? 'en-US' : language === 'zh-CN' ? 'zh-CN' : 'zh-TW',
    { month: 'short', day: 'numeric' },
  )

  const handleToggle = async () => {
    const willExpand = !expanded
    setExpanded(willExpand)
    if (willExpand && !detail) {
      setDetailLoading(true)
      try {
        const result = await api.getSessionDetail(session.sessionId)
        setDetail(result.visits)
      } catch {
        // silently fail
      } finally {
        setDetailLoading(false)
      }
    }
  }

  return (
    <div className={`session-card${expanded ? ' session-card--expanded' : ''}`}>
      <button
        className="session-card__header"
        type="button"
        aria-expanded={expanded}
        onClick={() => void handleToggle()}
      >
        <span className="session-card__expand-icon">
          {expanded ? '▼' : '▶'}
        </span>
        <span className="session-card__date-badge">
          {dateStr} {startTime} – {endTime}
        </span>
        <span className="session-card__title">
          {sanitizeExplorerDisplayText(session.autoTitle) ||
            intelligenceT('sessionUntitled')}
        </span>
        <span className="session-card__meta">
          {session.visitCount} {intelligenceT('sessionVisitLabel')}
          {session.searchCount > 0 && (
            <>
              {' '}
              · {session.searchCount} {intelligenceT('sessionSearchLabel')}
            </>
          )}
        </span>
        {session.isDeepDive && (
          <span
            className="session-card__deep-dive-badge"
            title={intelligenceT('sessionDeepDive')}
          >
            🔬
          </span>
        )}
      </button>

      {expanded && (
        <div className="session-card__body">
          {detailLoading ? (
            <div
              className="intelligence-skeleton intelligence-skeleton--list"
              style={{ height: 120 }}
            />
          ) : detail ? (
            <div className="session-card__visits">
              {detail.map((visit) => (
                <div
                  key={visit.visitId}
                  className={`session-visit-row${visit.isSearchEvent ? ' session-visit-row--search' : ''}`}
                  role="button"
                  tabIndex={0}
                  onClick={() => onSelectVisit?.(toSelection(visit, profileId))}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault()
                      onSelectVisit?.(toSelection(visit, profileId))
                    }
                  }}
                >
                  {visit.isSearchEvent ? (
                    <span className="session-visit-row__search-icon">🔍</span>
                  ) : (
                    <span className="session-visit-row__page-icon">📄</span>
                  )}
                  <span className="session-visit-row__content">
                    {visit.isSearchEvent && visit.searchQuery ? (
                      <span className="session-visit-row__query">
                        {visit.searchEngine ?? 'Search'}: "
                        {sanitizeExplorerDisplayText(visit.searchQuery, 72)}"
                      </span>
                    ) : (
                      <span className="session-visit-row__title">
                        {sanitizeExplorerDisplayText(visit.title || visit.url)}
                      </span>
                    )}
                  </span>
                  <span className="session-visit-row__time">
                    {new Date(visit.visitTimeMs).toLocaleTimeString(
                      language === 'en'
                        ? 'en-US'
                        : language === 'zh-CN'
                          ? 'zh-CN'
                          : 'zh-TW',
                      { hour: '2-digit', minute: '2-digit' },
                    )}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <div className="intelligence-empty">
              <p className="intelligence-empty__text">
                {intelligenceT('sessionDetailError')}
              </p>
            </div>
          )}
          <ExplainabilityPanel
            entityType="session"
            entityId={session.sessionId}
            t={intelligenceT}
          />
        </div>
      )}
    </div>
  )
}

function toSelection(
  visit: SessionVisit,
  profileId?: string | null,
): ExplorerVisitSelection {
  return {
    domain: visit.registrableDomain,
    profileId,
    title: visit.title,
    transition: visit.transitionType,
    url: visit.url,
    visitId: visit.visitId,
    visitedAt: new Date(visit.visitTimeMs).toISOString(),
  }
}
