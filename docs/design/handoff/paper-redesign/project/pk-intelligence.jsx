/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Intelligence View
   Topics, threads, domains, sessions
   ═══════════════════════════════════════════════════════════ */

const TOPICS = [
  { name: 'Rust async runtime', color: '#3d5a80', count: 89, trend: 'up', bars: [
    { left: 0, width: 18, opacity: 0.3 },
    { left: 22, width: 12, opacity: 0.5 },
    { left: 40, width: 15, opacity: 0.6 },
    { left: 60, width: 18, opacity: 0.75 },
    { left: 82, width: 18, opacity: 1.0 }
  ]},
  { name: 'Tauri 2 development', color: '#6b4c3b', count: 73, trend: 'up', bars: [
    { left: 5, width: 22, opacity: 0.55 },
    { left: 32, width: 18, opacity: 0.7 },
    { left: 55, width: 14, opacity: 0.85 },
    { left: 75, width: 25, opacity: 1.0 }
  ]},
  { name: 'LLM fine-tuning', color: '#8b4049', count: 54, trend: 'down', bars: [
    { left: 0, width: 28, opacity: 0.9 },
    { left: 32, width: 22, opacity: 0.65 },
    { left: 60, width: 12, opacity: 0.4 },
    { left: 78, width: 8, opacity: 0.25 }
  ]},
  { name: 'SQLite internals', color: '#7e8d50', count: 41, trend: 'up', bars: [
    { left: 50, width: 12, opacity: 0.4 },
    { left: 65, width: 14, opacity: 0.65 },
    { left: 82, width: 18, opacity: 0.95 }
  ]},
  { name: 'Wavetable synthesis', color: '#a47e58', count: 28, trend: 'flat', bars: [
    { left: 0, width: 15, opacity: 0.5 },
    { left: 22, width: 8, opacity: 0.35 },
    { left: 55, width: 13, opacity: 0.45 },
    { left: 88, width: 10, opacity: 0.55 }
  ]},
  { name: 'Personal knowledge archives', color: '#4a6c70', count: 22, trend: 'up', bars: [
    { left: 40, width: 10, opacity: 0.5 },
    { left: 60, width: 12, opacity: 0.7 },
    { left: 80, width: 18, opacity: 0.9 }
  ]}
];

const TOP_DOMAINS = [
  { rank: 1, name: 'github.com', count: 342, width: 100 },
  { rank: 2, name: 'docs.rs', count: 178, width: 52 },
  { rank: 3, name: 'stackoverflow.com', count: 131, width: 38 },
  { rank: 4, name: 'google.com', count: 102, width: 30 },
  { rank: 5, name: 'arxiv.org', count: 68, width: 20 },
  { rank: 6, name: 'v2.tauri.app', count: 54, width: 16 },
  { rank: 7, name: 'developer.mozilla.org', count: 47, width: 14 },
  { rank: 8, name: 'reddit.com', count: 38, width: 11 }
];

const SESSIONS = [
  { id: 's1', when: 'Today, 20:15 → 22:42', label: 'Rust async runtime deep dive', pages: 22, domains: 4 },
  { id: 's2', when: 'Today, 17:30 → 18:45', label: 'SQLite and Rust bindings', pages: 6, domains: 2 },
  { id: 's3', when: 'Yesterday, 21:00 → 22:30', label: 'Tauri 2 plugin development', pages: 5, domains: 3 },
  { id: 's4', when: 'Yesterday, 15:30 → 17:00', label: 'Music production / synthesis', pages: 5, domains: 4 },
  { id: 's5', when: 'May 14, 20:00 → 22:00', label: 'PathKeep development research', pages: 6, domains: 5 }
];

function IntelligenceView({ onNavigate, onSelectEntry }) {
  return (
    <div>
      {/* KPI row */}
      <div className="intel-kpis">
        <div className="intel-kpi">
          <div className="intel-kpi__label">This week</div>
          <div className="intel-kpi__value">1,247</div>
          <div className="intel-kpi__sub">pages · ↑ 14% vs last week</div>
        </div>
        <div className="intel-kpi">
          <div className="intel-kpi__label">Top domain</div>
          <div className="intel-kpi__value intel-kpi__value--mono">github.com</div>
          <div className="intel-kpi__sub">342 visits · 27.4%</div>
        </div>
        <div className="intel-kpi">
          <div className="intel-kpi__label">Explore / exploit</div>
          <div className="intel-kpi__value">38<span style={{color:'var(--ink-faint)', fontSize:18}}> / 62</span></div>
          <div className="intel-kpi__sub">focused mode</div>
        </div>
        <div className="intel-kpi">
          <div className="intel-kpi__label">Active threads</div>
          <div className="intel-kpi__value">7</div>
          <div className="intel-kpi__sub">3 new this week</div>
        </div>
      </div>

      {/* Topic timeline — full width */}
      <div className="pk-card" style={{marginBottom: 20}}>
        <div className="pk-card-header">
          <span className="pk-card-title">Topics, over the last 30 days</span>
          <span className="pk-card-badge">
            <span style={{cursor:'pointer'}}>30D</span> · <span style={{cursor:'pointer', color:'var(--ink-ghost)'}}>90D</span> · <span style={{cursor:'pointer', color:'var(--ink-ghost)'}}>1Y</span>
          </span>
        </div>
        <div className="pk-card-body">
          {TOPICS.map(topic => (
            <div key={topic.name} className="intel-topic-row">
              <div className="intel-topic-name">
                <span className="intel-topic-dot" style={{background: topic.color}}></span>
                <span className="intel-topic-label">{topic.name}</span>
              </div>
              <div className="intel-topic-track">
                {topic.bars.map((bar, i) => (
                  <div
                    key={i}
                    className="intel-topic-bar"
                    style={{
                      left: `${bar.left}%`,
                      width: `${bar.width}%`,
                      background: topic.color,
                      opacity: bar.opacity
                    }}
                  />
                ))}
              </div>
              <div className={`intel-topic-count ${topic.trend === 'up' ? 'intel-topic-count--up' : topic.trend === 'down' ? 'intel-topic-count--down' : ''}`}>
                {topic.trend === 'up' ? '↑' : topic.trend === 'down' ? '↓' : '—'} {topic.count}
              </div>
            </div>
          ))}
          <div className="intel-topic-axis">
            <span>Apr 17</span>
            <span>Apr 24</span>
            <span>May 1</span>
            <span>May 8</span>
            <span>May 17</span>
          </div>
          <div style={{
            marginTop: 16,
            paddingTop: 12,
            borderTop: '1px dashed var(--border-light)',
            fontFamily: 'var(--font-serif)',
            fontStyle: 'italic',
            fontSize: 13,
            color: 'var(--ink-muted)',
            lineHeight: 1.5,
          }} className="pk-llm-needed">
            Your week was dominated by Rust internals and Tauri plugin work. Music production receded into the background. <strong>Personal knowledge archives</strong> is a newly emerging interest — worth watching.
          </div>
        </div>
      </div>

      {/* Two columns below */}
      <div className="intel-grid">
        <div style={{display: 'flex', flexDirection: 'column', gap: 20}}>

          {/* Top domains */}
          <div className="pk-card">
            <div className="pk-card-header">
              <span className="pk-card-title">Where you spent your time</span>
              <span className="pk-card-badge">This week</span>
            </div>
            <div className="pk-card-body" style={{padding: '14px 18px'}}>
              {TOP_DOMAINS.map(d => (
                <div key={d.rank} className="intel-domain-row">
                  <span className="intel-domain-rank">{String(d.rank).padStart(2,'0')}</span>
                  <span className="intel-domain-name">{d.name}</span>
                  <div className="intel-domain-bar-wrap">
                    <div className="intel-domain-bar" style={{width: `${d.width}%`}}></div>
                  </div>
                  <span className="intel-domain-count">{d.count}</span>
                </div>
              ))}
            </div>
          </div>

          {/* Recent sessions */}
          <div className="pk-card">
            <div className="pk-card-header">
              <span className="pk-card-title">Recent sessions</span>
              <span className="pk-card-badge">A session = pages in one sitting</span>
            </div>
            <div className="pk-card-body" style={{padding: '6px 18px 14px'}}>
              {SESSIONS.map(s => (
                <div key={s.id} className="pk-thread" style={{cursor:'pointer'}} onClick={() => onNavigate('browse')}>
                  <div className="pk-thread__pulse"></div>
                  <div className="pk-thread__main">
                    <div className="pk-thread__title">{s.label}</div>
                    <div className="pk-thread__meta">{s.when} · {s.domains} domains</div>
                  </div>
                  <div className="pk-thread__count">
                    {s.pages}<small>pages</small>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>

        <div style={{display:'flex', flexDirection:'column', gap:20}}>

          {/* Active threads */}
          <div className="pk-card">
            <div className="pk-card-header">
              <span className="pk-card-title">Active threads</span>
            </div>
            <div className="pk-card-body" style={{padding: '6px 18px 14px'}}>
              {ACTIVE_THREADS.map(thread => (
                <div key={thread.id} className="pk-thread">
                  <div className={`pk-thread__pulse ${thread.tone === 'warm' ? 'pk-thread__pulse--warm' : thread.tone === 'cool' ? 'pk-thread__pulse--cool' : ''}`}></div>
                  <div className="pk-thread__main">
                    <div className="pk-thread__title">{thread.title}</div>
                    <div className="pk-thread__meta">
                      {thread.days}d · last touched {thread.lastTouched}
                    </div>
                  </div>
                  <div className="pk-thread__count">{thread.pages}<small>pages</small></div>
                </div>
              ))}
            </div>
          </div>

          {/* Refind candidates */}
          <div className="pk-card">
            <div className="pk-card-header">
              <span className="pk-card-title">Refind candidates</span>
              <span className="pk-card-badge">3+ visits / 90d</span>
            </div>
            <div className="pk-card-body" style={{padding: '6px 18px 14px'}}>
              {REFIND_SHELF.map(item => (
                <div
                  key={item.id}
                  className="otd-entry"
                  style={{cursor:'pointer'}}
                  onClick={() => onSelectEntry(item)}>
                  <div className="otd-icon" style={{background: getDomainColor(item.domain)}}>
                    {getDomainAbbr(item.domain)}
                  </div>
                  <div className="otd-text">
                    <div className="otd-title">{item.title}</div>
                    <div className="otd-domain">{item.visits} visits · {item.span}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Note about LLM */}
          <div className="pk-spec-note">
            <strong>Note on intelligence</strong>
            Topic clustering, weekly summaries, and refind ranking require local LLM + embeddings. When these are unavailable, Intelligence falls back to deterministic signals: domain frequency, visit recency, session boundaries from idle gaps.
          </div>
        </div>
      </div>
    </div>);
}

Object.assign(window, { IntelligenceView, TOPICS, TOP_DOMAINS, SESSIONS });
