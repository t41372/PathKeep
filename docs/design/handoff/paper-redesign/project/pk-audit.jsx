/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Archive Audit View
   Manifest chain, runs, snapshots, exports, integrity
   ═══════════════════════════════════════════════════════════ */

const AUDIT_CHAIN = [
  { id: '#1847', hash: '0a4c…ef82', when: '2h ago', type: 'backup', current: true },
  { id: '#1846', hash: '8b71…d3a9', when: '2h ago', type: 'backup' },
  { id: '#1845', hash: '4e29…91c7', when: '1d ago', type: 'import' },
  { id: '#1844', hash: '7f0a…2e4b', when: '1d ago', type: 'backup' },
  { id: '#1843', hash: '3d92…8a16', when: '2d ago', type: 'backup' }
];

const AUDIT_RUNS = [
  { id: '#1847', type: 'BACKUP', source: 'Chrome / Default', records: '+12,847', status: 'OK', when: 'May 17 · 14:23', duration: '4.2s', hash: '0a4cef82' },
  { id: '#1846', type: 'BACKUP', source: 'Arc / Work', records: '+3,291', status: 'OK', when: 'May 17 · 14:21', duration: '1.8s', hash: '8b71d3a9' },
  { id: '#1845', type: 'IMPORT', source: 'Google Takeout', records: '+89,412', status: 'OK', when: 'May 16 · 22:05', duration: '2m 14s', hash: '4e2991c7' },
  { id: '#1844', type: 'BACKUP', source: 'Firefox / Personal', records: '+1,023', status: 'OK', when: 'May 16 · 14:20', duration: '0.9s', hash: '7f0a2e4b' },
  { id: '#1843', type: 'BACKUP', source: 'Chrome / Default', records: '+8,196', status: 'REVERTED', when: 'May 15 · 20:11', duration: '3.6s', hash: '3d928a16' },
  { id: '#1842', type: 'BACKUP', source: 'Chrome / Default', records: '+11,204', status: 'OK', when: 'May 15 · 08:23', duration: '4.5s', hash: 'c20e5f44' },
  { id: '#1841', type: 'BACKUP', source: 'Arc / Work', records: '+2,847', status: 'OK', when: 'May 15 · 08:20', duration: '1.5s', hash: 'e9b71c08' }
];

const AUDIT_SNAPSHOTS = [
  { name: 'Snapshot before May 16 import', date: 'May 16, 22:04', size: '11.6 GB' },
  { name: 'Weekly snapshot · W19', date: 'May 12, 03:00', size: '11.2 GB' },
  { name: 'Weekly snapshot · W18', date: 'May 5, 03:00', size: '10.8 GB' },
  { name: 'Weekly snapshot · W17', date: 'Apr 28, 03:00', size: '10.4 GB' }
];

function AuditView({ onNavigate }) {
  return (
    <div>
      {/* Manifest chain */}
      <div className="pk-card" style={{marginBottom: 20}}>
        <div className="pk-card-header">
          <span className="pk-card-title">Manifest chain</span>
          <span className="pk-card-badge" style={{cursor:'pointer', color:'var(--accent)'}}>
            Verify integrity →
          </span>
        </div>
        <div className="pk-card-body" style={{paddingBottom: 8}}>
          <div className="chain-viz">
            {AUDIT_CHAIN.map((block, i) => (
              <React.Fragment key={block.id}>
                <div className={`chain-block ${block.current ? 'chain-block--current' : ''}`}>
                  <div className="chain-block__id">{block.id}</div>
                  <div className="chain-block__hash">{block.hash}</div>
                  <div className="chain-block__when">{block.type} · {block.when}</div>
                </div>
                {i < AUDIT_CHAIN.length - 1 && <div className="chain-arrow"></div>}
              </React.Fragment>
            ))}
            <div className="chain-arrow"></div>
            <div style={{
              fontFamily: 'var(--font-mono)',
              fontSize: 10,
              color: 'var(--ink-faint)',
              padding: '8px 12px',
              border: '1px dashed var(--border)',
              borderRadius: 'var(--radius)',
              minWidth: 100,
              textAlign: 'center'
            }}>
              earlier ↺
            </div>
          </div>
          <div className="audit-callout">
            <strong>Chain verified.</strong> All 1,847 manifest entries hash-link correctly. Latest hash <code>0a4cef82bd5e9c1d7a3f4e0b6c8a2d51</code> · checked 2 hours ago.
          </div>
        </div>
      </div>

      <div className="audit-grid">
        <div style={{display:'flex', flexDirection:'column', gap:20}}>

          {/* Recent runs table */}
          <div className="pk-card">
            <div className="pk-card-header">
              <span className="pk-card-title">Recent runs</span>
              <span className="pk-card-badge">Last 7 of 1,847</span>
            </div>
            <div className="pk-card-body" style={{padding: '8px 0 4px'}}>
              <table className="audit-runs-table">
                <thead>
                  <tr>
                    <th style={{paddingLeft: 18}}>Run</th>
                    <th>Type</th>
                    <th>Source</th>
                    <th>Records</th>
                    <th>Status</th>
                    <th>When</th>
                    <th style={{paddingRight: 18}}>Δ</th>
                  </tr>
                </thead>
                <tbody>
                  {AUDIT_RUNS.map(r => (
                    <tr key={r.id}>
                      <td className="mono" style={{paddingLeft: 18}}>{r.id}</td>
                      <td>
                        <span style={{
                          fontFamily:'var(--font-mono)', fontSize: 9.5, fontWeight: 600,
                          color: r.type === 'IMPORT' ? 'var(--accent-text)' : 'var(--info)',
                          border: '1px solid', padding: '1px 5px',
                          letterSpacing: '0.04em',
                        }}>{r.type}</span>
                      </td>
                      <td>{r.source}</td>
                      <td className="mono" style={{color: r.records.startsWith('+') ? 'var(--success)' : 'var(--ink-secondary)'}}>{r.records}</td>
                      <td>
                        <span style={{
                          fontFamily:'var(--font-mono)', fontSize:9.5, fontWeight:600,
                          color: r.status === 'OK' ? 'var(--success)' : 'var(--error)',
                          letterSpacing: '0.04em',
                        }}>{r.status}</span>
                      </td>
                      <td className="mono" style={{color:'var(--ink-faint)'}}>{r.when}</td>
                      <td className="mono" style={{color:'var(--ink-faint)', paddingRight: 18}}>{r.duration}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          {/* Storage breakdown */}
          <div className="pk-card">
            <div className="pk-card-header">
              <span className="pk-card-title">Storage breakdown</span>
              <span className="pk-card-badge">12.4 GB total</span>
            </div>
            <div className="pk-card-body">
              <StorageBar label="Core archive · pages + visits" size="8.2 GB" pct={66} tone="primary" />
              <StorageBar label="Full-text index (FTS5)" size="1.8 GB" pct={14.5} tone="secondary" />
              <StorageBar label="Embeddings (LanceDB)" size="1.6 GB" pct={13} tone="tertiary" />
              <StorageBar label="Snapshots (4 kept)" size="0.8 GB" pct={6.5} tone="muted" />
              <div style={{
                marginTop: 12, paddingTop: 12, borderTop: '1px dashed var(--border-light)',
                fontFamily: 'var(--font-serif)', fontStyle: 'italic',
                fontSize: 12.5, color: 'var(--ink-muted)', lineHeight: 1.5,
              }}>
                Embeddings are optional — clear them anytime to save 1.6 GB. The index can be rebuilt from your archive in about 4 minutes.
              </div>
            </div>
          </div>
        </div>

        <div style={{display:'flex', flexDirection:'column', gap:20}}>

          {/* Export */}
          <div className="pk-card">
            <div className="pk-card-header">
              <span className="pk-card-title">Take it with you</span>
            </div>
            <div className="pk-card-body">
              <p style={{
                margin: '0 0 14px',
                fontFamily:'var(--font-serif)', fontStyle: 'italic',
                fontSize: 13.5, color: 'var(--ink-muted)', lineHeight: 1.5,
              }}>
                Your archive is yours. Export everything any time, in formats that survive PathKeep itself.
              </p>
              <button className="btn-primary" style={{width: '100%', justifyContent:'center', marginBottom: 8}}>
                <PKGlyph icon="download" size={14} strokeWidth={1.7} />
                Export everything as JSON
              </button>
              <button className="btn-secondary" style={{width: '100%', justifyContent:'center', marginBottom: 8}}>
                Export as CSV
              </button>
              <button className="btn-secondary" style={{width: '100%', justifyContent:'center', marginBottom: 8}}>
                Copy SQLite file
              </button>
              <button className="btn-secondary" style={{width: '100%', justifyContent:'center'}}>
                <PKGlyph icon="folder_open" size={14} strokeWidth={1.7} />
                Reveal in Finder
              </button>
            </div>
          </div>

          {/* Snapshots */}
          <div className="pk-card">
            <div className="pk-card-header">
              <span className="pk-card-title">Snapshots</span>
              <span className="pk-card-badge">4 kept</span>
            </div>
            <div className="pk-card-body" style={{padding: 0}}>
              {AUDIT_SNAPSHOTS.map((s, i) => (
                <div key={i} className="audit-snapshot" style={i === 0 ? {paddingTop: 16} : i === AUDIT_SNAPSHOTS.length - 1 ? {paddingBottom: 16} : {}}>
                  <div>
                    <div className="audit-snapshot__main">{s.name}</div>
                    <div className="audit-snapshot__date">{s.date}</div>
                  </div>
                  <div className="audit-snapshot__size">{s.size}</div>
                </div>
              ))}
            </div>
          </div>

          {/* Quiet line */}
          <div style={{
            fontFamily: 'var(--font-serif)',
            fontStyle: 'italic',
            fontSize: 13,
            color: 'var(--ink-faint)',
            textAlign: 'center',
            lineHeight: 1.5,
          }}>
            Local. Plaintext SQLite by default.<br/>0 network requests, ever.
          </div>
        </div>
      </div>
    </div>);
}

function StorageBar({ label, size, pct, tone }) {
  const colorMap = {
    primary: 'var(--accent)',
    secondary: 'color-mix(in srgb, var(--accent) 70%, var(--ink-faint))',
    tertiary: 'color-mix(in srgb, var(--accent) 45%, var(--ink-faint))',
    muted: 'var(--ink-faint)'
  };
  return (
    <div style={{marginBottom: 12}}>
      <div style={{
        display: 'flex', justifyContent: 'space-between',
        marginBottom: 4,
        fontFamily: 'var(--font-sans)', fontSize: 12.5, color: 'var(--ink-secondary)',
      }}>
        <span>{label}</span>
        <span style={{fontFamily:'var(--font-mono)', fontSize: 11, color: 'var(--ink-muted)'}}>{size}</span>
      </div>
      <div style={{
        height: 6, background: 'var(--bg-page)',
        borderRadius: 2, overflow: 'hidden',
      }}>
        <div style={{
          height: '100%', width: `${pct}%`,
          background: colorMap[tone] || colorMap.primary,
        }}></div>
      </div>
    </div>
  );
}

Object.assign(window, { AuditView, AUDIT_CHAIN, AUDIT_RUNS, AUDIT_SNAPSHOTS });
