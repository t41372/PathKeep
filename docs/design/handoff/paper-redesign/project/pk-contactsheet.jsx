/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Contact Sheet View (v2)
   Infinite vertical scroll, domain stacking, list/card toggle
   ═══════════════════════════════════════════════════════════ */

/* ── Mock browsing data ── */
const BROWSE_DAYS = [
{
  date: '2026-05-16', label: 'Friday, May 16', year: 2026, totalVisits: 22,
  sessions: [
  { timeRange: '20:15 — 21:42', label: 'Rust async runtime deep dive', visits: [
    { id: 'a1', time: '21:42', title: 'tokio-rs/tokio: A runtime for writing reliable async applications', domain: 'github.com', url: 'https://github.com/tokio-rs/tokio', type: 'link', visitCount: 47, typedCount: 12, source: 'Chrome / Default', titleVersions: [{ date: 'v2 · 2026-03-12', title: 'tokio-rs/tokio: Async runtime for Rust' }, { date: 'v1 · 2024-11-08', title: 'tokio-rs/tokio: A runtime for writing reliable async apps' }] },
    { id: 'a2', time: '21:38', title: 'tokio/tokio/src/runtime/scheduler at main', domain: 'github.com', url: 'https://github.com/tokio-rs/tokio/tree/main/tokio/src/runtime', type: 'link', visitCount: 3 },
    { id: 'a3', time: '21:31', title: 'Issues · tokio-rs/tokio · Scheduler improvements', domain: 'github.com', url: 'https://github.com/tokio-rs/tokio/issues/6247', type: 'link', visitCount: 2 },
    { id: 'a4', time: '21:15', title: 'Work-stealing scheduler design in Tokio', domain: 'tokio.rs', url: 'https://tokio.rs/blog/2019-10-scheduler', type: 'link', visitCount: 1 },
    { id: 'a5', time: '20:52', title: 'Understanding Rust Futures and async/await', domain: 'fasterthanli.me', url: 'https://fasterthanli.me/articles/understanding-rust-futures', type: 'typed', visitCount: 8, typedCount: 3 },
    { id: 'a6', time: '20:38', title: 'Asynchronous Programming in Rust — The Book', domain: 'rust-lang.github.io', url: 'https://rust-lang.github.io/async-book/', type: 'link', visitCount: 14 },
    { id: 'a7', time: '20:15', title: 'rust async runtime comparison 2026 — Google Search', domain: 'google.com', url: 'https://google.com/search?q=rust+async+runtime+comparison+2026', type: 'typed' }]
  },
  { timeRange: '19:00 — 20:05', label: 'Transformer architecture research', visits: [
    { id: 'b1', time: '20:05', title: 'Attention Is All You Need', domain: 'arxiv.org', url: 'https://arxiv.org/abs/1706.03762', type: 'typed', visitCount: 15, typedCount: 6 },
    { id: 'b2', time: '19:52', title: 'transformer architecture explained — Google Search', domain: 'google.com', url: 'https://google.com/search?q=transformer+architecture+explained', type: 'typed' },
    { id: 'b3', time: '19:45', title: 'The Illustrated Transformer — Jay Alammar', domain: 'jalammar.github.io', url: 'https://jalammar.github.io/illustrated-transformer/', type: 'link', visitCount: 22, typedCount: 4 },
    { id: 'b4', time: '19:30', title: 'Illustrated Guide to Transformers — Step by Step', domain: 'youtube.com', url: 'https://youtube.com/watch?v=4Bdc55j80l8', type: 'link', visitCount: 3 },
    { id: 'b5', time: '19:12', title: 'Visual Explanation of Multi-Head Attention', domain: 'medium.com', url: 'https://medium.com/@neural3d/visual-multi-head-attention', type: 'link', visitCount: 1 }]
  },
  { timeRange: '17:30 — 18:45', label: 'SQLite and Rust bindings', visits: [
    { id: 'c1', time: '18:45', title: 'sqlx — Compile-time checked SQL queries for Rust', domain: 'docs.rs', url: 'https://docs.rs/sqlx/latest/sqlx/', type: 'typed', visitCount: 31, typedCount: 10 },
    { id: 'c2', time: '18:30', title: 'rusqlite — Ergonomic bindings to SQLite for Rust', domain: 'docs.rs', url: 'https://docs.rs/rusqlite/latest/rusqlite/', type: 'typed', visitCount: 19, typedCount: 7 },
    { id: 'c3', time: '18:15', title: 'SQLite FTS5 Full-Text Search Module', domain: 'sqlite.org', url: 'https://sqlite.org/fts5.html', type: 'link', visitCount: 8 },
    { id: 'c4', time: '18:00', title: 'How SQLite Is Tested', domain: 'sqlite.org', url: 'https://sqlite.org/testing.html', type: 'link', visitCount: 4 },
    { id: 'c5', time: '17:45', title: 'Write-Ahead Logging in SQLite', domain: 'sqlite.org', url: 'https://sqlite.org/wal.html', type: 'link', visitCount: 6 },
    { id: 'c6', time: '17:30', title: 'sqlx vs rusqlite benchmark — Reddit', domain: 'reddit.com', url: 'https://reddit.com/r/rust/comments/sqlx_vs_rusqlite', type: 'link', visitCount: 2 }]
  },
  { timeRange: '12:15 — 13:00', label: 'Lunch reading', visits: [
    { id: 'd1', time: '12:55', title: 'Show HN: I built a local-first browser history archive', domain: 'news.ycombinator.com', url: 'https://news.ycombinator.com/item?id=41234567', type: 'typed', visitCount: 1 },
    { id: 'd2', time: '12:42', title: 'Craig Mod — Walking, Making, and the Future of Books', domain: 'craigmod.com', url: 'https://craigmod.com/essays/walking/', type: 'link', visitCount: 3 },
    { id: 'd3', time: '12:30', title: 'Are.na — Knowledge as a commons', domain: 'are.na', url: 'https://are.na/editorial/knowledge-as-commons', type: 'link', visitCount: 2 },
    { id: 'd4', time: '12:15', title: 'Robin Sloan — A Year of Notation', domain: 'robinsloan.com', url: 'https://robinsloan.com/notes/year-of-notation/', type: 'typed', visitCount: 1 }]
  }]

},
{
  date: '2026-05-15', label: 'Thursday, May 15', year: 2026, totalVisits: 15,
  sessions: [
  { timeRange: '21:00 — 22:30', label: 'Tauri 2 plugin development', visits: [
    { id: 'e1', time: '22:28', title: 'Tauri 2 Guides — Plugin Development', domain: 'v2.tauri.app', url: 'https://v2.tauri.app/develop/plugins/', type: 'link', visitCount: 18 },
    { id: 'e2', time: '22:10', title: 'tauri-apps/plugins-workspace · GitHub', domain: 'github.com', url: 'https://github.com/tauri-apps/plugins-workspace', type: 'link', visitCount: 7 },
    { id: 'e3', time: '21:45', title: 'IPC Communication in Tauri v2', domain: 'v2.tauri.app', url: 'https://v2.tauri.app/concept/inter-process-communication/', type: 'link', visitCount: 12 },
    { id: 'e4', time: '21:20', title: 'serde_json — Serialize/Deserialize JSON in Rust', domain: 'docs.rs', url: 'https://docs.rs/serde_json/', type: 'typed', visitCount: 40 },
    { id: 'e5', time: '21:00', title: 'Tauri v2 migration guide — Medium', domain: 'medium.com', url: 'https://medium.com/@dev/tauri-v2-migration', type: 'link', visitCount: 2 }]
  },
  { timeRange: '15:30 — 17:00', label: 'Music production / synthesis', visits: [
    { id: 'f1', time: '16:55', title: 'Vital — Free spectral warping wavetable synth', domain: 'vital.audio', url: 'https://vital.audio/', type: 'typed', visitCount: 5 },
    { id: 'f2', time: '16:30', title: 'Sound design basics with wavetable synthesis', domain: 'youtube.com', url: 'https://youtube.com/watch?v=synth_basics', type: 'link', visitCount: 1 },
    { id: 'f3', time: '16:10', title: 'r/synthesizers — Weekly patch thread', domain: 'reddit.com', url: 'https://reddit.com/r/synthesizers/weekly_patch', type: 'link', visitCount: 2 },
    { id: 'f4', time: '15:45', title: 'Ableton Live 12 — What\'s New', domain: 'ableton.com', url: 'https://ableton.com/live/whats-new/', type: 'typed', visitCount: 3 },
    { id: 'f5', time: '15:30', title: 'Andrew Huang — Making music with household items', domain: 'youtube.com', url: 'https://youtube.com/watch?v=household_music', type: 'link', visitCount: 1 }]
  },
  { timeRange: '09:30 — 11:00', label: 'Morning development session', visits: [
    { id: 'g1', time: '10:55', title: 'MDN — Web Storage API', domain: 'developer.mozilla.org', url: 'https://developer.mozilla.org/en-US/docs/Web/API/Web_Storage_API', type: 'link', visitCount: 9 },
    { id: 'g2', time: '10:30', title: 'React 19 — What\'s New in Actions and Server Components', domain: 'react.dev', url: 'https://react.dev/blog/2025/04/25/react-19', type: 'link', visitCount: 6 },
    { id: 'g3', time: '10:15', title: 'TypeScript 5.6 Release Notes', domain: 'devblogs.microsoft.com', url: 'https://devblogs.microsoft.com/typescript/typescript-5-6/', type: 'link', visitCount: 3 },
    { id: 'g4', time: '09:45', title: 'Vite — Next Generation Frontend Tooling', domain: 'vitejs.dev', url: 'https://vitejs.dev/', type: 'typed', visitCount: 25 },
    { id: 'g5', time: '09:30', title: 'bun — JavaScript runtime & toolkit', domain: 'bun.sh', url: 'https://bun.sh/', type: 'typed', visitCount: 11 }]
  }]

},
{
  date: '2026-05-14', label: 'Wednesday, May 14', year: 2026, totalVisits: 11,
  sessions: [
  { timeRange: '20:00 — 22:00', label: 'PathKeep development', visits: [
    { id: 'h1', time: '21:50', title: 'chromium/src — history_backend.cc', domain: 'chromium.googlesource.com', url: 'https://chromium.googlesource.com/chromium/src/+/master/components/history/core/browser/history_backend.cc', type: 'link', visitCount: 8 },
    { id: 'h2', time: '21:30', title: 'Chrome History Expiration — 90 Days Default', domain: 'stackoverflow.com', url: 'https://stackoverflow.com/questions/chrome-history-90-days', type: 'link', visitCount: 4 },
    { id: 'h3', time: '21:10', title: 'Safari History Retention — Apple Support', domain: 'support.apple.com', url: 'https://support.apple.com/guide/safari/search-your-web-browsing-history', type: 'link', visitCount: 2 },
    { id: 'h4', time: '20:45', title: 'Firefox Sync — History expiry policy', domain: 'searchfox.org', url: 'https://searchfox.org/firefox-main/source/services/sync/modules/constants.sys.mjs', type: 'link', visitCount: 3 },
    { id: 'h5', time: '20:20', title: 'As We May Think — Vannevar Bush', domain: 'theatlantic.com', url: 'https://theatlantic.com/magazine/archive/1945/07/as-we-may-think/', type: 'typed', visitCount: 6, typedCount: 3 },
    { id: 'h6', time: '20:00', title: 'Andrej Karpathy on LLM Knowledge Bases — X', domain: 'x.com', url: 'https://x.com/karpathy/status/2039805659525644595', type: 'link', visitCount: 2 }]
  },
  { timeRange: '14:00 — 16:00', label: 'Afternoon reading', visits: [
    { id: 'i1', time: '15:50', title: 'iA Writer — The focused writing environment', domain: 'ia.net', url: 'https://ia.net/writer', type: 'typed', visitCount: 4 },
    { id: 'i2', time: '15:20', title: 'Cosmos — Your personal web', domain: 'cosmos.so', url: 'https://cosmos.so/', type: 'typed', visitCount: 2 },
    { id: 'i3', time: '14:45', title: 'Readwise Reader — The read-later app for power readers', domain: 'readwise.io', url: 'https://readwise.io/read', type: 'link', visitCount: 7 },
    { id: 'i4', time: '14:15', title: 'Tot — Tiny text companion for your Mac', domain: 'tot.rocks', url: 'https://tot.rocks/', type: 'link', visitCount: 1 },
    { id: 'i5', time: '14:00', title: 'Reeder 5 — Your news reader', domain: 'reederapp.com', url: 'https://reederapp.com/', type: 'link', visitCount: 3 }]
  }]

}];


function getAllVisits() {
  const all = [];
  BROWSE_DAYS.forEach((day) => {
    day.sessions.forEach((session) => {
      session.visits.forEach((v) => {
        all.push({ ...v, fullDate: day.date, dayLabel: day.label });
      });
    });
  });
  return all;
}

/* ═══════════════════════════════════════════════════════════
   GROUP consecutive same-domain visits into stacks
   ═══════════════════════════════════════════════════════════ */
function groupConsecutiveDomains(visits, threshold) {
  const groups = [];
  let i = 0;
  while (i < visits.length) {
    const cur = visits[i];
    const run = [cur];
    let j = i + 1;
    while (j < visits.length && visits[j].domain === cur.domain) {
      run.push(visits[j]);
      j++;
    }
    if (run.length >= threshold) {
      groups.push({ type: 'stack', domain: cur.domain, visits: run });
    } else {
      run.forEach((v) => groups.push({ type: 'single', visit: v }));
    }
    i = j;
  }
  return groups;
}

/* ═══════════════════════════════════════════════════════════
   COMPONENTS
   ═══════════════════════════════════════════════════════════ */

/* ── Contact Frame (card mode) ── */
function ContactFrame({ entry, index, selected, onClick }) {
  const color = getDomainColor(entry.domain);
  return (
    <div className={`cs-frame ${selected ? 'cs-frame--selected' : ''}`}
    data-entry-id={entry.id}
    onClick={() => onClick(entry)}>
      <div className="cs-frame-image" style={{ background: color }}>
        <span className="cs-frame-abbr">{getDomainAbbr(entry.domain)}</span>
        <span className="cs-frame-number">{String(index + 1).padStart(2, '0')}</span>
        <span className="cs-frame-type">{entry.type || 'link'}</span>
      </div>
      <div className="cs-frame-caption">
        <div className="cs-frame-title">{entry.title}</div>
        <div className="cs-frame-meta">
          <span className="cs-frame-domain">{entry.domain}</span>
          <span className="cs-frame-time">{entry.time}</span>
        </div>
      </div>
    </div>);

}

/* ── Domain Stack (collapsed album) ── */
function DomainStack({ stack, onSelectEntry, dayDate, targetEntry }) {
  const containsTarget = targetEntry && stack.visits.some((v) => v.id === targetEntry);
  const [expanded, setExpanded] = React.useState(!!containsTarget);

  // If a new targetEntry arrives that's in this stack, force-expand
  React.useEffect(() => {
    if (containsTarget) setExpanded(true);
  }, [containsTarget]);

  const color = getDomainColor(stack.domain);
  const count = stack.visits.length;
  const first = stack.visits[0];
  const last = stack.visits[stack.visits.length - 1];

  return (
    <div style={{
      border: '1px solid var(--border-light)',
      borderRadius: 'var(--radius)',
      overflow: 'hidden',
      background: 'var(--bg-card)',
      transition: 'box-shadow 150ms'
    }}>
      {/* Stack header — always visible */}
      <div
        onClick={() => setExpanded((e) => !e)}
        style={{
          display: 'grid',
          gridTemplateColumns: '44px 1fr auto',
          gap: 12,
          padding: '10px 14px',
          cursor: 'pointer',
          alignItems: 'center'
        }}>
        {/* Stacked favicon block */}
        <div style={{ position: 'relative', width: 44, height: 36 }}>
          <div style={{
            position: 'absolute', top: 0, left: 0, width: 36, height: 28,
            borderRadius: 'var(--radius)', background: color, opacity: 0.3
          }}></div>
          <div style={{
            position: 'absolute', top: 4, left: 4, width: 36, height: 28,
            borderRadius: 'var(--radius)', background: color, opacity: 0.6
          }}></div>
          <div style={{
            position: 'absolute', top: 8, left: 8, width: 36, height: 28,
            borderRadius: 'var(--radius)', background: color,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            fontFamily: 'var(--font-mono)', fontSize: 10, fontWeight: 600,
            color: 'rgba(255,255,255,0.8)', letterSpacing: '0.06em'
          }}>{getDomainAbbr(stack.domain)}</div>
        </div>
        <div style={{ minWidth: 0 }}>
          <div style={{
            fontFamily: 'var(--font-mono)', fontSize: 12,
            color: 'var(--ink)', fontWeight: 500
          }}>{stack.domain}</div>
          <div style={{
            fontFamily: 'var(--font-sans)', fontSize: 11.5,
            color: 'var(--ink-faint)', marginTop: 1
          }}>{count} pages · {last.time} — {first.time}</div>
        </div>
        <div style={{
          display: 'flex', alignItems: 'center', gap: 8
        }}>
          <span style={{
            fontFamily: 'var(--font-mono)', fontSize: 16,
            color: 'var(--ink-muted)', fontWeight: 400
          }}>{count}</span>
          <span style={{
            color: 'var(--ink-faint)', fontSize: 12,
            transition: 'transform 150ms',
            transform: expanded ? 'rotate(90deg)' : 'rotate(0)'
          }}>▸</span>
        </div>
      </div>

      {/* Collapsed preview — show titles */}
      {!expanded &&
      <div style={{
        padding: '0 14px 10px 70px',
        display: 'flex', flexDirection: 'column', gap: 2
      }}>
          {stack.visits.slice(0, 4).map((v, i) =>
        <div key={v.id}
        data-entry-id={v.id}
        style={{
          fontFamily: 'var(--font-serif)', fontSize: 12,
          color: 'var(--ink-muted)', lineHeight: 1.3,
          overflow: 'hidden', textOverflow: 'ellipsis',
          whiteSpace: 'nowrap', cursor: 'pointer'
        }}
        onClick={(e) => {e.stopPropagation();onSelectEntry({ ...v, fullDate: dayDate });}}>
              <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--ink-faint)', marginRight: 6 }}>{v.time}</span>
              {v.title}
            </div>
        )}
          {count > 4 &&
        <div style={{
          fontFamily: 'var(--font-mono)', fontSize: 10.5,
          color: 'var(--ink-faint)', marginTop: 2, cursor: 'pointer'
        }} onClick={(e) => {e.stopPropagation();setExpanded(true);}}>
              + {count - 4} more
            </div>
        }
        </div>
      }

      {/* Expanded — full list */}
      {expanded &&
      <div style={{ borderTop: '1px solid var(--border-light)' }}>
          {stack.visits.map((v) =>
        <div key={v.id}
        data-entry-id={v.id}
        onClick={() => onSelectEntry({ ...v, fullDate: dayDate })}
        style={{
          display: 'grid',
          gridTemplateColumns: '28px 1fr auto',
          gap: 10, padding: '8px 14px',
          cursor: 'pointer', alignItems: 'center',
          borderBottom: '1px solid var(--border-light)',
          transition: 'background 120ms'
        }}
        onMouseEnter={(e) => e.currentTarget.style.background = 'var(--bg-hover)'}
        onMouseLeave={(e) => e.currentTarget.style.background = ''}>
              <div style={{
            width: 28, height: 28, borderRadius: 'var(--radius)',
            background: color, display: 'flex', alignItems: 'center',
            justifyContent: 'center', fontFamily: 'var(--font-mono)',
            fontSize: 9, fontWeight: 600, color: 'rgba(255,255,255,0.7)'
          }}>{getDomainAbbr(v.domain)}</div>
              <div style={{ minWidth: 0 }}>
                <div style={{
              fontFamily: 'var(--font-serif)', fontSize: 13, color: 'var(--ink)',
              overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap'
            }}>{v.title}</div>
                <div style={{
              fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--ink-faint)',
              overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', marginTop: 1
            }}>{v.url}</div>
              </div>
              <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10.5, color: 'var(--ink-faint)' }}>{v.time}</span>
            </div>
        )}
        </div>
      }
    </div>);

}

/* ── List Row (list mode) ── */
function ListRow({ entry, onClick }) {
  const color = getDomainColor(entry.domain);
  return (
    <div
      data-entry-id={entry.id}
      onClick={() => onClick(entry)}
      style={{
        display: 'grid',
        gridTemplateColumns: '26px 1fr auto',
        gap: 10, padding: '7px 4px',
        cursor: 'pointer', alignItems: 'center',
        borderBottom: '1px solid var(--border-light)',
        transition: 'background 100ms'
      }}
      onMouseEnter={(e) => e.currentTarget.style.background = 'var(--bg-hover)'}
      onMouseLeave={(e) => e.currentTarget.style.background = ''}>
      <div style={{
        width: 24, height: 24, borderRadius: 6,
        background: color, display: 'flex', alignItems: 'center',
        justifyContent: 'center', fontFamily: 'var(--font-mono)',
        fontSize: 8, fontWeight: 600, color: 'rgba(255,255,255,0.7)'
      }}>{getDomainAbbr(entry.domain)}</div>
      <div style={{ minWidth: 0, display: 'flex', alignItems: 'baseline', gap: 8 }}>
        <span style={{
          fontFamily: 'var(--font-sans)', fontSize: 12.5, color: 'var(--ink)',
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', flex: 1
        }}>{entry.title}</span>
        <span style={{
          fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--ink-faint)',
          flexShrink: 0
        }}>{entry.domain}</span>
      </div>
      <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--ink-faint)', flexShrink: 0 }}>{entry.time}</span>
    </div>);

}

/* ═══════════════════════════════════════════════════════════
   Day Insights Strip (separator)
   ═══════════════════════════════════════════════════════════ */
function DayInsightsStrip({ day }) {
  const domainCounts = {};
  let totalPages = 0,typedCount = 0,linkCount = 0,searchCount = 0;
  day.sessions.forEach((s) => s.visits.forEach((v) => {
    totalPages++;
    domainCounts[v.domain] = (domainCounts[v.domain] || 0) + 1;
    if (v.type === 'typed') typedCount++;else
    if (v.type === 'link') linkCount++;
    if (v.domain === 'google.com') searchCount++;
  }));
  const topDomains = Object.entries(domainCounts).sort((a, b) => b[1] - a[1]).slice(0, 4);
  const maxDC = topDomains[0]?.[1] || 1;

  return (
    <div style={{
      display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 16,
      padding: '12px 0 16px'
    }}>
      <div>
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: 9, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--ink-faint)', marginBottom: 6 }}>Top domains</div>
        {topDomains.map(([d, c]) =>
        <div key={d} style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 3 }}>
            <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--ink-muted)', width: 75, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{d.replace('www.', '')}</span>
            <div style={{ flex: 1, height: 4, background: 'var(--bg-page)', borderRadius: 1, overflow: 'hidden' }}>
              <div style={{ height: '100%', width: `${c / maxDC * 100}%`, background: 'var(--accent)', opacity: 0.55 }}></div>
            </div>
            <span style={{ fontFamily: 'var(--font-mono)', fontSize: 9.5, color: 'var(--ink-faint)', width: 14, textAlign: 'right' }}>{c}</span>
          </div>
        )}
      </div>
      <div>
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: 9, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--ink-faint)', marginBottom: 6 }}>Activity</div>
        {[['Pages', totalPages], ['Typed', typedCount], ['Links', linkCount], ['Searches', searchCount]].map(([l, v]) =>
        <div key={l} style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11.5, color: 'var(--ink-secondary)', padding: '2px 0' }}>
            <span>{l}</span><span style={{ fontFamily: 'var(--font-mono)', fontSize: 10.5 }}>{v}</span>
          </div>
        )}
      </div>
      <div>
        <div style={{ fontFamily: 'var(--font-mono)', fontSize: 9, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--ink-faint)', marginBottom: 6 }}>24-hour activity</div>
        <HourlySparkline day={day} />
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11.5, color: 'var(--ink-secondary)', marginTop: 6 }}>
          <span>{day.sessions.length} sessions</span>
          <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10.5 }}>{Object.keys(domainCounts).length} domains</span>
        </div>
      </div>
    </div>);

}

/* ── 24-hour sparkline ── */
/* NOTE: implement with tailwind + shadcn recharts in production */
function HourlySparkline({ day }) {
  const hc = new Array(24).fill(0);
  day.sessions.forEach((s) => s.visits.forEach((v) => {
    const h = parseInt(v.time.split(':')[0], 10);
    if (!isNaN(h)) hc[h]++;
  }));
  const isToday = day.date === new Date().toISOString().slice(0, 10);
  const curH = isToday ? new Date().getHours() : 23;
  const max = Math.max(...hc, 1);
  const W = 220,H = 36,pL = 10,pR = 10,pY = 4,iW = W - pL - pR;
  const pts = [];
  for (let h = 0; h <= curH; h++) {
    pts.push(`${pL + h / 23 * iW},${pY + (1 - hc[h] / max) * (H - pY * 2)}`);
  }
  const fill = [`${pL},${H}`, ...pts, `${pL + curH / 23 * iW},${H}`].join(' ');
  return (
    <svg width="100%" viewBox={`0 0 ${W} ${H + 12}`} style={{ display: 'block', overflow: 'visible' }}>
      {[0, 6, 12, 18].map((h) => <line key={h} x1={pL + h / 23 * iW} y1={0} x2={pL + h / 23 * iW} y2={H} stroke="var(--border-light)" strokeWidth="0.5" />)}
      <polygon points={fill} fill="var(--accent)" opacity="0.1" />
      <polyline points={pts.join(' ')} fill="none" stroke="var(--accent)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      {hc.slice(0, curH + 1).map((c, h) => c > 0 ? <circle key={h} cx={pL + h / 23 * iW} cy={pY + (1 - c / max) * (H - pY * 2)} r="2" fill="var(--accent)" /> : null)}
      {isToday && curH < 23 && <line x1={pL + curH / 23 * iW} y1={H} x2={pL + iW} y2={H} stroke="var(--border)" strokeWidth="1" strokeDasharray="3 3" />}
      {[0, 6, 12, 18, 23].map((h) => <text key={h} x={pL + h / 23 * iW} y={H + 10} textAnchor="middle" style={{ fontFamily: 'var(--font-mono)', fontSize: 7.5, fill: 'var(--ink-faint)' }}>{h}</text>)}
    </svg>);

}

/* ═══════════════════════════════════════════════════════════
   MAIN VIEW — infinite scroll with day separators
   ═══════════════════════════════════════════════════════════ */
function ContactSheetView({ onSelectEntry, selectedId, targetDate, targetEntry, targetSource, targetQuery, onClearTarget }) {
  const [viewMode, setViewMode] = React.useState('cards'); // 'cards' | 'list'
  const [calOpen, setCalOpen] = React.useState(false);
  const [stuck, setStuck] = React.useState(false);
  const [placeholderDay, setPlaceholderDay] = React.useState(null); // {iso, count} for jumps outside loaded set
  const [currentDate, setCurrentDate] = React.useState(BROWSE_DAYS[0].date);
  const currentDateRef = React.useRef(BROWSE_DAYS[0].date);
  const STACK_THRESHOLD = 3;
  const dayRefs = React.useRef({});
  const bannerRef = React.useRef(null);
  const toolbarRef = React.useRef(null);
  const calRef = React.useRef(null);

  // Density map — generated once across the full 60-year archive
  const density = React.useMemo(() => generateArchiveDensity(), []);
  const loadedDates = React.useMemo(
    () => new Set(BROWSE_DAYS.map((d) => d.date)),
    []
  );

  const targetDay = React.useMemo(
    () => targetDate ? BROWSE_DAYS.find((d) => d.date === targetDate) : null,
    [targetDate]
  );

  // Format a yyyy-mm-dd string as "Saturday, May 17, 2025"
  const prettyDate = React.useMemo(() => {
    if (!targetDate) return '';
    return prettyDay(targetDate);
  }, [targetDate]);

  // Programmatically scroll the .pk-content scroller to a given day,
  // landing it just below the sticky toolbar (so its sticky header
  // tucks under the toolbar without overlap).
  // We use a hand-rolled smooth scroller — native `behavior:'smooth'` is
  // silently ignored in this preview environment.
  const smoothScrollRef = React.useRef(null);
  const smoothScrollTo = React.useCallback((scroller, top, duration = 480) => {
    if (smoothScrollRef.current) cancelAnimationFrame(smoothScrollRef.current);
    const start = scroller.scrollTop;
    const dist = top - start;
    if (Math.abs(dist) < 2) { scroller.scrollTop = top; return; }
    const t0 = performance.now();
    const step = (now) => {
      const t = Math.min(1, (now - t0) / duration);
      const eased = 1 - Math.pow(1 - t, 3);
      scroller.scrollTop = start + dist * eased;
      if (t < 1) smoothScrollRef.current = requestAnimationFrame(step);
      else smoothScrollRef.current = null;
    };
    smoothScrollRef.current = requestAnimationFrame(step);
  }, []);

  const scrollToDay = React.useCallback((date) => {
    const scroller = document.querySelector('.pk-content');
    const node = dayRefs.current[date];
    if (!scroller || !node) return;
    const sRect = scroller.getBoundingClientRect();
    const nRect = node.getBoundingClientRect();
    const toolbarH = toolbarRef.current ? toolbarRef.current.offsetHeight : 44;
    const top = scroller.scrollTop + (nRect.top - sRect.top) - toolbarH - 8;
    smoothScrollTo(scroller, top);
  }, [smoothScrollTo]);

  // Resolve a jump target — if the date is loaded, scroll to it; otherwise
  // drop a placeholder day at the top of the list and scroll there.
  const handleJump = React.useCallback((iso) => {
    currentDateRef.current = iso; // sync ref immediately so prev/next can chain
    setCurrentDate(iso);
    if (loadedDates.has(iso)) {
      setPlaceholderDay(null);
      requestAnimationFrame(() => scrollToDay(iso));
    } else {
      const count = density.byDate.get(iso) || 0;
      setPlaceholderDay({ iso, count });
      requestAnimationFrame(() => {
        const scroller = document.querySelector('.pk-content');
        if (scroller) smoothScrollTo(scroller, 0);
      });
    }
    setCalOpen(false);
  }, [loadedDates, density, scrollToDay]);

  // Prev / Next day — step by one calendar day from the LATEST ref value
  // (so rapid clicks don't stack on a stale closure).
  const handlePrev = React.useCallback(() => {
    const next = addDays(currentDateRef.current, -1);
    if (next < density.bounds.first) return;
    handleJump(next);
  }, [density, handleJump]);

  const handleNext = React.useCallback(() => {
    const next = addDays(currentDateRef.current, 1);
    if (next > density.bounds.last) return;
    handleJump(next);
  }, [density, handleJump]);

  const handleToday = React.useCallback(() => {
    // "Today" jumps to the latest loaded day (or today if it exists)
    handleJump(loadedDates.has(ARCHIVE_TODAY) ? ARCHIVE_TODAY : ARCHIVE_LATEST);
  }, [handleJump, loadedDates]);

  // Scroll to the matching day (or to the banner) when targetDate changes.
  // If targetEntry is also set, scroll-and-pulse the matching entry once it's
  // mounted in the DOM (retries while DomainStacks open).
  React.useEffect(() => {
    if (!targetDate) return;
    const scroller = document.querySelector('.pk-content');
    if (!scroller) return;

    if (loadedDates.has(targetDate)) {
      currentDateRef.current = targetDate;
      setCurrentDate(targetDate);
      setPlaceholderDay(null);
    }

    // If we have a specific entry, ride directly to it — skip the intermediate
    // day-only scroll. The day's stack auto-expands because we pass targetEntry
    // down to DomainStack.
    if (targetEntry) {
      let attempts = 0;
      const tryEntry = () => {
        attempts += 1;
        const node = scroller.querySelector(`[data-entry-id="${targetEntry}"]`);
        if (!node) {
          if (attempts < 20) setTimeout(tryEntry, 80);
          return;
        }
        const sRect = scroller.getBoundingClientRect();
        const nRect = node.getBoundingClientRect();
        const toolbarH = toolbarRef.current ? toolbarRef.current.offsetHeight : 44;
        // Center the entry vertically below the toolbar
        const desired = sRect.height / 2 - nRect.height / 2;
        const top = scroller.scrollTop + (nRect.top - sRect.top) - Math.max(toolbarH + 24, desired);
        smoothScrollTo(scroller, top);
        // Pulse highlight
        node.classList.add('cs-pulse');
        setTimeout(() => node.classList.remove('cs-pulse'), 2600);
      };
      // Wait for stack expansion / banner mount
      requestAnimationFrame(() => requestAnimationFrame(tryEntry));
      return;
    }

    // Day-only scroll (or banner if day isn't loaded)
    const node = dayRefs.current[targetDate] || bannerRef.current;
    if (!node) return;
    const sRect = scroller.getBoundingClientRect();
    const nRect = node.getBoundingClientRect();
    const toolbarH = toolbarRef.current ? toolbarRef.current.offsetHeight : 44;
    const top = scroller.scrollTop + (nRect.top - sRect.top) - toolbarH - 8;
    smoothScrollTo(scroller, top);
  }, [targetDate, targetEntry, loadedDates]);

  // Track stuck state + current top-most day for the toolbar pill
  React.useEffect(() => {
    const scroller = document.querySelector('.pk-content');
    if (!scroller) return;
    const onScroll = () => {
      setStuck(scroller.scrollTop > 2);
      // Find the topmost day header within the scroll viewport
      const toolbarH = toolbarRef.current ? toolbarRef.current.offsetHeight : 44;
      const probeY = scroller.getBoundingClientRect().top + toolbarH + 24;
      let best = null;
      Object.entries(dayRefs.current).forEach(([date, node]) => {
        if (!node) return;
        const r = node.getBoundingClientRect();
        if (r.top <= probeY) {
          if (!best || r.top > best.top) best = { date, top: r.top };
        }
      });
      if (best && best.date !== currentDate && !placeholderDay) {
        currentDateRef.current = best.date;
        setCurrentDate(best.date);
      }
    };
    onScroll();
    scroller.addEventListener('scroll', onScroll, { passive: true });
    return () => scroller.removeEventListener('scroll', onScroll);
  }, [currentDate, placeholderDay]);

  // Close calendar on outside click / Escape
  React.useEffect(() => {
    if (!calOpen) return;
    const onDown = (e) => {
      if (calRef.current && !calRef.current.contains(e.target)) {
        setCalOpen(false);
      }
    };
    const onKey = (e) => { if (e.key === 'Escape') setCalOpen(false); };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [calOpen]);

  // Global keyboard shortcuts on the Browse view
  React.useEffect(() => {
    const onKey = (e) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const t = e.target;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
      if (calOpen) return; // let the calendar own keys while open
      if (e.key === 'g' || e.key === 'G') { e.preventDefault(); setCalOpen((o) => !o); }
      else if (e.key === 'ArrowLeft' || e.key === 'j' || e.key === 'J') { e.preventDefault(); handlePrev(); }
      else if (e.key === 'ArrowRight' || e.key === 'k' || e.key === 'K') { e.preventDefault(); handleNext(); }
      else if (e.key === 't' || e.key === 'T') { e.preventDefault(); handleToday(); }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [calOpen, handlePrev, handleNext, handleToday]);

  return (
    <div>
      {/* Target-date banner — when arriving from On This Day / a year ago / Search */}
      {targetDate &&
      <div ref={bannerRef} className={'cs-target-banner cs-target-banner--' + (targetSource || 'on-this-day')}>
          <div className="cs-target-banner__main">
            <span className="cs-target-banner__kicker">
              {targetSource === 'search'
                ? (targetQuery ? <>From search · <em>"{targetQuery}"</em></> : 'From search')
                : 'From "On this day"'}
            </span>
            <div className="cs-target-banner__row">
              <span className="cs-target-banner__date">{prettyDate}</span>
              <span className="cs-target-banner__status">
                {targetSource === 'search' && targetEntry
                  ? 'Scrolled to the matching record · neighbours visible above and below'
                  : targetDay
                    ? `${targetDay.totalVisits} pages archived`
                    : 'No archive for this exact day — showing the nearest sessions'}
              </span>
            </div>
          </div>
          <button
          type="button"
          className="cs-target-banner__clear"
          onClick={() => onClearTarget && onClearTarget()}
          aria-label="Clear date filter">
            Clear ×
          </button>
        </div>
      }

      {/* Sticky toolbar — Day nav + View toggle */}
      <div
        ref={toolbarRef}
        className={'cs-toolbar' + (stuck ? ' cs-toolbar--stuck' : '')}
        data-comment-anchor="b12c9aa728-div-477-7">
        {/* Left: prev / day-pill / next / today */}
        <div className="cs-toolbar__left" ref={calRef}>
          <DayNavControl
            currentDate={currentDate}
            density={density}
            loadedDates={loadedDates}
            calOpen={calOpen}
            onToggleCal={() => setCalOpen((o) => !o)}
            onPrev={handlePrev}
            onNext={handleNext}
            onToday={handleToday}
            onJump={handleJump} />

          {calOpen &&
            <CalendarPopover
              value={currentDate}
              density={density}
              bounds={density.bounds}
              loadedDates={loadedDates}
              onSelect={handleJump} />
          }
        </div>

        {/* Right: view-mode toggle */}
        <div className="cs-toolbar__right">
          <span className="cs-toolbar__label">View</span>
          <div className="cs-view-toggle" role="tablist">
            {['cards', 'list'].map((m) =>
              <button
                key={m}
                role="tab"
                aria-selected={viewMode === m}
                onClick={() => setViewMode(m)}
                className={'cs-view-toggle__btn' + (viewMode === m ? ' cs-view-toggle__btn--active' : '')}>
                {m === 'cards' ? '⊞ Cards' : '☰ List'}
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Right-edge year rail — full-archive mini-map */}
      <YearRail
        density={density}
        bounds={density.bounds}
        currentDate={currentDate}
        onJump={handleJump} />

      {/* Placeholder day — shown when a jumped-to date isn't loaded */}
      {placeholderDay && <PlaceholderDay iso={placeholderDay.iso} count={placeholderDay.count} />}

      {/* All days — infinite scroll */}
      {BROWSE_DAYS.map((day, di) => {
        let frameCounter = 0;
        // Count frames in previous days for global index
        for (let k = 0; k < di; k++) {
          BROWSE_DAYS[k].sessions.forEach((s) => {frameCounter += s.visits.length;});
        }
        let dayFrameStart = frameCounter;

        return (
          <div key={day.date} ref={(el) => {if (el) dayRefs.current[day.date] = el;}}>
            {/* Day separator / header — only the title row floats */}
            <div className="cs-day-sticky" style={{ marginTop: di > 0 ? 32 : 0 }}>
              <div className="cs-day-sticky__inner" style={{
                borderBottom: targetDate === day.date ? '2px solid var(--accent)' : '2px solid var(--border)'
              }}>
                <div className="cs-day-sticky__title">
                  <span className="cs-day-sticky__label">{day.label}</span>
                  <span className="cs-day-sticky__meta">{day.totalVisits} pages · {day.sessions.length} sessions</span>
                </div>
                <span className="cs-day-sticky__index">Day {BROWSE_DAYS.length - di}</span>
              </div>
            </div>

            {/* Day insights strip — scrolls with content, NOT sticky */}
            <DayInsightsStrip day={day} />

            {/* Sessions */}
            {day.sessions.map((session, si) => {
              const grouped = groupConsecutiveDomains(session.visits, STACK_THRESHOLD);
              let sessionIdx = dayFrameStart;
              // Count all visits in prior sessions of this day
              for (let s = 0; s < si; s++) sessionIdx += day.sessions[s].visits.length;
              let localIdx = 0;

              return (
                <div key={si} style={{ marginBottom: 24 }}>
                  <div className="cs-session-header">
                    <span className="cs-session-time">{session.timeRange}</span>
                    <span className="cs-session-label">{session.label}</span>
                  </div>

                  {viewMode === 'cards' ?
                  <div className="cs-grid">
                      {grouped.map((group, gi) => {
                      if (group.type === 'stack') {
                        const el =
                        <DomainStack
                          key={`stack-${gi}`}
                          stack={group}
                          onSelectEntry={onSelectEntry}
                          dayDate={day.date}
                          targetEntry={targetEntry} />;


                        localIdx += group.visits.length;
                        return el;
                      } else {
                        const idx = sessionIdx + localIdx;
                        localIdx++;
                        return (
                          <ContactFrame
                            key={group.visit.id}
                            entry={{ ...group.visit, fullDate: day.date }}
                            index={idx}
                            selected={selectedId === group.visit.id}
                            onClick={onSelectEntry} />);


                      }
                    })}
                    </div> :

                  <div>
                      {session.visits.map((v) =>
                    <ListRow key={v.id} entry={{ ...v, fullDate: day.date }} onClick={onSelectEntry} />
                    )}
                    </div>
                  }
                </div>);

            })}
          </div>);

      })}

      {/* Archive depth — loading more (older) */}
      <LoadingSkeleton
        direction="down"
        label={`Loading older days · ${(density.total / 1e6).toFixed(1)}M pages across ${density.bounds.totalDays.toLocaleString()} days`} />

      {/* Quiet caption at the very floor */}
      <div style={{
        textAlign: 'center', padding: '20px 0 8px',
        fontFamily: 'var(--font-serif)', fontStyle: 'italic',
        fontSize: 12, color: 'var(--ink-ghost)', letterSpacing: 0
      }}>
        Archive begins {prettyDay(density.bounds.first)}
      </div>
    </div>);

}

Object.assign(window, { ContactSheetView, DayInsightsStrip, HourlySparkline, DomainStack, BROWSE_DAYS, getAllVisits });