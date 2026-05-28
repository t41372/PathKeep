/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Shared Components
   Sidebar, Nav Icons, Detail Panel (enhanced), Search Palette,
   Heatmap, Status Bar
   ═══════════════════════════════════════════════════════════ */

/* ── Glyphs — paths lifted from src/components/ui.tsx ── */
const GLYPH_PATHS = {
  bar_chart: () => (<>
    <path d="M4.5 19.5h15" />
    <path d="M7 18v-5" />
    <path d="M12 18V7" />
    <path d="M17 18v-9" />
  </>),
  auto_stories: () => (<>
    <path d="M6.5 5.5h4.5A3 3 0 0 1 14 8.5v10H9.5A3 3 0 0 0 6.5 21z" />
    <path d="M17.5 5.5H13A3 3 0 0 0 10 8.5v10h4.5A3 3 0 0 1 17.5 21z" />
    <path d="M10 9.5h4" />
  </>),
  search: () => (<>
    <circle cx="10.5" cy="10.5" r="4.5" />
    <path d="m14 14 5 5" />
  </>),
  memory: () => (<>
    <rect height="8" rx="1.5" width="10" x="7" y="8" />
    <path d="M9.5 8V6" /><path d="M12 8V6" /><path d="M14.5 8V6" />
    <path d="M9.5 18v-2" /><path d="M12 18v-2" /><path d="M14.5 18v-2" />
    <path d="M7 10H5" /><path d="M7 14H5" />
    <path d="M19 10h-2" /><path d="M19 14h-2" />
  </>),
  smart_toy: () => (<>
    <rect height="8" rx="2" width="10" x="7" y="8" />
    <path d="M12 8V5.5" />
    <path d="m10 17 1.2 1.5" />
    <path d="m14 17-1.2 1.5" />
    <path d="M8 11H6.5" />
    <path d="M17.5 11H16" />
    <circle cx="10" cy="11.5" fill="currentColor" stroke="none" r="0.8" />
    <circle cx="14" cy="11.5" fill="currentColor" stroke="none" r="0.8" />
    <path d="M10 14h4" />
  </>),
  download: () => (<>
    <path d="M12 4v9.5" />
    <path d="m8 13 4 4 4-4" />
    <path d="M4 20h16" />
  </>),
  history: () => (<>
    <path d="M3.5 5.5V10H8" />
    <path d="M4.3 10A8 8 0 1 0 8 5.3" />
    <path d="M12 8v4.2l2.8 1.8" />
  </>),
  sync: () => (<>
    <path d="M20 7h-6a5 5 0 0 0-5 5v1" />
    <path d="m17 4 3 3-3 3" />
    <path d="M4 17h6a5 5 0 0 0 5-5v-1" />
    <path d="m7 20-3-3 3-3" />
  </>),
  settings: () => (<>
    <path d="M12.2 2h-.4a2 2 0 0 0-2 2v.2a2 2 0 0 1-1 1.7l-.4.2a2 2 0 0 1-2 0l-.2-.1a2 2 0 0 0-2.7.7l-.2.4a2 2 0 0 0 .7 2.7l.2.1a2 2 0 0 1 1 1.7v.8a2 2 0 0 1-1 1.7l-.2.1a2 2 0 0 0-.7 2.7l.2.4a2 2 0 0 0 2.7.7l.2-.1a2 2 0 0 1 2 0l.4.2a2 2 0 0 1 1 1.7v.2a2 2 0 0 0 2 2h.4a2 2 0 0 0 2-2v-.2a2 2 0 0 1 1-1.7l.4-.2a2 2 0 0 1 2 0l.2.1a2 2 0 0 0 2.7-.7l.2-.4a2 2 0 0 0-.7-2.7l-.2-.1a2 2 0 0 1-1-1.7v-.8a2 2 0 0 1 1-1.7l.2-.1a2 2 0 0 0 .7-2.7l-.2-.4a2 2 0 0 0-2.7-.7l-.2.1a2 2 0 0 1-2 0l-.4-.2a2 2 0 0 1-1-1.7V4a2 2 0 0 0-2-2z" />
    <circle cx="12" cy="12" r="3" />
  </>),
  preview: () => (<>
    <path d="M2.5 12s3.5-5.5 9.5-5.5S21.5 12 21.5 12s-3.5 5.5-9.5 5.5S2.5 12 2.5 12Z" />
    <circle cx="12" cy="12" r="2.5" />
  </>),
  shield: () => (<>
    <path d="M12 3.5 19 6v5.5c0 4.2-2.7 8-7 9.7-4.3-1.7-7-5.5-7-9.7V6z" />
  </>),
  database: () => (<>
    <ellipse cx="12" cy="6" rx="7" ry="3" />
    <path d="M5 6v6c0 2 3 3 7 3s7-1 7-3V6" />
    <path d="M5 12v6c0 2 3 3 7 3s7-1 7-3v-6" />
  </>),
  cloud_upload: () => (<>
    <path d="M7 18a4 4 0 1 1 .8-7.9A5.2 5.2 0 0 1 18 11a3.5 3.5 0 1 1 0 7H7z" />
    <path d="M12 15V9.5" />
    <path d="m9.5 11.8 2.5-2.5 2.5 2.5" />
  </>),
  folder_open: () => (<>
    <path d="M3.5 9.5a2 2 0 0 1 2-2H10l2 2h6.5a2 2 0 0 1 2 2l-1 6.5a2 2 0 0 1-2 1.5H6a2 2 0 0 1-2-1.5z" />
    <path d="M3.5 9.5V7A2 2 0 0 1 5.5 5H10l2 2h4" />
  </>),
  warning: () => (<>
    <path d="M12 4.5 20 19H4z" />
    <path d="M12 9v4.5" />
    <circle cx="12" cy="16.5" fill="currentColor" stroke="none" r="0.8" />
  </>),
  check: () => (<>
    <path d="m5 12.5 4.2 4.2L19 7.5" />
  </>),
  content_copy: () => (<>
    <rect height="11" rx="1.5" width="10" x="9" y="7" />
    <path d="M15 5H6a1 1 0 0 0-1 1v9" />
  </>),
  arrow_forward: () => (<>
    <path d="M5 12h14M13 6l6 6-6 6" />
  </>),
  arrow_back: () => (<>
    <path d="M19 12H5M11 18l-6-6 6-6" />
  </>),
  public: () => (<>
    <circle cx="12" cy="12" r="8" />
    <path d="M4 12h16" />
    <path d="M12 4c2.4 2.1 3.6 4.8 3.6 8s-1.2 5.9-3.6 8c-2.4-2.1-3.6-4.8-3.6-8S9.6 6.1 12 4Z" />
  </>),
  // PathKeep additions
  link: () => (<>
    <path d="M10.5 13.5l3-3M13.5 8.5l1.2-1.2a3 3 0 014.2 4.2L17.5 13M10.5 11l-1.2 1.2a3 3 0 11-4.2-4.2L6.5 6.8" />
  </>),
  branch: () => (<>
    <circle cx="7" cy="5" r="2" />
    <circle cx="7" cy="19" r="2" />
    <circle cx="16" cy="12" r="2" />
    <path d="M7 7v10" />
    <path d="M7 11c0 2 1 3 3 3h4" />
  </>),
  bookmark: () => (<>
    <path d="M7 4h10v17l-5-4-5 4z" />
  </>),
  // ── glyphs from ui.tsx not yet mapped ──
  build: () => (<>
    <path d="M14.7 6.3a3.7 3.7 0 0 0 5 5L10.2 20.8a2 2 0 0 1-2.8 0L5.2 18.6a2 2 0 0 1 0-2.8z" />
  </>),
  delete_sweep: () => (<>
    <path d="M5 7h14" />
    <path d="M9.5 7V5.5A1.5 1.5 0 0 1 11 4h2a1.5 1.5 0 0 1 1.5 1.5V7" />
    <path d="m8 7 1 12h6l1-12" />
    <path d="M11 10.5v5" />
    <path d="M13 10.5v5" />
  </>),
  language: () => (<>
    <circle cx="12" cy="12" r="8" />
    <path d="M4 12h16" />
    <path d="M12 4c2.4 2.1 3.6 4.8 3.6 8s-1.2 5.9-3.6 8c-2.4-2.1-3.6-4.8-3.6-8S9.6 6.1 12 4Z" />
  </>),
  notifications: () => (<>
    <path d="M18 10.5A6 6 0 0 0 6 10.5c0 4-1.8 5.2-2.5 6h17c-.7-.8-2.5-2-2.5-6Z" />
    <path d="M9.8 19a2.3 2.3 0 0 0 4.4 0" />
  </>),
  system_update: () => (<>
    <path d="M12 5v9" />
    <path d="m8.5 10.5 3.5 3.5 3.5-3.5" />
    <path d="M5 18h14v2H5z" />
  </>),
};

/* PKGlyph — renderer matching codebase shape (1.6 stroke, 24×24 vb) */
function PKGlyph({ icon, size = 18, strokeWidth = 1.8 }) {
  const render = GLYPH_PATHS[icon];
  if (!render) return null;
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ width: size, height: size, display: 'block', flexShrink: 0 }}
      aria-hidden="true">
      {render()}
    </svg>);
}

/* Back-compat shim — replaces old NavIcons used elsewhere */
const NavIcons = new Proxy({}, {
  get(_, key) {
    // map old keys to new glyph names
    const remap = {
      home: 'bar_chart',
      browse: 'auto_stories',
      search: 'search',
      insights: 'memory',
      assistant: 'smart_toy',
      download: 'download',
      archive: 'history',
      jobs: 'database',
      integrations: 'cloud_upload',
      schedule: 'sync',
      security: 'shield',
      maintenance: 'build',
      settings: 'settings',
      link: 'link',
      globe: 'public',
      branch: 'branch',
      external: 'arrow_forward',
      copy: 'content_copy',
      notifications: 'notifications',
      system_update: 'system_update',
      delete_sweep: 'delete_sweep',
      language: 'language',
    };
    const name = remap[key] || key;
    return () => <PKGlyph icon={name} size={20} />;
  }
});

/* ── PathKeep Brand Mark — inline SVG from src/assets/pathkeep-mark.svg ── */
function PKBrandMark({ size = 30 }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 512 512"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      style={{ display: 'block', flexShrink: 0 }}
      aria-label="PathKeep"
      role="img">
      <rect x="86" y="86" width="340" height="340" stroke="#FF7B33" strokeWidth="24" />
      <rect x="166" y="178" width="54" height="54" fill="#FF7B33" />
      <rect x="166" y="280" width="54" height="54" fill="#FF7B33" />
      <path d="M193 232V280" stroke="#FF7B33" strokeWidth="18" />
      <path d="M262 205H350M262 256H350M262 307H322" stroke="#8B8B8B" strokeWidth="18" strokeLinecap="square" />
    </svg>);
}

/* ── Domain color mapping ── */
const DOMAIN_COLORS = {
  'github.com': '#24292e',
  'google.com': '#4285F4',
  'stackoverflow.com': '#E87922',
  'arxiv.org': '#A8322D',
  'youtube.com': '#CC0000',
  'docs.rs': '#7B5B3A',
  'reddit.com': '#CC4500',
  'medium.com': '#1A8967',
  'wikipedia.org': '#555',
  'news.ycombinator.com': '#E86517',
  'twitter.com': '#1A8CD8',
  'x.com': '#1A1A1A',
  'v2.tauri.app': '#24C8D8',
  'crates.io': '#A8744A',
  'claude.ai': '#CC785C',
  'developer.mozilla.org': '#1A1A2E'
};

function getDomainColor(domain) {
  if (!domain) return '#8a7f70';
  for (const [key, color] of Object.entries(DOMAIN_COLORS)) {
    if (domain.includes(key)) return color;
  }
  let hash = 0;
  for (let i = 0; i < domain.length; i++) hash = domain.charCodeAt(i) + ((hash << 5) - hash);
  const h = Math.abs(hash) % 360;
  return `oklch(0.45 0.06 ${h})`;
}

function getDomainAbbr(domain) {
  if (!domain) return '??';
  const parts = domain.replace('www.', '').split('.');
  const name = parts[0];
  if (name.length <= 2) return name.toUpperCase();
  return name.substring(0, 2).toUpperCase();
}

/* ── Sidebar — section structure mirrors router.tsx sidebarSections ── */
function PKSidebar({ activeView, onNavigate, darkMode, onToggleTheme, collapsed, onToggleCollapse }) {
  // Mirrors router.tsx: CORE / OPERATIONS / SYSTEM
  // Views without a prototype screen fall back to nearest sibling on click
  const navSections = [
    {
      id: 'core',
      label: 'CORE',
      items: [
        { id: 'home',      label: 'Dashboard',    icon: 'home' },
        { id: 'browse',    label: 'Explorer',     icon: 'browse' },
        { id: 'insights',  label: 'Intelligence', icon: 'insights' },
        { id: 'assistant', label: 'AI Assistant', icon: 'assistant', badge: 'v0.3' },
      ]
    },
    {
      id: 'operations',
      label: 'OPERATIONS',
      items: [
        { id: 'import',       label: 'Import',       icon: 'download' },
        { id: 'archive',      label: 'Audit Ledger', icon: 'archive' },
        { id: 'jobs',         label: 'Jobs',         icon: 'jobs' },
        { id: 'integrations', label: 'Integrations', icon: 'integrations' },
      ]
    },
    {
      id: 'system',
      label: 'SYSTEM',
      items: [
        { id: 'schedule',    label: 'Schedule',    icon: 'schedule' },
        { id: 'security',    label: 'Security',    icon: 'security' },
        { id: 'settings',    label: 'Settings',    icon: 'settings' },
        { id: 'maintenance', label: 'Maintenance', icon: 'maintenance' },
      ]
    }
  ];

  // Views the prototype actually has implemented
  const implementedViews = new Set(['home','browse','search','insights','assistant','import','archive','schedule','settings']);

  return (
    <aside className={`pk-sidebar${collapsed ? ' pk-sidebar--collapsed' : ''}`}>
      <div className="pk-sidebar-header">
        <div className="pk-logo-mark"><PKBrandMark /></div>
        <div className="pk-logo-text">
          <span className="pk-logo-name">PathKeep</span>
          <span className="pk-logo-version">v0.3.0</span>
        </div>
        <button
          className="pk-collapse-btn"
          onClick={onToggleCollapse}
          title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
          aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}>
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            {collapsed ? <path d="M6 3l5 5-5 5" /> : <path d="M10 3l-5 5 5 5" />}
          </svg>
        </button>
      </div>

      <nav className="pk-nav">
        {navSections.map((section, si) => (
          <div key={section.id} className="pk-nav-section">
            {!collapsed && (
              <div className="pk-nav-section-label">{section.label}</div>
            )}
            {section.items.map(item => {
              const Icon = NavIcons[item.icon];
              const hasView = implementedViews.has(item.id);
              return (
                <button
                  key={item.id}
                  className={`pk-nav-item ${activeView === item.id ? 'pk-nav-item--active' : ''} ${!hasView ? 'pk-nav-item--stub' : ''}`}
                  onClick={() => hasView ? onNavigate(item.id) : onNavigate('settings')}
                  title={!hasView ? `${item.label} (not yet in prototype)` : item.label}>
                  <span className="pk-nav-icon">{Icon && <Icon />}</span>
                  <span className="pk-nav-item__label">{item.label}</span>
                  {item.badge && !collapsed && <span className="pk-nav-badge">{item.badge}</span>}
                </button>
              );
            })}
          </div>
        ))}
      </nav>

      <div className="pk-sidebar-footer-slim pk-sidebar-footer-slim--solo">
        <button className="pk-theme-toggle" onClick={onToggleTheme} title={darkMode ? 'Light mode' : 'Dark mode'} data-comment-anchor="a0f0b49b18-pk-om-id-jsx-7">
          {darkMode ? '☼' : '◐'}
        </button>
      </div>
    </aside>);
}

/* ── Status Bar (bottom of app) ── */
const EPIGRAPHS = [
  'Memory is patient.',
  'Nothing is lost.',
  'Every page, kept.',
  'You\'ve been somewhere.',
  'The archive remembers.',
  'A small library, growing.'
];

const PK_SOURCES = [
  { id: 'chrome',  label: 'Chrome',  profile: 'Default',  color: '#4285F4', pages: 1847203, size: '7.2 GB' },
  { id: 'firefox', label: 'Firefox', profile: 'Work',     color: '#FF6B35', pages: 724891,  size: '3.6 GB' },
  { id: 'safari',  label: 'Safari',  profile: 'Personal', color: '#FF7139', pages: 275297,  size: '1.6 GB' },
];
const PK_SOURCES_TOTAL = PK_SOURCES.reduce((s, x) => s + x.pages, 0);

function PKStatusBar({ backupRunning, onNavigate, sourceFilter, onSourceFilterChange }) {
  const [epigraphIdx] = React.useState(() => Math.floor(Math.random() * EPIGRAPHS.length));
  const [open, setOpen] = React.useState(false);
  const popRef = React.useRef(null);

  // Close on outside click
  React.useEffect(() => {
    if (!open) return;
    function handleClick(e) {
      if (popRef.current && !popRef.current.contains(e.target)) setOpen(false);
    }
    function handleKey(e) { if (e.key === 'Escape') setOpen(false); }
    document.addEventListener('mousedown', handleClick);
    document.addEventListener('keydown', handleKey);
    return () => {
      document.removeEventListener('mousedown', handleClick);
      document.removeEventListener('keydown', handleKey);
    };
  }, [open]);

  const activeSource = sourceFilter ? PK_SOURCES.find(s => s.id === sourceFilter) : null;
  const displayLabel = activeSource
    ? `${activeSource.label} · ${activeSource.profile}`
    : '3 sources';

  return (
    <footer className="pk-statusbar" data-comment-anchor="228db78d28-div-130-7">
      <span className="pk-statusbar__group">
        <span className={`pk-statusbar__dot ${backupRunning ? 'pk-statusbar__dot--running' : ''}`}></span>
        <span>{backupRunning ? 'Archiving…' : 'Archive kept'}</span>
      </span>
      <span className="pk-statusbar__sep"></span>
      <span className="pk-statusbar__group">
        <span>2,847,391 pages · 12.4 GB</span>
      </span>
      <span className="pk-statusbar__sep"></span>
      <span className="pk-statusbar__group pk-statusbar__group--clickable" onClick={() => onNavigate('archive')}>
        <span>Since Sep 2021</span>
      </span>
      <span className="pk-statusbar__sep"></span>

      {/* ── Source switcher ── */}
      <span className="pk-statusbar__source-anchor" ref={popRef}>
        <span
          className={`pk-statusbar__group pk-statusbar__group--clickable pk-statusbar__source-trigger${open ? ' pk-statusbar__source-trigger--open' : ''}${activeSource ? ' pk-statusbar__source-trigger--filtered' : ''}`}
          onClick={() => setOpen(o => !o)}>
          {activeSource ? (
            <span className="pk-statusbar__profile-dot pk-statusbar__profile-dot--solid" style={{ color: activeSource.color }}></span>
          ) : (
            <span className="pk-statusbar__profiles">
              {PK_SOURCES.map(s => (
                <span key={s.id} className="pk-statusbar__profile-dot" style={{ color: s.color }}></span>
              ))}
            </span>
          )}
          <span>{displayLabel}</span>
          <svg className="pk-statusbar__source-caret" width="8" height="8" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d={open ? 'M4 10l4-4 4 4' : 'M4 6l4 4 4-4'} /></svg>
        </span>

        {open && (
          <div className="pk-source-popover">
            <div className="pk-source-popover__head">
              <span className="pk-source-popover__title">Sources</span>
              <span className="pk-source-popover__count">{PK_SOURCES.length} connected</span>
            </div>

            {/* All sources option */}
            <button
              className={`pk-source-popover__item${!sourceFilter ? ' pk-source-popover__item--active' : ''}`}
              onClick={() => { onSourceFilterChange(null); setOpen(false); }}>
              <span className="pk-source-popover__item-dots">
                {PK_SOURCES.map(s => (
                  <span key={s.id} className="pk-source-popover__mini-dot" style={{ background: s.color }}></span>
                ))}
              </span>
              <span className="pk-source-popover__item-main">
                <span className="pk-source-popover__item-label">All sources</span>
                <span className="pk-source-popover__item-meta">{PK_SOURCES_TOTAL.toLocaleString()} pages · 12.4 GB</span>
              </span>
              {!sourceFilter && <span className="pk-source-popover__check">✓</span>}
            </button>

            <div className="pk-source-popover__divider"></div>

            {/* Individual sources */}
            {PK_SOURCES.map(src => (
              <button
                key={src.id}
                className={`pk-source-popover__item${sourceFilter === src.id ? ' pk-source-popover__item--active' : ''}`}
                onClick={() => { onSourceFilterChange(sourceFilter === src.id ? null : src.id); setOpen(false); }}>
                <span className="pk-source-popover__item-dot" style={{ background: src.color }}></span>
                <span className="pk-source-popover__item-main">
                  <span className="pk-source-popover__item-label">{src.label} <span className="pk-source-popover__item-profile">· {src.profile}</span></span>
                  <span className="pk-source-popover__item-meta">{src.pages.toLocaleString()} pages · {src.size}</span>
                </span>
                {sourceFilter === src.id && <span className="pk-source-popover__check">✓</span>}
              </button>
            ))}

            <div className="pk-source-popover__foot">
              <button
                className="pk-source-popover__manage"
                onClick={() => { onNavigate('settings'); setOpen(false); }}>
                Manage sources…
              </button>
            </div>
          </div>
        )}
      </span>

      <span className="pk-statusbar__sep"></span>
      <span className="pk-statusbar__group">
        <span>Last archived 14:23 · +1,847</span>
      </span>

      <span className="pk-statusbar__epigraph">{EPIGRAPHS[epigraphIdx]}</span>
    </footer>);
}

/* ═══════════════════════════════════════════════════════════
   HEATMAP — GitHub-style, paper palette
   ═══════════════════════════════════════════════════════════ */
function PKHeatmap({ data, onSelectDate }) {
  // data: array of 364 days (52 weeks × 7), each {date, count, level: 0-4}
  // build 7 rows of 52 cells
  const rows = [[], [], [], [], [], [], []]; // Sun-Sat
  data.forEach((d, i) => {
    const dayOfWeek = i % 7;
    rows[dayOfWeek].push(d);
  });

  const dayLabels = ['', 'Mon', '', 'Wed', '', 'Fri', ''];
  const months = ['Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec', 'Jan', 'Feb', 'Mar', 'Apr', 'May'];

  return (
    <div className="pk-heatmap-wrap">
      <div className="pk-heatmap__months">
        <span></span>
        {months.map((m, i) => <span key={i} className="pk-heatmap__month">{m}</span>)}
        {/* fill remaining columns */}
      </div>
      <div className="pk-heatmap">
        {rows.map((row, ri) => (
          <React.Fragment key={ri}>
            <span className="pk-heatmap__daylabel">{dayLabels[ri]}</span>
            {row.map((cell, ci) => (
              <div
                key={ci}
                className={`pk-heatmap__cell ${cell.level > 0 ? `pk-heatmap__cell--l${cell.level}` : ''}`}
                title={`${cell.date} · ${cell.count} pages`}
                onClick={() => cell.count > 0 && onSelectDate && onSelectDate(cell.date)}
              />
            ))}
          </React.Fragment>
        ))}
      </div>
      <div className="pk-heatmap__legend">
        <span>Less</span>
        <div className="pk-heatmap__legend-cells">
          <span className="pk-heatmap__legend-cell"></span>
          <span className="pk-heatmap__legend-cell pk-heatmap__cell--l1"></span>
          <span className="pk-heatmap__legend-cell pk-heatmap__cell--l2"></span>
          <span className="pk-heatmap__legend-cell pk-heatmap__cell--l3"></span>
          <span className="pk-heatmap__legend-cell pk-heatmap__cell--l4"></span>
        </div>
        <span>More</span>
      </div>
    </div>);
}

function generateHeatmapData() {
  const data = [];
  const today = new Date();
  const start = new Date(today);
  start.setDate(start.getDate() - 363);
  for (let i = 0; i < 364; i++) {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    // weekend bias down, recent bias up
    const dow = d.getDay();
    const weekend = (dow === 0 || dow === 6) ? 0.5 : 1.0;
    const recency = 0.4 + (i / 364) * 0.6;
    const noise = Math.random();
    const intensity = weekend * recency * noise;
    const count = Math.floor(intensity * 220);
    let level = 0;
    if (count > 5) level = 1;
    if (count > 30) level = 2;
    if (count > 80) level = 3;
    if (count > 140) level = 4;
    data.push({ date: d.toISOString().slice(0, 10), count, level });
  }
  return data;
}

/* ═══════════════════════════════════════════════════════════
   DETAIL PANEL — Enhanced shared component
   Used everywhere we surface a visit; carries notes, tags,
   page-level insights, related entries, visit history.
   ═══════════════════════════════════════════════════════════ */
function PKDetailPanel({ entry, onClose, onNavigate, notes, tags, onUpdateNotes, onUpdateTags }) {
  const [tagInput, setTagInput] = React.useState('');
  const [notesValue, setNotesValue] = React.useState(notes || '');
  const noteSaveTimer = React.useRef(null);

  React.useEffect(() => {
    setNotesValue(notes || '');
  }, [entry?.id, notes]);

  if (!entry) return null;

  const handleNotesChange = (e) => {
    const v = e.target.value;
    setNotesValue(v);
    clearTimeout(noteSaveTimer.current);
    noteSaveTimer.current = setTimeout(() => onUpdateNotes && onUpdateNotes(v), 400);
  };

  const handleAddTag = () => {
    const t = tagInput.trim().toLowerCase();
    if (!t || (tags || []).includes(t)) return;
    onUpdateTags && onUpdateTags([...(tags || []), t]);
    setTagInput('');
  };

  const handleRemoveTag = (t) => {
    onUpdateTags && onUpdateTags((tags || []).filter(x => x !== t));
  };

  // Mock visit history
  const visitHistory = entry.visitHistoryData || [
    { date: '2026-05-16', count: Math.max(1, (entry.visitCount || 5) - 2) },
    { date: '2026-05-12', count: 3 },
    { date: '2026-04-22', count: 5 },
    { date: '2026-03-18', count: 2 },
    { date: '2026-02-09', count: 1 }
  ];
  const maxCount = Math.max(...visitHistory.map(v => v.count), 1);

  return (
    <div className="pk-detail-overlay">
      <div className="pk-detail-backdrop" onClick={onClose}></div>
      <div className="pk-detail-panel" data-comment-anchor="12c123eb46-div-152-7">
        <div className="pk-detail-header">
          <div className="pk-detail-label">Record</div>
          <button className="pk-detail-close" onClick={onClose} title="Close (Esc)">✕</button>
        </div>

        <div className="pk-detail-body" data-comment-anchor="a5912e87f8-div-159-9">
          {/* Title + URL */}
          <h2 className="pk-detail-title">{entry.title}</h2>
          <a className="pk-detail-url" href={entry.url} target="_blank" rel="noopener">{entry.url}</a>

          {/* Quick actions */}
          <div className="pk-detail-actions" style={{marginTop: 14}}>
            <button className="pk-detail-action pk-detail-action--primary">
              <span style={{width:11,height:11,display:'grid',placeItems:'center'}}><NavIcons.external /></span>
              Open
            </button>
            <button className="pk-detail-action">
              <span style={{width:11,height:11,display:'grid',placeItems:'center'}}><NavIcons.copy /></span>
              Copy URL
            </button>
            <button className="pk-detail-action">Refind…</button>
            <button className="pk-detail-action">Export</button>
          </div>

          <div className="pk-detail-divider"></div>

          {/* Visit summary */}
          <div className="pk-detail-row">
            <div className="pk-detail-field">
              <div className="pk-detail-label">First visit</div>
              <div className="pk-detail-value pk-detail-value--mono">2025-11-04 09:17</div>
            </div>
            <div className="pk-detail-field">
              <div className="pk-detail-label">Last visit</div>
              <div className="pk-detail-value pk-detail-value--mono">{entry.fullDate || '2026-05-16'} {entry.time}</div>
            </div>
          </div>
          <div className="pk-detail-row">
            <div className="pk-detail-field">
              <div className="pk-detail-label">Total visits</div>
              <div className="pk-detail-value pk-detail-value--mono">{entry.visitCount || 1}</div>
            </div>
            <div className="pk-detail-field">
              <div className="pk-detail-label">Typed directly</div>
              <div className="pk-detail-value pk-detail-value--mono">{entry.typedCount || 0}</div>
            </div>
          </div>

          {/* Visit history mini sparkline */}
          <div className="pk-detail-field" style={{marginTop: 14}}>
            <div className="pk-detail-label">Recent visits</div>
            <div className="pk-visit-history">
              {visitHistory.map((v, i) => (
                <div key={i} className="pk-visit-history__row">
                  <span className="pk-visit-history__date">{v.date}</span>
                  <div className="pk-visit-history__bar" style={{width: `${(v.count / maxCount) * 100}%`}}></div>
                  <span className="pk-visit-history__count">{v.count}×</span>
                </div>
              ))}
            </div>
          </div>

          <div className="pk-detail-divider"></div>

          {/* Provenance & technical */}
          <h3 className="pk-detail-section-title">Provenance</h3>
          <div className="pk-detail-field">
            <div className="pk-detail-label">Source</div>
            <div className="pk-detail-value">{entry.source || 'Chrome / Default Profile'}</div>
          </div>
          <div className="pk-detail-field">
            <div className="pk-detail-label">Transition</div>
            <div className="pk-detail-value">{entry.type || 'link'} {entry.type === 'typed' ? '(typed in address bar)' : entry.type === 'link' ? '(followed a link)' : ''}</div>
          </div>
          <div className="pk-detail-field">
            <div className="pk-detail-label">Captured in run</div>
            <div className="pk-detail-value pk-detail-value--mono" style={{ color: 'var(--ink-faint)' }}>
              #1847 · {entry.fullDate || '2026-05-16'} 18:30:00
            </div>
          </div>

          {entry.titleVersions && entry.titleVersions.length > 0 && (
            <div className="pk-detail-field" style={{marginTop: 12}}>
              <div className="pk-detail-label">Title history</div>
              {entry.titleVersions.map((v, i) =>
                <div key={i} style={{ marginTop: 6 }}>
                  <div className="pk-detail-value pk-detail-value--mono" style={{ color: 'var(--ink-faint)', fontSize: 10 }}>{v.date}</div>
                  <div className="pk-detail-value" style={{ fontSize: 12.5, fontStyle: 'italic' }}>{v.title}</div>
                </div>
              )}
            </div>
          )}

          <div className="pk-detail-divider"></div>

          {/* User Notes */}
          <h3 className="pk-detail-section-title">Your notes</h3>
          <textarea
            className="pk-notes-textarea"
            placeholder="Why did this matter? What were you looking for?"
            value={notesValue}
            onChange={handleNotesChange}
          />
          <div className="pk-notes-meta">
            <span>{notesValue ? `${notesValue.length} chars` : 'Empty'}</span>
            <span>{notesValue ? 'Saved · local-only' : ''}</span>
          </div>

          {/* Tags */}
          <h3 className="pk-detail-section-title" style={{marginTop: 18}}>Tags</h3>
          <div className="pk-tags">
            {(tags || []).map(t => (
              <span key={t} className="pk-tag">
                {t}
                <span className="pk-tag__remove" onClick={() => handleRemoveTag(t)}>✕</span>
              </span>
            ))}
            <input
              className="pk-tag-input"
              placeholder="+ add tag"
              value={tagInput}
              onChange={e => setTagInput(e.target.value)}
              onKeyDown={e => {
                if (e.key === 'Enter') { e.preventDefault(); handleAddTag(); }
                if (e.key === 'Backspace' && !tagInput && tags?.length) {
                  handleRemoveTag(tags[tags.length - 1]);
                }
              }}
            />
          </div>

          <div className="pk-spec-note">
            <strong>Design doc todo</strong>
            User-editable notes and tags need a new schema in the archive DB: per-canonical-URL annotations table, search indexing for notes, export/import round-trip, sync across profiles. Front-end prototype only — saves to localStorage for now.
          </div>

          <div className="pk-detail-divider"></div>

          {/* Page-level insights — entry points */}
          <h3 className="pk-detail-section-title">Look further</h3>
          <div className="pk-related-list">
            <div className="pk-related-item" onClick={() => onNavigate && onNavigate('insights')}>
              <span className="pk-related-item__label">
                <span className="pk-related-item__icon"><NavIcons.globe /></span>
                Page-level insights
              </span>
              <span className="pk-related-item__hint">{entry.visitCount || 47} visits over 6 months</span>
            </div>
            <div className="pk-related-item" onClick={() => onNavigate && onNavigate('insights')}>
              <span className="pk-related-item__label">
                <span className="pk-related-item__icon"><NavIcons.globe /></span>
                All of <code style={{fontFamily:'var(--font-mono)', fontSize:11, color:'var(--accent-text)'}}>{entry.domain}</code>
              </span>
              <span className="pk-related-item__hint">2,341 pages</span>
            </div>
            <div className="pk-related-item" onClick={() => onNavigate && onNavigate('insights')}>
              <span className="pk-related-item__label">
                <span className="pk-related-item__icon"><NavIcons.branch /></span>
                Thread: Rust async runtime
              </span>
              <span className="pk-related-item__hint">89 pages · active</span>
            </div>
            <div className="pk-related-item" onClick={() => onNavigate && onNavigate('insights')}>
              <span className="pk-related-item__label">
                <span className="pk-related-item__icon"><NavIcons.schedule /></span>
                Session: Apr 5, 20:15–20:42
              </span>
              <span className="pk-related-item__hint">14 pages</span>
            </div>
          </div>
        </div>
      </div>
    </div>);
}

/* ── Search Palette (⌘K) — quick switcher ── */
function PKSearchPalette({ onClose, onSelect, searchData, onOpenFullSearch }) {
  const [query, setQuery] = React.useState('');
  const inputRef = React.useRef(null);

  React.useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const results = React.useMemo(() => {
    if (!query.trim()) return searchData.slice(0, 8);
    const q = query.toLowerCase();
    return searchData.filter((item) =>
      item.title.toLowerCase().includes(q) ||
      item.domain.toLowerCase().includes(q) ||
      item.url && item.url.toLowerCase().includes(q)
    ).slice(0, 10);
  }, [query, searchData]);

  return (
    <div className="pk-search-overlay">
      <div className="pk-search-bg" onClick={onClose}></div>
      <div className="pk-search-dialog">
        <input
          ref={inputRef}
          className="pk-search-input"
          type="text"
          placeholder="Find a page…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') onClose();
            if (e.key === 'Enter' && e.metaKey && onOpenFullSearch) {
              onOpenFullSearch(query);
              onClose();
            }
          }} />

        <div className="pk-search-results">
          {results.length === 0 && query.trim() ?
            <div className="pk-empty" style={{ padding: '30px 20px' }}>
              <div className="pk-empty-text">Nothing here yet. Memory is patient.</div>
            </div> :
            results.map((item, i) =>
              <div
                key={i}
                className="pk-search-item"
                onClick={() => { onSelect(item); onClose(); }}>
                <div className="pk-search-item-icon" style={{ background: getDomainColor(item.domain) }}>
                  {getDomainAbbr(item.domain)}
                </div>
                <div className="pk-search-item-text">
                  <div className="pk-search-item-title">{item.title}</div>
                  <div className="pk-search-item-url">{item.domain} · {item.time}</div>
                </div>
              </div>
            )
          }
        </div>
        <div className="pk-search-hint">
          <span>↵ open</span>
          <span>⌘↵ full search</span>
          <span>↑↓ navigate</span>
          <span>esc close</span>
        </div>
      </div>
    </div>);
}

/* ── Export to window ── */
Object.assign(window, {
  PKSidebar, PKStatusBar, PKDetailPanel, PKSearchPalette, PKBrandMark,
  PKHeatmap, generateHeatmapData,
  NavIcons, getDomainColor, getDomainAbbr, DOMAIN_COLORS,
  EPIGRAPHS
});
