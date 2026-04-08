import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend'
import {
  calendarDayKey,
  formatDateTime,
  formatRelativeTime,
} from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import {
  aiStatusMeta,
  assistantHref,
  dedupeEvidence,
  evidenceHref,
} from '../../lib/intelligence'
import type {
  InsightCard,
  InsightEvidenceItem,
  InsightExplanation,
  InsightSnapshot,
} from '../../lib/types'

const topicColors = ['#FF7832', '#4ECDC4', '#FFE66D', '#FF6B6B', '#89CFF0']

function domainFromUrl(url: string) {
  try {
    return new URL(url).hostname
  } catch {
    return url
  }
}

function flattenInsightEvidence(snapshot: InsightSnapshot) {
  return dedupeEvidence([
    ...snapshot.cards.flatMap((card) => card.evidence),
    ...snapshot.topics.flatMap((topic) => topic.evidence),
    ...snapshot.threads.flatMap((thread) => thread.evidence),
  ])
}

export function InsightsPage() {
  const { language, ns } = useI18n()
  const { refreshAppData, refreshKey, snapshot } = useShellData()
  const [insights, setInsights] = useState<InsightSnapshot | null>(null)
  const [explanation, setExplanation] = useState<InsightExplanation | null>(
    null,
  )
  const [selectedCard, setSelectedCard] = useState<InsightCard | null>(null)
  const [action, setAction] = useState<string | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const insightsT = ns('insights')
  const intelligenceT = ns('intelligence')
  useEffect(() => {
    let cancelled = false
    setLoading(true)
    const load = async () => {
      try {
        const result = await backend.loadInsights({ fullRebuild: false })
        if (!cancelled) {
          setInsights(result)
          setLoadError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setLoadError(
            error instanceof Error ? error.message : insightsT('loadingLabel'),
          )
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    void load()
    return () => {
      cancelled = true
    }
  }, [insightsT, refreshKey])

  const aiMeta = snapshot
    ? aiStatusMeta(snapshot.aiStatus, intelligenceT)
    : null
  const allEvidence = useMemo(
    () => (insights ? flattenInsightEvidence(insights) : []),
    [insights],
  )
  const todayKey = calendarDayKey(new Date())
  const onThisDay = useMemo(
    () =>
      allEvidence
        .filter((item) => calendarDayKey(item.visitedAt) === todayKey)
        .slice(0, 6),
    [allEvidence, todayKey],
  )
  const siteAnalytics = useMemo(() => {
    const counts = new Map<string, number>()
    allEvidence.forEach((item) => {
      const domain = domainFromUrl(item.url)
      counts.set(domain, (counts.get(domain) ?? 0) + 1)
    })
    const top = [...counts.entries()]
      .sort((left, right) => right[1] - left[1])
      .slice(0, 5)
    const maxCount = top[0]?.[1] ?? 1
    return top.map(([domain, count]) => ({
      domain,
      count,
      pct: Math.round((count / maxCount) * 100),
    }))
  }, [allEvidence])
  const periodicSummary = useMemo(() => {
    if (!insights) return []
    const seeded = insights.cards.slice(0, 2).map((card) => card.summary)
    return seeded.length > 0 ? seeded : insights.notes
  }, [insights])

  async function handleRefreshInsights() {
    setAction(insightsT('refreshingAction'))
    setLoadError(null)
    try {
      await backend.runInsightsNow({ fullRebuild: false })
      const nextInsights = await backend.loadInsights({ fullRebuild: false })
      setInsights(nextInsights)
      await refreshAppData()
    } catch (error) {
      setLoadError(
        error instanceof Error
          ? error.message
          : insightsT('refreshAttentionTitle'),
      )
    } finally {
      setAction(null)
    }
  }

  async function handleExplain(card: InsightCard) {
    setAction(insightsT('explainingAction'))
    setLoadError(null)
    setSelectedCard(card)
    try {
      const nextExplanation = await backend.explainInsight({
        insightId: card.cardId,
        insightKind: card.kind,
        profileId: card.profileId ?? null,
        windowDays: card.windowDays,
      })
      setExplanation(nextExplanation)
    } catch (error) {
      setLoadError(
        error instanceof Error ? error.message : insightsT('explainability'),
      )
    } finally {
      setAction(null)
    }
  }

  if (!snapshot?.config.initialized) {
    return (
      <section className="page-shell">
        <EmptyState
          description={insightsT('archiveNotInitializedDescription')}
          eyebrow={insightsT('intelligenceEyebrow')}
          title={insightsT('archiveNotInitializedTitle')}
        />
      </section>
    )
  }

  if (loading && !insights) {
    return (
      <section className="page-shell">
        <LoadingState label={insightsT('loadingLabel')} />
      </section>
    )
  }

  if (loadError && !insights) {
    return (
      <section className="page-shell">
        <ErrorState
          title={insightsT('unavailableTitle')}
          description={loadError}
        />
      </section>
    )
  }

  if (!insights) {
    return (
      <section className="page-shell">
        <EmptyState
          description={insightsT('emptyDescription')}
          eyebrow={insightsT('intelligenceEyebrow')}
          title={insightsT('emptyTitle')}
        />
      </section>
    )
  }

  return (
    <section className="page-shell insights-page" data-testid="insights-page">
      {aiMeta && (
        <StatusCallout
          tone={aiMeta.tone}
          eyebrow={insightsT('intelligenceEyebrow')}
          title={aiMeta.label}
          body={aiMeta.description}
          actions={
            <div className="intelligence-actions">
              <button
                className="btn-secondary"
                type="button"
                onClick={() => void handleRefreshInsights()}
                disabled={Boolean(action)}
              >
                {insightsT('refreshInsights')}
              </button>
              <Link className="btn-secondary" to="/explorer?mode=hybrid">
                {insightsT('openExplorer')}
              </Link>
              <Link
                className="btn-secondary"
                to={assistantHref(insightsT('assistantSummaryPrompt'))}
              >
                {insightsT('askAssistant')}
              </Link>
            </div>
          }
        />
      )}

      {loadError ? (
        <ErrorState
          title={insightsT('refreshAttentionTitle')}
          description={loadError}
        />
      ) : null}

      <div className="insights-summary">
        <div className="insight-kpi">
          <div className="kpi-label">{insightsT('window')}</div>
          <div className="kpi-value">
            {insightsT('windowDaysCompact', { days: insights.windowDays })}
          </div>
          <div className="kpi-sublabel">
            {insightsT('generatedAt', {
              time: formatRelativeTime(insights.generatedAt, language),
            })}
          </div>
        </div>
        <div className="insight-kpi">
          <div className="kpi-label">{insightsT('cards')}</div>
          <div className="kpi-value">{insights.cards.length}</div>
          <div className="kpi-sublabel">{insightsT('cardsDescription')}</div>
        </div>
        <div className="insight-kpi">
          <div className="kpi-label">{insightsT('topics')}</div>
          <div className="kpi-value">{insights.topics.length}</div>
          <div className="kpi-sublabel">{insightsT('topicsDescription')}</div>
        </div>
        <div className="insight-kpi">
          <div className="kpi-label">{insightsT('coverage')}</div>
          <div className="kpi-value">
            {Math.round(insights.status.contentCoverage * 100)}%
          </div>
          <div className="kpi-sublabel">{insightsT('coverageDescription')}</div>
        </div>
      </div>

      <div className="insights-grid">
        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">{insightsT('onThisDay')}</span>
            <span className="panel-action">{todayKey}</span>
          </div>
          <div className="panel-body intelligence-stack">
            {onThisDay.length > 0 ? (
              onThisDay.map((item) => (
                <Link
                  key={`${item.historyId}-${item.url}`}
                  className="result-row"
                  to={evidenceHref(item)}
                >
                  <div className="result-row__header">
                    <strong>{item.title ?? item.url}</strong>
                    <span className="mono-support">
                      {formatDateTime(item.visitedAt, language) ??
                        item.visitedAt}
                    </span>
                  </div>
                  <p>{item.note ?? insightsT('nothingForDayDescription')}</p>
                </Link>
              ))
            ) : (
              <EmptyState
                description={insightsT('nothingForDayDescription')}
                eyebrow={insightsT('nothingForDayEyebrow')}
                title={insightsT('nothingForDayTitle')}
              />
            )}
          </div>
        </div>

        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">{insightsT('siteAnalytics')}</span>
            <span className="panel-action">
              {insightsT('currentEvidenceSample')}
            </span>
          </div>
          <div className="panel-body">
            {siteAnalytics.length > 0 ? (
              <div className="domain-list">
                {siteAnalytics.map((item, index) => (
                  <Link
                    key={item.domain}
                    className="domain-item"
                    to={evidenceHref({ domain: item.domain })}
                  >
                    <span className="domain-rank mono dim">
                      {String(index + 1).padStart(2, '0')}
                    </span>
                    <span className="domain-name mono">{item.domain}</span>
                    <div className="domain-bar-container">
                      <div
                        className="domain-bar"
                        style={{ width: `${item.pct}%` }}
                      />
                    </div>
                    <span className="domain-count mono">{item.count}</span>
                  </Link>
                ))}
              </div>
            ) : (
              <EmptyState
                description={insightsT('noSiteAnalyticsDescription')}
                eyebrow={insightsT('noSiteAnalyticsEyebrow')}
                title={insightsT('noSiteAnalyticsTitle')}
              />
            )}
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('periodicSummary')}</span>
            <span className="panel-action">
              {insightsT('snapshotLabel', {
                time:
                  formatDateTime(insights.generatedAt, language) ??
                  insights.generatedAt,
              })}
            </span>
          </div>
          <div className="panel-body intelligence-stack">
            {periodicSummary.map((paragraph) => (
              <p key={paragraph} className="summary-text">
                {paragraph}
              </p>
            ))}
            {insights.notes.length > 0 && (
              <div className="intelligence-note-list">
                {insights.notes.map((note) => (
                  <p key={note} className="mono-support">
                    {note}
                  </p>
                ))}
              </div>
            )}
            <div className="summary-stats">
              <div className="summary-stat">
                <span className="dim">{insightsT('threads')}</span>
                <span className="mono">{insights.threads.length}</span>
              </div>
              <div className="summary-stat">
                <span className="dim">{insightsT('cardsStat')}</span>
                <span className="mono">{insights.cards.length}</span>
              </div>
              <div className="summary-stat">
                <span className="dim">{insightsT('topicsStat')}</span>
                <span className="mono">{insights.topics.length}</span>
              </div>
            </div>
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('topicTimeline')}</span>
            <span className="panel-action">
              {insightsT('lastDays', { days: insights.windowDays })}
            </span>
          </div>
          <div className="panel-body">
            <div className="topic-timeline">
              {insights.topics.map((topic, index) => (
                <div key={topic.topicId} className="topic-row">
                  <div className="topic-name">
                    <div
                      className="topic-dot"
                      style={{
                        background: topicColors[index % topicColors.length],
                      }}
                    />
                    <span>{topic.label}</span>
                  </div>
                  <div className="topic-bars">
                    <div className="topic-bar-track">
                      <div
                        className="topic-bar"
                        style={{
                          width: `${Math.min(100, Math.max(12, topic.visitCount * 6))}%`,
                          background: topicColors[index % topicColors.length],
                          opacity: 0.8,
                        }}
                      />
                    </div>
                  </div>
                  <Link
                    className="topic-count mono"
                    to={evidenceHref({ title: topic.label })}
                  >
                    {topic.visitCount}
                  </Link>
                </div>
              ))}
            </div>
            <div className="topic-axis">
              <span>
                {insightsT('windowAxis', { days: insights.windowDays })}
              </span>
              <span>{insights.generatedAt.slice(0, 10)}</span>
            </div>
          </div>
        </div>

        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">{insightsT('insightCards')}</span>
            <span className="panel-action">{insightsT('explainable')}</span>
          </div>
          <div className="panel-body intelligence-result-list">
            {insights.cards.map((card) => (
              <div key={card.cardId} className="result-row">
                <div className="result-row__header">
                  <strong>{card.title}</strong>
                  <span className="mono-support">
                    {card.kind === 'open-loop'
                      ? insightsT('openLoopSignal')
                      : card.kind === 'revisit'
                        ? insightsT('revisitSignal')
                        : card.kind === 'focus-balance'
                          ? insightsT('focusBalanceSignal')
                          : insightsT('genericCard')}
                  </span>
                </div>
                <p>{card.summary}</p>
                <div className="result-row__meta">
                  <span className="mono-support">
                    {insightsT('evidenceItems', {
                      count: card.evidence.length,
                      days: card.windowDays,
                    })}
                  </span>
                  <span className="mono-support">
                    {card.chromiumEnhanced
                      ? insightsT('chromiumEnhanced')
                      : insightsT('crossBrowserSafe')}
                  </span>
                </div>
                <div className="intelligence-actions">
                  <button
                    className="btn-tiny"
                    type="button"
                    onClick={() => void handleExplain(card)}
                    disabled={Boolean(action)}
                  >
                    {insightsT('explain')}
                  </button>
                  <Link
                    className="btn-tiny"
                    to={assistantHref(
                      insightsT('explainCardPrompt', { title: card.title }),
                    )}
                  >
                    {insightsT('askAssistant')}
                  </Link>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {selectedCard && explanation ? (
        <div className="panel intelligence-panel">
          <div className="panel-header">
            <span className="panel-title">{insightsT('explainability')}</span>
            <span className="panel-action">{selectedCard.title}</span>
          </div>
          <div className="panel-body intelligence-stack">
            <p className="summary-text">{explanation.explanation}</p>
            {explanation.notes.length > 0 && (
              <div className="intelligence-note-list">
                {explanation.notes.map((note) => (
                  <p key={note} className="mono-support">
                    {note}
                  </p>
                ))}
              </div>
            )}
            <div className="intelligence-result-list">
              {explanation.citations.map((item: InsightEvidenceItem) => (
                <Link
                  key={`${item.historyId}-${item.url}`}
                  className="result-row"
                  to={evidenceHref(item)}
                >
                  <div className="result-row__header">
                    <strong>{item.title ?? item.url}</strong>
                    <span className="mono-support">
                      {formatDateTime(item.visitedAt, language) ??
                        item.visitedAt}
                    </span>
                  </div>
                  <p>{item.note ?? insightsT('usedToExplain')}</p>
                </Link>
              ))}
            </div>
          </div>
        </div>
      ) : null}

      {action ? <p className="mono-support">{action}…</p> : null}
    </section>
  )
}
