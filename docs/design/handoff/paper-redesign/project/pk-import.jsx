/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Import Wizard
   ═══════════════════════════════════════════════════════════ */

const IMPORT_METHODS = [
  {
    id: 'takeout',
    icon: 'cloud_upload',
    title: 'Google Takeout',
    desc: 'Import an exported Google archive. Recovers up to ~18 months of history beyond the local 90-day cap.',
    hint: 'Recommended · ZIP or unpacked'
  },
  {
    id: 'browser',
    icon: 'folder_open',
    title: 'Browser direct',
    desc: 'Read directly from a Chrome / Edge / Firefox / Arc / Safari profile database.',
    hint: 'macOS · needs file access'
  },
  {
    id: 'csv',
    icon: 'download',
    title: 'CSV / JSON',
    desc: 'Import from another archive tool or a previous PathKeep export.',
    hint: 'Schema-aware'
  }
];

const STEPS = ['Upload', 'Scan', 'Preview', 'Confirm', 'Import'];

function ImportView() {
  const [activeMethod, setActiveMethod] = React.useState('takeout');
  const [currentStep, setCurrentStep] = React.useState(2); // 0-indexed: at "Preview"

  return (
    <div>
      {/* Method selector */}
      <div style={{
        fontFamily: 'var(--font-serif)',
        fontStyle: 'italic',
        fontSize: 14,
        color: 'var(--ink-muted)',
        marginBottom: 14,
        maxWidth: 580,
        lineHeight: 1.5,
      }}>
        Bring history into the archive. PathKeep stages a safe copy, deduplicates, and never touches your live browser databases.
      </div>
      <div className="import-methods">
        {IMPORT_METHODS.map(m => (
          <div
            key={m.id}
            className={`import-method ${activeMethod === m.id ? 'import-method--active' : ''}`}
            onClick={() => setActiveMethod(m.id)}>
            <div className="import-method__icon">
              <PKGlyph icon={m.icon} size={28} strokeWidth={1.5} />
            </div>
            <div className="import-method__title">{m.title}</div>
            <div className="import-method__desc">{m.desc}</div>
            <div className="import-method__hint">{m.hint}</div>
          </div>
        ))}
      </div>

      {/* Wizard */}
      <div className="import-wizard">
        {/* Stepper */}
        <div className="import-stepper">
          {STEPS.map((label, i) => {
            const state = i < currentStep ? 'done' : i === currentStep ? 'active' : 'idle';
            return (
              <React.Fragment key={label}>
                <div className={`import-step ${state === 'done' ? 'import-step--done' : ''} ${state === 'active' ? 'import-step--active' : ''}`}>
                  <div className="import-step__num">
                    {state === 'done' ? <PKGlyph icon="check" size={12} strokeWidth={2} /> : i + 1}
                  </div>
                  <span className="import-step__label">{label}</span>
                </div>
                {i < STEPS.length - 1 && (
                  <div className={`import-step__line ${i < currentStep ? 'import-step__line--done' : ''}`}></div>
                )}
              </React.Fragment>
            );
          })}
        </div>

        {/* Body */}
        <div className="import-body">
          <h2 className="import-title">Preview what we found</h2>
          <p className="import-subtitle">
            Review before committing. PathKeep won't change your archive until you confirm.
          </p>

          <div className="import-preview-stats">
            <div className="import-stat">
              <div className="import-stat__label">Records found</div>
              <div className="import-stat__value">89,412</div>
            </div>
            <div className="import-stat">
              <div className="import-stat__label">Time range</div>
              <div className="import-stat__value">2019 · 2023</div>
            </div>
            <div className="import-stat">
              <div className="import-stat__label">Duplicates</div>
              <div className="import-stat__value">2,847</div>
            </div>
            <div className="import-stat">
              <div className="import-stat__label">New to archive</div>
              <div className="import-stat__value import-stat__value--accent">+86,565</div>
            </div>
          </div>

          <div className="import-files">
            <div style={{
              fontFamily: 'var(--font-mono)',
              fontSize: 9.5,
              letterSpacing: '0.08em',
              textTransform: 'uppercase',
              color: 'var(--ink-faint)',
              marginBottom: 8,
            }}>
              Detected files
            </div>
            <div className="import-file">
              <span className="import-file__status--ok"><PKGlyph icon="check" size={16} strokeWidth={2} /></span>
              <span className="import-file__name">BrowserHistory.json</span>
              <span className="import-file__detail">89,412 entries · known format · dates 2019-03 → 2023-11</span>
              <span className="import-file__size">4.2 MB</span>
            </div>
            <div className="import-file">
              <span className="import-file__status--warn"><PKGlyph icon="warning" size={16} strokeWidth={1.7} /></span>
              <span className="import-file__name">SearchHistory.json</span>
              <span className="import-file__detail">unknown schema · quarantined for review</span>
              <span className="import-file__size">1.8 MB</span>
            </div>
            <div className="import-file">
              <span className="import-file__status--ok"><PKGlyph icon="check" size={16} strokeWidth={2} /></span>
              <span className="import-file__name">Activity.json</span>
              <span className="import-file__detail">12,847 entries · partial overlap with main file</span>
              <span className="import-file__size">0.6 MB</span>
            </div>
          </div>

          <div style={{
            padding: '12px 14px',
            background: 'color-mix(in srgb, var(--info) 8%, var(--bg-paper))',
            borderLeft: '2px solid var(--info)',
            borderRadius: '0 var(--radius) var(--radius) 0',
            fontFamily: 'var(--font-serif)',
            fontStyle: 'italic',
            fontSize: 13.5,
            color: 'var(--ink-secondary)',
            lineHeight: 1.5,
          }}>
            <strong style={{fontStyle:'normal',color:'var(--info)',fontWeight:500}}>What happens on confirm:</strong>
            {' '}A new manifest entry (Run #1848) appends to your archive. Existing pages aren't touched — duplicates are skipped by URL + visit-time. You can revert the import from the Archive Audit page.
          </div>

          <div className="import-actions">
            <button className="btn-secondary" onClick={() => setCurrentStep(s => Math.max(0, s - 1))}>
              <PKGlyph icon="arrow_back" size={14} strokeWidth={1.7} />
              Back
            </button>
            <div style={{display:'flex', gap:8}}>
              <button className="btn-secondary">Quarantine all & continue</button>
              <button className="btn-primary" onClick={() => setCurrentStep(s => Math.min(4, s + 1))}>
                Confirm import
                <PKGlyph icon="arrow_forward" size={14} strokeWidth={1.7} />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>);
}

Object.assign(window, { ImportView, IMPORT_METHODS });
