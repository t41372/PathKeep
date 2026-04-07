import { useEffect, useState } from 'react'
import { useApp } from '../lib/app-context'
import { formatDateTime } from '../lib/format'
import { EmptyState, Glyph, StatusTag, Surface } from '../components/ui'
import { backend } from '../lib/backend'
import type {
  InsightExplanation,
  InsightSnapshot,
  InsightThreadDetail,
  RunInsightsReport,
} from '../lib/types'

export function InsightsPage() {
  const {
    t,
    resolvedLanguage,
    initialized,
    unlocked,
    snapshot,
    insightStatus,
    runTask,
    setNotice,
    setError,
  } = useApp()

  const [insightSnapshot, setInsightSnapshot] =
    useState<InsightSnapshot | null>(null)
  const [insightRunReport, setInsightRunReport] =
    useState<RunInsightsReport | null>(null)
  const [profileFilter, setProfileFilter] = useState('')
  const [windowDays, setWindowDays] = useState(30)
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(null)
  const [threadDetail, setThreadDetail] = useState<InsightThreadDetail | null>(
    null,
  )
  const [explanation, setExplanation] = useState<InsightExplanation | null>(
    null,
  )
  const [selectedInsightLabel, setSelectedInsightLabel] = useState<
    string | null
  >(null)

  const profiles = snapshot?.browserProfiles ?? []
  const activeSelectedThreadId = insightSnapshot?.threads.some(
    (thread) => thread.threadId === selectedThreadId,
  )
    ? selectedThreadId
    : (insightSnapshot?.threads[0]?.threadId ?? null)
  const visibleThreadDetail =
    threadDetail?.summary.threadId === activeSelectedThreadId
      ? threadDetail
      : null

  // Load insights on mount/filter change
  useEffect(() => {
    if (!initialized || !unlocked) return
    let cancelled = false

    void (async () => {
      try {
        const next = await backend.loadInsights({
          profileId: profileFilter || null,
          windowDays,
          fullRebuild: false,
          limit: null,
        })
        if (!cancelled) setInsightSnapshot(next)
      } catch (taskError) {
        if (!cancelled) {
          setError(
            taskError instanceof Error ? taskError.message : String(taskError),
          )
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [initialized, unlocked, profileFilter, windowDays, setError])

  // Thread detail loading
  useEffect(() => {
    if (!initialized || !unlocked || !activeSelectedThreadId) return
    let cancelled = false

    void (async () => {
      try {
        const detail = await backend.loadThreadDetail(activeSelectedThreadId)
        if (!cancelled) setThreadDetail(detail)
      } catch (taskError) {
        if (!cancelled) {
          setError(
            taskError instanceof Error ? taskError.message : String(taskError),
          )
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [activeSelectedThreadId, initialized, unlocked, setError])

  async function handleRunInsights() {
    await runTask(t('runInsightsNow'), async () => {
      const report = await backend.runInsightsNow({
        profileId: profileFilter || null,
        windowDays,
        fullRebuild: false,
        limit: null,
      })
      setInsightRunReport(report)
      // Reload insights data
      const next = await backend.loadInsights({
        profileId: profileFilter || null,
        windowDays,
        fullRebuild: false,
        limit: null,
      })
      setInsightSnapshot(next)
      setNotice(t('insightsUpdated'))
    })
  }

  async function handleExplainInsight(insightId: string, insightKind: string) {
    await runTask(t('explainInsight'), async () => {
      const result = await backend.explainInsight({
        insightId,
        insightKind,
        profileId: profileFilter || null,
        windowDays,
      })
      setExplanation(result)
      setSelectedInsightLabel(insightId)
    })
  }

  const generatedAt =
    formatDateTime(
      insightSnapshot?.generatedAt ?? insightStatus.lastRunAt,
      resolvedLanguage,
    ) ?? t('notAvailable')
  const coverage = `${Math.round(insightStatus.contentCoverage * 100)}%`

  return (
    <div className="pageContent">
      <section className="pageIntro">
        <p className="sectionEyebrow">{t('insightsNav')}</p>
        <h2>{t('analysisDescription')}</h2>
      </section>

      {!initialized || !unlocked ? (
        <EmptyState icon="lock" message={t('archiveLocked')} />
      ) : (
        <>
          {/* Controls */}
          <Surface
            eyebrow={t('insightsNav')}
            title={t('analysisSection')}
            icon="neurology"
          >
            <div className="insightControls">
              <select
                className="selectInput"
                value={profileFilter}
                onChange={(e) => {
                  setProfileFilter(e.target.value)
                  setExplanation(null)
                  setSelectedInsightLabel(null)
                  setThreadDetail(null)
                }}
              >
                <option value="">{t('allProfiles')}</option>
                {profiles.map((p) => (
                  <option key={p.profileId} value={p.profileId}>
                    {p.profileName} ({p.browserName})
                  </option>
                ))}
              </select>
              <select
                className="selectInput"
                value={windowDays}
                onChange={(e) => {
                  setWindowDays(Number(e.target.value))
                  setExplanation(null)
                  setSelectedInsightLabel(null)
                  setThreadDetail(null)
                }}
              >
                <option value={7}>7 {t('days')}</option>
                <option value={14}>14 {t('days')}</option>
                <option value={30}>30 {t('days')}</option>
                <option value={60}>60 {t('days')}</option>
                <option value={90}>90 {t('days')}</option>
              </select>
              <button
                className="primaryButton"
                type="button"
                onClick={handleRunInsights}
              >
                <Glyph icon="play_arrow" filled />
                {t('runInsightsNow')}
              </button>
            </div>

            <div className="insightMeta">
              <span className="muted">
                {t('insightsGeneratedAt')}: {generatedAt}
              </span>
              <span className="muted">
                {t('insightCoverage')}: {coverage}
              </span>
            </div>
          </Surface>

          {/* Insight cards */}
          {insightSnapshot?.cards.length ? (
            <Surface
              eyebrow={t('insightCards')}
              title={t('insightCards')}
              icon="auto_awesome"
            >
              <div className="cardList">
                {insightSnapshot.cards.map((card) => (
                  <div className="insightCard" key={card.cardId}>
                    <div className="insightCardHeader">
                      <StatusTag tone="info">{card.kind}</StatusTag>
                      <strong>{card.title}</strong>
                    </div>
                    <p>{card.summary}</p>
                    <button
                      className="secondaryButton"
                      type="button"
                      onClick={() =>
                        handleExplainInsight(card.cardId, card.kind)
                      }
                    >
                      <Glyph icon="lightbulb" />
                      {t('explainInsight')}
                    </button>
                  </div>
                ))}
              </div>
            </Surface>
          ) : null}

          {/* Explanation */}
          {explanation && selectedInsightLabel && (
            <Surface
              eyebrow={t('explainInsight')}
              title={selectedInsightLabel}
              icon="lightbulb"
            >
              <p>{explanation.explanation}</p>
              {explanation.citations.length > 0 && (
                <div className="citationList">
                  {explanation.citations.map((cite, i) => (
                    <div key={i} className="citation">
                      <a
                        href={cite.url}
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        {cite.title ?? cite.url}
                      </a>
                      <span className="muted">
                        {formatDateTime(cite.visitedAt, resolvedLanguage)}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </Surface>
          )}

          {/* Topics */}
          {insightSnapshot?.topics.length ? (
            <Surface
              eyebrow={t('topicsSection')}
              title={t('topicsSection')}
              icon="topic"
            >
              <div className="topicList">
                {insightSnapshot.topics.map((topic) => (
                  <div key={topic.topicId} className="topicRow">
                    <strong>{topic.label}</strong>
                    <span className="muted">
                      {topic.visitCount} {t('visits')} · {t('trendSlope')}:{' '}
                      {topic.trendSlope.toFixed(2)}
                    </span>
                  </div>
                ))}
              </div>
            </Surface>
          ) : null}

          {/* Threads */}
          {insightSnapshot?.threads.length ? (
            <Surface
              eyebrow={t('threadsSection')}
              title={t('threadsSection')}
              icon="forum"
            >
              <div className="threadList">
                {insightSnapshot.threads.map((thread) => (
                  <button
                    key={thread.threadId}
                    className={`threadRow ${activeSelectedThreadId === thread.threadId ? 'selected' : ''}`}
                    type="button"
                    onClick={() => setSelectedThreadId(thread.threadId)}
                  >
                    <div className="threadHeader">
                      <StatusTag
                        tone={
                          thread.status === 'open-loop' ? 'info' : 'success'
                        }
                      >
                        {thread.status}
                      </StatusTag>
                      <strong>{thread.title}</strong>
                    </div>
                    <span className="muted">
                      {thread.visitCount} {t('visits')} · {thread.reopenCount}{' '}
                      {t('reopens')}
                    </span>
                  </button>
                ))}
              </div>

              {/* Thread detail  */}
              {visibleThreadDetail && (
                <div className="threadDetail">
                  <h4>{visibleThreadDetail.summary.title}</h4>
                  {visibleThreadDetail.visits.map((visit, i) => (
                    <div key={i} className="threadVisit">
                      <a
                        href={visit.url}
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        {visit.title ?? visit.url}
                      </a>
                      <span className="muted">
                        {formatDateTime(visit.visitedAt, resolvedLanguage)}
                      </span>
                      {visit.note && (
                        <span className="muted">{visit.note}</span>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </Surface>
          ) : null}

          {/* Insight run report */}
          {insightRunReport && (
            <Surface
              eyebrow={t('insightRunReport')}
              title={t('insightRunReport')}
              icon="summarize"
            >
              <div className="runReportGrid">
                <div>
                  {t('processedVisits')}: {insightRunReport.processedVisits}
                </div>
                <div>
                  {t('enrichedVisits')}: {insightRunReport.enrichedVisits}
                </div>
                <div>
                  {t('insightTopicsCount')}: {insightRunReport.topicCount}
                </div>
                <div>
                  {t('insightThreadsCount')}: {insightRunReport.threadCount}
                </div>
                <div>
                  {t('insightCardsCount')}: {insightRunReport.cardCount}
                </div>
              </div>
            </Surface>
          )}
        </>
      )}
    </div>
  )
}
