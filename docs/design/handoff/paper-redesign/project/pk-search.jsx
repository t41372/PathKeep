/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Search View
   A literary, contact-sheet-aware search experience
   ═══════════════════════════════════════════════════════════ */

const RECENT_SEARCHES = [
  { q: 'tokio scheduler', count: 89, when: 'yesterday', mode: 'keyword' },
  { q: 'tauri plugin development', count: 34, when: '3 days ago', mode: 'keyword' },
  { q: 'gaussian splatting', count: 27, when: 'last week', mode: 'semantic' },
  { q: 'r/synthesizers', count: 12, when: 'last week', mode: 'keyword' },
  { q: 'as we may think bush', count: 4, when: '2 weeks ago', mode: 'semantic' },
];

const SAVED_PROMPTS = [
  { cue: 'Just ask', text: 'What was that paper about transformer architecture I read last spring?', hint: 'Semantic recall · ~12 results' },
  { cue: 'By time', text: 'Everything I read on the weekend of Mar 14–15', hint: 'Date filter' },
  { cue: 'By domain', text: 'All my visits to docs.rs this year', hint: '178 results' },
  { cue: 'By thread', text: 'Pages I revisited 3+ times in the last 90 days', hint: 'Refind candidates' },
];

const MODE_HINTS = {
  keyword: 'Match the exact words. Supports site:, before:, after:, regex:',
  regex: 'JavaScript regex. Case-insensitive by default.',
  semantic: 'Meaning, not just words. Requires local embeddings.',
};

const SAMPLE_SNIPPETS = [
  'A short tour of the scheduler — how work-stealing makes concurrent tasks…',
  'In this post we walk through the design constraints that shaped tokio\'s…',
  'The classic 1945 essay introducing the memex — a device for the personal…',
  'Tauri 2 introduces a redesigned plugin system with first-class IPC handlers…',
  'Step-by-step guide to building a wavetable synthesizer in the browser…',
];

function highlightMatch(text, query) {
  if (!query.trim()) return text;
  try {
    const re = new RegExp(`(${query.trim().split(/\s+/).map(s => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|')})`, 'gi');
    const parts = text.split(re);
    return parts.map((p, i) => re.test(p) ? <mark key={i}>{p}</mark> : <React.Fragment key={i}>{p}</React.Fragment>);
  } catch {
    return text;
  }
}

function SearchView({ onSelectEntry, onNavigate, initialQuery, searchData }) {
  const [query, setQuery] = React.useState(initialQuery || '');
  const [mode, setMode] = React.useState('keyword');
  const [activeFilters, setActiveFilters] = React.useState(['Last 30 days']);
  const inputRef = React.useRef(null);

  React.useEffect(() => {
    inputRef.current?.focus();
  }, []);

  React.useEffect(() => {
    if (initialQuery) setQuery(initialQuery);
  }, [initialQuery]);

  // Compute results
  const results = React.useMemo(() => {
    if (!query.trim()) return [];
    const q = query.toLowerCase();
    return searchData.filter(item =>
      item.title.toLowerCase().includes(q) ||
      item.domain.toLowerCase().includes(q) ||
      (item.url && item.url.toLowerCase().includes(q))
    );
  }, [query, searchData]);

  // Group by date
  const grouped = React.useMemo(() => {
    const map = new Map();
    results.forEach(r => {
      const date = r.fullDate || 'Unknown';
      if (!map.has(date)) map.set(date, []);
      map.get(date).push(r);
    });
    return [...map.entries()].sort((a, b) => b[0].localeCompare(a[0]));
  }, [results]);

  const removeFilter = (f) => setActiveFilters(arr => arr.filter(x => x !== f));

  return (
    <div>
      {/* Hero */}
      <div className="sv-hero">
        <div className="sv-prompt">What would you like to find again?</div>
        <div className="sv-input-wrap">
          <input
            ref={inputRef}
            className="sv-input"
            type="text"
            placeholder="A page, a phrase, a feeling…"
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Escape') setQuery('');
            }}
          />
        </div>

        <div className="sv-modes">
          <span className="sv-modes-label">Mode</span>
          <div className="sv-mode-group">
            {['keyword', 'regex', 'semantic'].map(m => (
              <button
                key={m}
                className={`sv-mode-btn ${mode === m ? 'sv-mode-btn--active' : ''}`}
                onClick={() => setMode(m)}>
                {m}
              </button>
            ))}
          </div>
          <span className="sv-mode-hint">{MODE_HINTS[mode]}</span>
        </div>

        <div className="sv-filters">
          <span className="sv-modes-label" style={{marginRight: 4}}>Filters</span>
          {activeFilters.map(f => (
            <span key={f} className="sv-chip sv-chip--active">
              {f}
              <span className="sv-chip__remove" onClick={() => removeFilter(f)}>✕</span>
            </span>
          ))}
          <span className="sv-chip">+ Date</span>
          <span className="sv-chip">+ Source</span>
          <span className="sv-chip">+ Domain</span>
          <span className="sv-chip">+ Visit count</span>
        </div>
      </div>

      {/* Empty state — saved prompts, recents */}
      {!query.trim() ? (
        <div className="sv-empty">
          <h3 className="sv-section-title">Try asking</h3>
          <div className="sv-suggestions">
            {SAVED_PROMPTS.map((p, i) => (
              <div key={i} className="sv-suggestion" onClick={() => setQuery(p.text)}>
                <div className="sv-suggestion__cue">{p.cue}</div>
                <div className="sv-suggestion__text">{p.text}</div>
                <div className="sv-suggestion__hint">{p.hint}</div>
              </div>
            ))}
          </div>

          <h3 className="sv-section-title">Recent</h3>
          <div className="sv-recent">
            {RECENT_SEARCHES.map((r, i) => (
              <div key={i} className="sv-recent-item" onClick={() => { setQuery(r.q); setMode(r.mode); }}>
                <span>{r.q}</span>
                <span className="sv-recent-item__meta">{r.mode} · {r.count} results · {r.when}</span>
              </div>
            ))}
          </div>

          <div style={{
            marginTop: 40, padding: '20px 0',
            borderTop: '1px solid var(--border-light)',
            fontFamily: 'var(--font-serif)',
            fontStyle: 'italic',
            fontSize: 13,
            color: 'var(--ink-faint)',
            textAlign: 'center',
            letterSpacing: 0,
          }}>
            The archive holds 2,847,391 pages across 4 years and 7 months.<br/>
            Search is local. Nothing leaves your machine.
          </div>
        </div>
      ) : results.length === 0 ? (
        <div className="sv-results">
          <div className="pk-empty">
            <div className="pk-empty-text">Nothing here yet. Memory is patient.</div>
            <div className="pk-empty-attr">— try a broader phrase, or switch to semantic mode</div>
          </div>
        </div>
      ) : (
        <div className="sv-results">
          <div className="sv-results-header">
            <div className="sv-results-count">
              <strong>{results.length}</strong> {results.length === 1 ? 'page' : 'pages'} found
            </div>
            <div className="sv-results-range">
              {grouped[grouped.length - 1][0]} — {grouped[0][0]} · {mode}
            </div>
          </div>

          {grouped.map(([date, items]) => (
            <div key={date} className="sv-day-group">
              <div className="sv-day-header">
                <span className="sv-day-title">{formatDay(date)}</span>
                <span className="sv-day-count">{items.length} {items.length === 1 ? 'page' : 'pages'}</span>
              </div>

              {items.map((item, i) => (
                <div key={item.id || i} className="sv-result" onClick={() => onSelectEntry(item)}>
                  <div className="sv-result__icon" style={{ background: getDomainColor(item.domain) }}>
                    {getDomainAbbr(item.domain)}
                  </div>
                  <div className="sv-result__main">
                    <div className="sv-result__title">{highlightMatch(item.title, query)}</div>
                    <div className="sv-result__url">{item.domain} · {item.url}</div>
                    {mode === 'semantic' && (
                      <div className="sv-result__snippet">
                        “…{SAMPLE_SNIPPETS[i % SAMPLE_SNIPPETS.length]}…”
                      </div>
                    )}
                    <button
                      type="button"
                      className="sv-result__seein"
                      onClick={(e) => {
                        e.stopPropagation();
                        onNavigate && onNavigate('browse', {
                          date: item.fullDate,
                          entryId: item.id,
                          source: 'search',
                          query
                        });
                      }}
                      title="Open this entry in Browse and see what surrounds it">
                      <svg width="11" height="11" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                        <rect x="2.5" y="3.5" width="11" height="10" rx="1" />
                        <path d="M2.5 6.5h11" />
                        <path d="M5.5 2v3" />
                        <path d="M10.5 2v3" />
                      </svg>
                      See in context
                      <span aria-hidden="true">→</span>
                    </button>
                  </div>
                  <div className="sv-result__meta">
                    <span>{item.time}</span>
                    <span className="sv-result__type">{item.type || 'link'}</span>
                  </div>
                </div>
              ))}
            </div>
          ))}
        </div>
      )}
    </div>);
}

function formatDay(dateStr) {
  try {
    const [y, m, d] = dateStr.split('-').map(Number);
    const date = new Date(y, m - 1, d);
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);
    if (date.getTime() === today.getTime()) return 'Today';
    if (date.getTime() === yesterday.getTime()) return 'Yesterday';
    return date.toLocaleDateString('en-US', { weekday: 'long', month: 'long', day: 'numeric' });
  } catch {
    return dateStr;
  }
}

Object.assign(window, { SearchView, RECENT_SEARCHES, SAVED_PROMPTS });
