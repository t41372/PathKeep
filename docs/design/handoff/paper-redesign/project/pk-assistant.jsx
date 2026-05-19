/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Assistant View
   Chat with grounded evidence from your archive
   ═══════════════════════════════════════════════════════════ */

const SAMPLE_PROMPTS = [
  'What was that paper about transformers I read last spring?',
  'When did I first start reading about Tauri?',
  'Show me everything I read about SQLite in the last month.',
  'What rabbit hole was I down on the weekend of March 14?',
  'Pages I keep coming back to but never finished.',
  'Was I researching anything about gaussian splatting last year?'
];

const SAMPLE_CONVERSATION = [
  {
    role: 'user',
    content: 'When did I first start reading about Tauri? What did I read first?'
  },
  {
    role: 'ai',
    content: (
      <>
        <p>You first opened Tauri's <strong>GitHub repository</strong> on <strong>April 5, 2025</strong> — about 13 months ago.</p>
        <p>In the two weeks that followed, you visited:</p>
        <ul>
          <li>Tauri's "Getting Started" and "Architecture" docs (14 visits total)</li>
          <li>Several Tauri-vs-Electron comparison posts</li>
          <li>Rust's WebView2 binding threads on GitHub</li>
        </ul>
        <p>The pace picked up sharply around <strong>March 2026</strong>, when you started PathKeep itself. Tauri docs are now in your top-5 most-revisited domains.</p>
      </>
    ),
    evidence: [
      { date: '2025-04-05', title: 'tauri-apps/tauri: Build desktop apps with web technology', domain: 'github.com', url: 'https://github.com/tauri-apps/tauri' },
      { date: '2025-04-06', title: 'Getting Started | Tauri', domain: 'v2.tauri.app', url: 'https://v2.tauri.app/start/' },
      { date: '2025-04-08', title: 'Tauri vs Electron: Real-world Performance Comparison', domain: 'blog.logrocket.com', url: 'https://blog.logrocket.com/tauri-vs-electron/' },
      { date: '2025-04-12', title: 'Architecture | Tauri v2', domain: 'v2.tauri.app', url: 'https://v2.tauri.app/concept/architecture/' }
    ]
  }
];

function AssistantView({ onSelectEntry }) {
  const [messages, setMessages] = React.useState(SAMPLE_CONVERSATION);
  const [input, setInput] = React.useState('');
  const scrollRef = React.useRef(null);

  React.useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages.length]);

  const handleSubmit = (e) => {
    e?.preventDefault();
    const q = input.trim();
    if (!q) return;
    setMessages(prev => [
      ...prev,
      { role: 'user', content: q },
      {
        role: 'ai',
        content: (
          <p style={{fontStyle:'italic', color:'var(--ink-faint)'}}>
            … searching your archive. (Stub in the prototype — a real local LLM would answer here, grounded in pages you visited.)
          </p>
        ),
        evidence: []
      }
    ]);
    setInput('');
  };

  const handleSuggestion = (q) => {
    setInput(q);
  };

  const isEmpty = messages.length === 0;

  return (
    <div className="assist-wrap">
      <div className="assist-messages" ref={scrollRef}>
        {isEmpty ? (
          <div>
            <div className="assist-greeting">What would you like to remember?</div>
            <div className="assist-greeting-sub">
              I can read your archive and tell you what's in it.<br/>
              Try one of these — or write your own.
            </div>
            <div className="assist-empty-prompts">
              {SAMPLE_PROMPTS.map((p, i) => (
                <div key={i} className="assist-empty-prompt" onClick={() => handleSuggestion(p)}>
                  {p}
                </div>
              ))}
            </div>
          </div>
        ) : (
          messages.map((msg, i) => (
            <div key={i} className={`assist-msg assist-msg--${msg.role}`}>
              {msg.role === 'ai' && (
                <div className="assist-msg__byline">
                  <PKGlyph icon="smart_toy" size={11} strokeWidth={1.6} /> Local · llama 3.2
                </div>
              )}
              <div className="assist-msg__bubble">
                {typeof msg.content === 'string' ? <p style={{margin:0}}>{msg.content}</p> : msg.content}
                {msg.evidence && msg.evidence.length > 0 && (
                  <div className="assist-evidence">
                    <div className="assist-evidence__label">Evidence · {msg.evidence.length} records</div>
                    {msg.evidence.map((e, j) => (
                      <div
                        key={j}
                        className="assist-evidence__item"
                        onClick={() => onSelectEntry({
                          id: `evidence-${j}`,
                          title: e.title,
                          domain: e.domain,
                          url: e.url,
                          fullDate: e.date,
                          time: '14:23',
                          type: 'link',
                          visitCount: 8
                        })}>
                        <span className="assist-evidence__date">{e.date}</span>
                        <span className="assist-evidence__title">{e.title} <span style={{color:'var(--ink-faint)', fontFamily:'var(--font-mono)', fontSize:10.5}}>· {e.domain}</span></span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          ))
        )}
      </div>

      <form className="assist-input-wrap" onSubmit={handleSubmit}>
        <div className="assist-input-row">
          <textarea
            className="assist-input"
            placeholder="Ask about your archive…"
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); handleSubmit(); }
            }}
            rows={1}
          />
          <button type="submit" className="assist-input-send" title="Send (↵)">
            <PKGlyph icon="arrow_forward" size={18} strokeWidth={1.8} />
          </button>
        </div>
        <div className="assist-meta">
          <span>Powered by local LLM · Ollama / llama3.2 · 0 network requests</span>
          <span>↵ send · ⇧↵ newline</span>
        </div>
      </form>
    </div>);
}

Object.assign(window, { AssistantView, SAMPLE_PROMPTS, SAMPLE_CONVERSATION });
