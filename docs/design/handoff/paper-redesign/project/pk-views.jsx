/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Home View (v2, more innovative)
   Editorial layout, visual rhythm, compact heatmap,
   On This Day hero, inline stats, active threads.
   ═══════════════════════════════════════════════════════════ */

const ON_THIS_DAY = [
{ id: 'otd1', title: 'Understanding Gaussian Splatting — A Visual Guide', domain: 'medium.com', time: '14:23', url: 'https://medium.com/@neural3d/gaussian-splatting-guide' },
{ id: 'otd2', title: '3D Gaussian Splatting for Real-Time Radiance Field Rendering', domain: 'arxiv.org', time: '14:45', url: 'https://arxiv.org/abs/2308.14737' },
{ id: 'otd3', title: 'tauri-apps/tauri: Build desktop apps with web technology', domain: 'github.com', time: '15:12', url: 'https://github.com/tauri-apps/tauri' },
{ id: 'otd4', title: 'Getting Started | Tauri — v2.tauri.app', domain: 'v2.tauri.app', time: '15:30', url: 'https://v2.tauri.app/start/' },
{ id: 'otd5', title: 'Tauri vs Electron: Real-world Performance Comparison', domain: 'blog.logrocket.com', time: '16:02', url: 'https://blog.logrocket.com/tauri-vs-electron/' }];


const ACTIVE_THREADS = [
{ id: 't1', title: 'Building PathKeep — browser history parser', pages: 89, days: 12, lastTouched: 'today', tone: 'hot' },
{ id: 't2', title: 'Tokio scheduler deep dive', pages: 34, days: 3, lastTouched: 'today', tone: 'hot' },
{ id: 't3', title: 'SQLite FTS5 vs Tantivy comparison', pages: 21, days: 4, lastTouched: 'yesterday', tone: 'warm' },
{ id: 't4', title: 'Tauri 2 plugin architecture', pages: 18, days: 2, lastTouched: 'yesterday', tone: 'warm' },
{ id: 't5', title: 'Wavetable synthesis & Vital', pages: 12, days: 1, lastTouched: '3d ago', tone: 'cool' }];


const REFIND_SHELF = [
{ id: 'r1', title: 'tokio-rs/tokio: A runtime for writing reliable async applications', domain: 'github.com', visits: 47, span: 'over 11 months' },
{ id: 'r2', title: 'docs.rs — sqlx', domain: 'docs.rs', visits: 31, span: 'over 6 months' },
{ id: 'r3', title: 'Tauri v2 IPC Communication', domain: 'v2.tauri.app', visits: 28, span: 'over 4 months' },
{ id: 'r4', title: 'serde_json documentation', domain: 'docs.rs', visits: 25, span: 'over 8 months' }];


function HomeView({ onNavigate, onSelectEntry }) {
  const today = new Date();
  const hour = today.getHours();
  const greeting = hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening';
  const heatmapData = React.useMemo(() => generateHeatmapData(), []);

  return (
    <div style={{ maxWidth: 1080 }}>

      {/* ═══ HERO BAND — greeting + inline stats ═══ */}
      <div style={{
        display: 'grid',
        gridTemplateColumns: '1fr auto',
        gap: 40,
        alignItems: 'end',
        marginBottom: 28,
        paddingBottom: 20,
        borderBottom: '1px solid var(--border-light)'
      }}>
        <div>
          <h1 className="home-greeting" style={{ marginBottom: 6 }}>{greeting}</h1>
          <p style={{
            fontFamily: 'var(--font-serif)', fontStyle: 'italic',
            fontSize: 15, color: 'var(--ink-muted)', margin: 0,
            lineHeight: 1.5, maxWidth: 500
          }}>
            12 days away. +18,394 pages quietly kept. Everything is here.
          </p>
        </div>
        <div style={{
          display: 'flex', gap: 28,
          alignItems: 'flex-end'
        }}>
          {[
          { value: '2.8M', label: 'Pages' },
          { value: '4y 7m', label: 'Span' },
          { value: '12.4', label: 'GB' },
          { value: '3', label: 'Sources' }].
          map((s) =>
          <div key={s.label} style={{ textAlign: 'center' }}>
              <div style={{
              fontFamily: 'var(--font-serif)', fontSize: 24,
              fontWeight: 400, color: 'var(--ink)',
              letterSpacing: '-0.02em', lineHeight: 1
            }}>{s.value}</div>
              <div style={{
              fontFamily: 'var(--font-mono)', fontSize: 9,
              color: 'var(--ink-faint)', textTransform: 'uppercase',
              letterSpacing: '0.08em', marginTop: 4
            }}>{s.label}</div>
            </div>
          )}
        </div>
      </div>

      {/* ═══ ROW 1 — On This Day (hero) + This Week summary ═══ */}
      <div style={{
        display: 'grid',
        gridTemplateColumns: '1fr 1fr',
        gap: 16,
        marginBottom: 16
      }}>
        {/* On This Day — full left half, hero treatment */}
        <div className="pk-card pk-card--accent" style={{ minHeight: 0 }}>
          <div className="pk-card-header" data-comment-anchor="8322eb8f18-div-94-11">
            <span className="pk-card-title">On this day, a year ago</span>
            <button
              type="button"
              className="otd-jump"
              onClick={() => onNavigate('browse', { date: '2025-05-17' })}
              title="Open May 17, 2025 in Browse">
              May 17, 2025
              <span aria-hidden="true" style={{ marginLeft: 6 }}>→</span>
            </button>
          </div>
          <div className="pk-card-body" style={{ padding: '14px 18px' }}>
            {ON_THIS_DAY.slice(0, 4).map((item, i) =>
            <div key={i} className="otd-entry" style={{ cursor: 'pointer', padding: '8px 0' }} onClick={() => onSelectEntry && onSelectEntry(item)}>
                <div className="otd-icon" style={{ background: getDomainColor(item.domain), width: 28, height: 28, fontSize: 9 }}>
                  {getDomainAbbr(item.domain)}
                </div>
                <div className="otd-text">
                  <div className="otd-title" style={{ fontSize: 13 }}>{item.title}</div>
                  <div className="otd-domain">{item.domain}</div>
                </div>
              </div>
            )}
            <div className="otd-summary pk-llm-needed" data-comment-anchor="155cb90598-div-157-15" style={{
              marginTop: 10, paddingTop: 10, fontSize: 13
            }}>
              Deep in 3D Gaussian Splatting — and evaluating Tauri as your desktop framework. The seed of this app.
            </div>
            <div style={{
              fontFamily: 'var(--font-mono)', fontSize: 9,
              color: 'var(--ink-ghost)', marginTop: 6
            }}>
              ◌ Summary needs local LLM · falls back to page listing
            </div>
          </div>
        </div>

        {/* This Week */}
        <div className="pk-card">
          <div className="pk-card-header">
            <span className="pk-card-title">This week</span>
            <span className="pk-card-badge">Week 20</span>
          </div>
          <div className="pk-card-body" style={{ padding: '14px 18px' }}>
            <div className="pk-llm-needed" style={{ position: 'relative', paddingRight: 50 }}>
              <p style={{ fontFamily: 'var(--font-serif)', fontSize: 14, color: 'var(--ink-secondary)', lineHeight: 1.55, margin: '0 0 8px' }}>
                Your week was <strong>Rust async runtimes</strong> and <strong>Tauri 2 plugins</strong>. Lots of tokio source code on GitHub and docs.rs.
              </p>
              <p style={{ fontFamily: 'var(--font-serif)', fontSize: 14, color: 'var(--ink-secondary)', lineHeight: 1.55, margin: 0 }}>
                <strong>LLM fine-tuning</strong> fell −38%. <strong>SQLite internals</strong> newly emerging.
              </p>
            </div>
            <div style={{
              display: 'flex', gap: 0,
              marginTop: 14, paddingTop: 12,
              borderTop: '1px dashed var(--border-light)'
            }}>
              {[
              { val: '1,247', label: 'Pages' },
              { val: '14', label: 'New domains' },
              { val: '83', label: 'Searches' },
              { val: '7', label: 'Threads' }].
              map((s) =>
              <div key={s.label} style={{ flex: 1 }}>
                  <div style={{ fontFamily: 'var(--font-serif)', fontSize: 20, color: 'var(--ink)', fontWeight: 400, letterSpacing: '-0.01em' }}>{s.val}</div>
                  <div style={{ fontFamily: 'var(--font-mono)', fontSize: 8.5, color: 'var(--ink-faint)', textTransform: 'uppercase', letterSpacing: '0.06em', marginTop: 2 }}>{s.label}</div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* ═══ ROW 2 — Heatmap (full width, compact) ═══ */}
      <div className="pk-card" style={{ marginBottom: 16 }}>
        <div className="pk-card-header" style={{ padding: '10px 18px' }}>
          <span className="pk-card-title">A year in pages</span>
          <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
            <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--ink-faint)' }}>
              Busiest: <span style={{ color: 'var(--ink-secondary)' }}>Apr 5 · 218p</span>
            </span>
            <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--ink-faint)' }}>
              Streak: <span style={{ color: 'var(--ink-secondary)' }}>43d</span>
            </span>
            <span className="pk-card-badge" style={{ cursor: 'pointer' }} onClick={() => onNavigate('insights')}>Insights →</span>
          </div>
        </div>
        <div className="pk-card-body" style={{ padding: '10px 18px 14px' }}>
          <PKHeatmap data={heatmapData} onSelectDate={() => onNavigate('browse')} />
        </div>
      </div>

      {/* ═══ ROW 3 — 3-column: Threads + Archive + Quick links ═══ */}
      <div style={{
        display: 'grid',
        gridTemplateColumns: '1fr 1fr 1fr',
        gap: 16,
        marginBottom: 16
      }}>
        {/* Active threads */}
        <div className="pk-card" style={{ gridColumn: 'span 2' }}>
          <div className="pk-card-header">
            <span className="pk-card-title">What you've been thinking about</span>
            <span className="pk-card-badge" style={{ cursor: 'pointer' }} onClick={() => onNavigate('insights')}>All threads →</span>
          </div>
          <div className="pk-card-body" style={{ padding: '4px 18px 10px' }}>
            {ACTIVE_THREADS.map((thread) =>
            <div key={thread.id} className="pk-thread" onClick={() => onNavigate('insights')}>
                <div className={`pk-thread__pulse ${thread.tone === 'warm' ? 'pk-thread__pulse--warm' : thread.tone === 'cool' ? 'pk-thread__pulse--cool' : ''}`}></div>
                <div className="pk-thread__main">
                  <div className="pk-thread__title">{thread.title}</div>
                  <div className="pk-thread__meta">{thread.days}d · {thread.lastTouched}</div>
                </div>
                <div className="pk-thread__count">{thread.pages}<small>pages</small></div>
              </div>
            )}
          </div>
        </div>

        {/* Archive card — compact */}
        <div className="pk-card">
          <div className="pk-card-header">
            <span className="pk-card-title">Your archive</span>
          </div>
          <div className="pk-card-body" style={{ padding: '12px 16px' }}>
            <div style={{
              fontFamily: 'var(--font-mono)', fontSize: 10.5,
              color: 'var(--ink-muted)', wordBreak: 'break-all',
              lineHeight: 1.4, padding: '6px 8px',
              background: 'var(--bg-page)', borderRadius: 'var(--radius)',
              marginBottom: 10
            }}>~/Library/.../PathKeep/archive.db</div>

            {[
            ['Core archive', '8.2 GB'],
            ['FTS5 index', '1.8 GB'],
            ['Snapshots', '0.8 GB']].
            map(([k, v]) =>
            <div key={k} style={{
              display: 'flex', justifyContent: 'space-between',
              padding: '4px 0', fontSize: 12, color: 'var(--ink-secondary)'
            }}>
                <span>{k}</span>
                <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--ink-muted)' }}>{v}</span>
              </div>
            )}

            <div style={{
              marginTop: 10, paddingTop: 8,
              borderTop: '1px solid var(--border-light)',
              fontFamily: 'var(--font-serif)', fontStyle: 'italic',
              fontSize: 12, color: 'var(--ink-faint)', lineHeight: 1.4
            }}>
              Chain verified · <code style={{ fontFamily: 'var(--font-mono)', fontSize: 10, fontStyle: 'normal', color: 'var(--ink-muted)' }}>0a4c…ef82</code>
            </div>

            <div style={{ marginTop: 10, display: 'flex', gap: 4 }}>
              <button className="btn-secondary" style={{ padding: '4px 8px', fontSize: 11, flex: 1 }}>Export</button>
              <button className="btn-secondary" style={{ padding: '4px 8px', fontSize: 11, flex: 1 }}>Reveal</button>
            </div>
          </div>
        </div>
      </div>

      {/* ═══ QUIET FOOTER ═══ */}
      <div style={{
        textAlign: 'center', marginTop: 24, paddingTop: 20,
        borderTop: '1px solid var(--border-light)',
        fontFamily: 'var(--font-serif)', fontStyle: 'italic',
        fontSize: 13, color: 'var(--ink-faint)', letterSpacing: 0,
        paddingBottom: 8
      }}>
        Local-first. 0 network requests. Your reading life, kept safe.
      </div>
    </div>);

}

/* ── Placeholder views ── */
function PlaceholderView({ title, description }) {
  return (
    <div className="pk-empty">
      <div className="pk-empty-text">{description || 'This view is being crafted.'}</div>
      <div className="pk-empty-attr">— PathKeep v0.3.0</div>
    </div>);
}

Object.assign(window, { HomeView, PlaceholderView, ON_THIS_DAY, ACTIVE_THREADS, REFIND_SHELF });