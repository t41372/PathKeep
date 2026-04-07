import { useEffect, useMemo, useState } from 'react'
import { useShellData } from '../../app/shell-data-context'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'
import type { InsightSnapshot } from '../../lib/types'

const topicColors = [
  '#FF7832',
  '#4ECDC4',
  '#FFE66D',
  '#FF6B6B',
  '#C792EA',
  '#89CFF0',
  '#98D8C8',
]

const mockKpis = [
  { label: 'THIS WEEK', value: '1,247', sublabel: 'pages visited' },
  {
    label: 'TOP DOMAIN',
    value: 'github.com',
    sublabel: '342 visits · 27.4%',
    mono: true,
  },
  {
    label: 'EXPLORE / EXPLOIT',
    value: '38% / 62%',
    sublabel: 'Focused mode this week',
  },
  { label: 'ACTIVE THREADS', value: '7', sublabel: '3 new this week' },
]

const mockTopics = [
  {
    name: 'Rust async runtime',
    color: '#FF7832',
    count: 89,
    trend: '↑',
    bars: [
      { left: 0, width: 20, opacity: 0.3 },
      { left: 25, width: 10, opacity: 0.5 },
      { left: 60, width: 15, opacity: 0.7 },
      { left: 85, width: 15, opacity: 1 },
    ],
  },
  {
    name: 'LLM fine-tuning',
    color: '#4ECDC4',
    count: 54,
    trend: '↓',
    bars: [
      { left: 0, width: 30, opacity: 0.8 },
      { left: 35, width: 20, opacity: 0.6 },
      { left: 70, width: 10, opacity: 0.3 },
    ],
  },
  {
    name: 'SQLite internals',
    color: '#FFE66D',
    count: 41,
    trend: '↑',
    bars: [
      { left: 50, width: 10, opacity: 0.4 },
      { left: 65, width: 15, opacity: 0.7 },
      { left: 85, width: 15, opacity: 1 },
    ],
  },
  {
    name: 'Tauri 2 development',
    color: '#FF6B6B',
    count: 73,
    trend: '↑',
    bars: [
      { left: 10, width: 25, opacity: 0.6 },
      { left: 40, width: 20, opacity: 0.8 },
      { left: 75, width: 25, opacity: 1 },
    ],
  },
  {
    name: 'Music production / DAW',
    color: '#C792EA',
    count: 28,
    trend: '—',
    bars: [
      { left: 0, width: 15, opacity: 0.5 },
      { left: 20, width: 8, opacity: 0.3 },
      { left: 55, width: 12, opacity: 0.4 },
      { left: 90, width: 10, opacity: 0.6 },
    ],
  },
]

const mockThreads = [
  {
    name: 'Building PathKeep — browser history parser',
    meta: '5 days active · 89 pages · reopened 3x',
    status: 'hot',
  },
  {
    name: 'Tokio scheduler deep dive',
    meta: '2 days active · 34 pages',
    status: 'hot',
  },
  {
    name: 'SQLite FTS5 vs Tantivy comparison',
    meta: '3 days active · 21 pages',
    status: 'warm',
  },
  {
    name: 'Elden Ring DLC strategy guides',
    meta: '1 day · 12 pages · idle 2d',
    status: 'cool',
  },
]

const mockDomains = [
  { name: 'github.com', count: 342, pct: 100 },
  { name: 'docs.rs', count: 178, pct: 52 },
  { name: 'stackoverflow.com', count: 131, pct: 38 },
  { name: 'google.com', count: 102, pct: 30 },
  { name: 'arxiv.org', count: 68, pct: 20 },
]

export function InsightsPage() {
  const { refreshKey } = useShellData()
  const [insights, setInsights] = useState<InsightSnapshot | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

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
      } catch (e) {
        if (!cancelled)
          setLoadError(
            e instanceof Error ? e.message : 'Failed to load insights',
          )
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    void load()
    return () => {
      cancelled = true
    }
  }, [refreshKey])

  const hasRealData = Boolean(insights && insights.topics.length > 0)

  // Use real data for topics if available, fallback to mock
  const topics = useMemo(() => {
    if (!hasRealData || !insights) return mockTopics
    return insights.topics.map((t, i) => ({
      name: t.label,
      color: topicColors[i % topicColors.length],
      count: t.visitCount,
      trend: t.trendSlope > 0.1 ? '↑' : t.trendSlope < -0.1 ? '↓' : '—',
      bars: [{ left: 20, width: Math.min(80, t.visitCount * 5), opacity: 0.8 }],
    }))
  }, [hasRealData, insights])

  const threads = useMemo(() => {
    if (!hasRealData || !insights) return mockThreads
    return insights.threads.map((t) => ({
      name: t.title,
      meta: `${t.visitCount} pages · reopened ${t.reopenCount}x`,
      status:
        t.openLoopScore > 1.5 ? 'hot' : t.openLoopScore > 0.5 ? 'warm' : 'cool',
    }))
  }, [hasRealData, insights])

  if (loading && !insights)
    return (
      <section className="page-shell">
        <LoadingState label="Loading insights" />
      </section>
    )
  if (loadError && !insights)
    return (
      <section className="page-shell">
        <ErrorState title="Insights unavailable" description={loadError} />
      </section>
    )

  return (
    <section className="page-shell insights-page" data-testid="insights-page">
      {/* KPI Row */}
      <div className="insights-summary">
        {mockKpis.map((kpi) => (
          <div key={kpi.label} className="insight-kpi">
            <div className="kpi-label">{kpi.label}</div>
            <div className={`kpi-value ${kpi.mono ? 'mono' : ''}`}>
              {kpi.value}
            </div>
            <div className="kpi-sublabel">{kpi.sublabel}</div>
          </div>
        ))}
      </div>

      <div className="insights-grid">
        {/* Topic Timeline */}
        <div className="panel panel-wide">
          <div className="panel-header">
            <span className="panel-title">TOPIC TIMELINE · LAST 30 DAYS</span>
            <div className="panel-controls">
              <button className="ctrl-btn active" type="button">
                30D
              </button>
              <button className="ctrl-btn" type="button">
                90D
              </button>
              <button className="ctrl-btn" type="button">
                1Y
              </button>
            </div>
          </div>
          <div className="panel-body">
            <div className="topic-timeline">
              {topics.map((topic) => (
                <div key={topic.name} className="topic-row">
                  <div className="topic-name">
                    <div
                      className="topic-dot"
                      style={{ background: topic.color }}
                    />
                    <span>{topic.name}</span>
                  </div>
                  <div className="topic-bars">
                    <div className="topic-bar-track">
                      {topic.bars.map((bar, j) => (
                        <div
                          key={j}
                          className="topic-bar"
                          style={{
                            left: `${bar.left}%`,
                            width: `${bar.width}%`,
                            opacity: bar.opacity,
                            background: topic.color,
                          }}
                        />
                      ))}
                    </div>
                  </div>
                  <div className="topic-count mono">
                    {topic.trend} {topic.count}
                  </div>
                </div>
              ))}
            </div>
            <div className="topic-axis">
              <span>Mar 6</span>
              <span>Mar 13</span>
              <span>Mar 20</span>
              <span>Mar 27</span>
              <span>Apr 5</span>
            </div>
          </div>
        </div>

        {/* Weekly Summary */}
        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">WEEKLY SUMMARY</span>
          </div>
          <div className="panel-body">
            <div className="summary-text">
              <p>
                This week&apos;s primary focus shifted to{' '}
                <strong>Rust async runtime architecture</strong> and{' '}
                <strong>Tauri 2 desktop development</strong>. Significant time
                was spent reading tokio source code and documentation.
              </p>
              <p style={{ marginTop: 'var(--space-3)' }}>
                Compared to last week, <strong>LLM fine-tuning</strong> interest
                dropped significantly, while <strong>SQLite internals</strong>{' '}
                is a new emerging topic.
              </p>
            </div>
            <div className="summary-stats">
              <div className="summary-stat">
                <span className="dim">New domains</span>
                <span className="mono">14</span>
              </div>
              <div className="summary-stat">
                <span className="dim">Revisited domains</span>
                <span className="mono">47</span>
              </div>
              <div className="summary-stat">
                <span className="dim">Search queries</span>
                <span className="mono">83</span>
              </div>
            </div>
          </div>
        </div>

        {/* Active Threads */}
        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">ACTIVE THREADS</span>
          </div>
          <div className="panel-body">
            {threads.map((thread) => (
              <div key={thread.name} className="thread-item">
                <div className={`thread-status ${thread.status}`} />
                <div className="thread-info">
                  <div className="thread-name">{thread.name}</div>
                  <div className="thread-meta dim mono">{thread.meta}</div>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Top Domains */}
        <div className="panel">
          <div className="panel-header">
            <span className="panel-title">TOP DOMAINS · THIS WEEK</span>
          </div>
          <div className="panel-body">
            <div className="domain-list">
              {mockDomains.map((d, i) => (
                <div key={d.name} className="domain-item">
                  <span className="domain-rank mono dim">
                    {String(i + 1).padStart(2, '0')}
                  </span>
                  <span className="domain-name mono">{d.name}</span>
                  <div className="domain-bar-container">
                    <div
                      className="domain-bar"
                      style={{ width: `${d.pct}%` }}
                    />
                  </div>
                  <span className="domain-count mono">{d.count}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}
