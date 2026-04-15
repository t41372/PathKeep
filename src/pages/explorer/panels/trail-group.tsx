/**
 * Trail Group panel — displays browsing history grouped by search trails.
 *
 * Why this file exists:
 * - Part of Core Intelligence P1-3b Explorer Trail View.
 * - Renders search trails as collapsible cards showing query evolution and page hierarchy.
 * - Shows initial query, reformulation chain, landing pages, and trail members.
 *
 * Source-of-truth:
 * - `docs/features/core-intelligence-ultimate-design.md` §2.3 & §3.2
 */

import { useState } from 'react'
import { useAsyncData } from '../../../lib/core-intelligence/hooks'
import * as api from '../../../lib/core-intelligence/api'
import type {
  DateRange,
  TrailSummary,
  TrailMember,
} from '../../../lib/core-intelligence/types'
import type { ResolvedLanguage } from '../../../lib/i18n'
import type { Translator } from '../types'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface TrailGroupPanelProps {
  dateRange: DateRange
  profileId?: string | null
  language: ResolvedLanguage
  explorerT: Translator
  intelligenceT: Translator
  onSelectVisitUrl?: (url: string) => void
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function TrailGroupPanel({
  dateRange,
  profileId,
  language,
  explorerT,
  intelligenceT,
  onSelectVisitUrl,
}: TrailGroupPanelProps) {
  const [page, setPage] = useState(1)

  const { data, loading, error } = useAsyncData(
    () =>
      api.getSearchTrails(dateRange, profileId, undefined, {
        page,
        pageSize: 20,
      }),
    [dateRange, profileId, page],
  )

  if (loading) {
    return (
      <div className="trail-group-panel">
        <div className="intelligence-skeleton intelligence-skeleton--list" />
      </div>
    )
  }

  if (error || !data || data.trails.length === 0) {
    return (
      <div className="trail-group-panel">
        <div className="intelligence-empty">
          <p className="intelligence-empty__text">
            {error || intelligenceT('trailGroupEmpty')}
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="trail-group-panel">
      <div className="trail-group-panel__header">
        <span className="trail-group-panel__summary">
          {intelligenceT('trailGroupSummary', {
            count: data.total,
            page: data.page,
          })}
        </span>
      </div>
      <div className="trail-group-panel__list">
        {data.trails.map((trail) => (
          <TrailCard
            key={trail.trailId}
            trail={trail}
            language={language}
            intelligenceT={intelligenceT}
            onSelectVisitUrl={onSelectVisitUrl}
          />
        ))}
      </div>

      <div className="trail-group-panel__pagination">
        <button
          className="btn-secondary"
          type="button"
          disabled={page <= 1}
          onClick={() => setPage((p) => Math.max(1, p - 1))}
        >
          {explorerT('previousPage')}
        </button>
        <span className="trail-group-panel__page-label">
          {page} / {Math.ceil(data.total / 20)}
        </span>
        <button
          className="btn-secondary"
          type="button"
          disabled={page * 20 >= data.total}
          onClick={() => setPage((p) => p + 1)}
        >
          {explorerT('nextPage')}
        </button>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Trail Card
// ---------------------------------------------------------------------------

/** Engine icon lookup */
function engineIcon(engine: string): string {
  const lower = engine.toLowerCase()
  if (lower.includes('google')) return '🔵'
  if (lower.includes('bing')) return '🟢'
  if (lower.includes('youtube')) return '🔴'
  if (lower.includes('bilibili')) return '🟣'
  if (lower.includes('github')) return '⚫'
  if (lower.includes('duckduckgo')) return '🟠'
  if (lower.includes('baidu')) return '🔵'
  return '🔍'
}

function TrailCard({
  trail,
  language,
  intelligenceT,
  onSelectVisitUrl,
}: {
  trail: TrailSummary
  language: ResolvedLanguage
  intelligenceT: Translator
  onSelectVisitUrl?: (url: string) => void
}) {
  const [expanded, setExpanded] = useState(false)
  const [detail, setDetail] = useState<TrailMember[] | null>(null)
  const [detailLoading, setDetailLoading] = useState(false)

  const locale =
    language === 'en' ? 'en-US' : language === 'zh-CN' ? 'zh-CN' : 'zh-TW'
  const dateStr = new Date(trail.firstVisitMs).toLocaleDateString(locale, {
    month: 'short',
    day: 'numeric',
  })
  const startTime = new Date(trail.firstVisitMs).toLocaleTimeString(locale, {
    hour: '2-digit',
    minute: '2-digit',
  })

  const handleToggle = async () => {
    const willExpand = !expanded
    setExpanded(willExpand)
    if (willExpand && !detail) {
      setDetailLoading(true)
      try {
        const result = await api.getTrailDetail(trail.trailId)
        setDetail(result.members)
      } catch {
        // silently fail
      } finally {
        setDetailLoading(false)
      }
    }
  }

  return (
    <div className={`trail-card${expanded ? ' trail-card--expanded' : ''}`}>
      <button
        className="trail-card__header"
        type="button"
        aria-expanded={expanded}
        onClick={() => void handleToggle()}
      >
        <span className="trail-card__expand-icon">{expanded ? '▼' : '▶'}</span>
        <span className="trail-card__engine-icon">
          {engineIcon(trail.searchEngine)}
        </span>
        <span className="trail-card__query">"{trail.initialQuery}"</span>
        {trail.reformulationCount > 0 && (
          <span className="trail-card__reformulation-badge">
            {trail.reformulationCount}× {intelligenceT('trailReformulation')}
          </span>
        )}
        <span className="trail-card__meta">
          {dateStr} {startTime} · {trail.visitCount}{' '}
          {intelligenceT('sessionVisitLabel')}
        </span>
      </button>

      {expanded && (
        <div className="trail-card__body">
          {/* Query evolution chain */}
          {trail.queries.length > 1 && (
            <div className="trail-card__evolution">
              <span className="trail-card__evolution-label">
                {intelligenceT('trailEvolution')}
              </span>
              <div className="trail-card__evolution-chain">
                {trail.queries.map((q, i) => (
                  <span key={i} className="trail-card__evolution-step">
                    {i > 0 && (
                      <span className="trail-card__evolution-arrow">→</span>
                    )}
                    <span className="trail-card__evolution-query">"{q}"</span>
                  </span>
                ))}
                {trail.landingUrl && (
                  <>
                    <span className="trail-card__evolution-arrow">└──</span>
                    <span className="trail-card__evolution-landing">
                      {intelligenceT('trailLanding')}:{' '}
                      {trail.landingDomain ?? trail.landingUrl}
                    </span>
                  </>
                )}
              </div>
            </div>
          )}

          {/* Trail members */}
          {detailLoading ? (
            <div
              className="intelligence-skeleton intelligence-skeleton--list"
              style={{ height: 120 }}
            />
          ) : detail ? (
            <div className="trail-card__members">
              {detail.map((member) => (
                <div
                  key={member.visitId}
                  className={`trail-member-row trail-member-row--${member.role}`}
                  role="button"
                  tabIndex={0}
                  onClick={() => onSelectVisitUrl?.(member.url)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault()
                      onSelectVisitUrl?.(member.url)
                    }
                  }}
                >
                  <span className="trail-member-row__ordinal">
                    {member.role === 'search_event'
                      ? '🔍'
                      : member.role === 'landing'
                        ? '🎯'
                        : '📄'}
                  </span>
                  <span className="trail-member-row__content">
                    {member.role === 'search_event' && member.searchQuery
                      ? `"${member.searchQuery}"`
                      : member.title || member.url}
                  </span>
                  <span className="trail-member-row__time">
                    {new Date(member.visitTimeMs).toLocaleTimeString(locale, {
                      hour: '2-digit',
                      minute: '2-digit',
                    })}
                  </span>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      )}
    </div>
  )
}
