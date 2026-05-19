/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Main App Shell
   Routing, status bar, notes/tags persistence, tweaks
   ═══════════════════════════════════════════════════════════ */

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "accentColor": "#3d5a80",
  "darkMode": false,
  "serifFont": "Newsreader",
  "density": "comfortable",
  "paperTexture": true
}/*EDITMODE-END*/;

// localStorage helpers for note/tag persistence (prototype only)
const NOTES_KEY = 'pk.notes';
const TAGS_KEY = 'pk.tags';
function loadMap(key) {
  try { return JSON.parse(localStorage.getItem(key) || '{}'); } catch { return {}; }
}
function saveMap(key, map) {
  try { localStorage.setItem(key, JSON.stringify(map)); } catch {}
}

function PathKeepApp() {
  const [tweaks, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const [activeView, setActiveView] = React.useState('home');
  const [selectedEntry, setSelectedEntry] = React.useState(null);
  const [showSearch, setShowSearch] = React.useState(false);
  const [showDetail, setShowDetail] = React.useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = React.useState(() => {
    try { return localStorage.getItem('pk.sidebar.collapsed') === 'true'; } catch { return false; }
  });
  const [searchInitialQuery, setSearchInitialQuery] = React.useState('');
  const [browseTargetDate, setBrowseTargetDate] = React.useState(null);
  const [browseTargetEntry, setBrowseTargetEntry] = React.useState(null);
  const [browseTargetSource, setBrowseTargetSource] = React.useState(null);
  const [browseTargetQuery, setBrowseTargetQuery] = React.useState('');
  const [sourceFilter, setSourceFilter] = React.useState(null);

  // Navigate with optional payload (e.g. {date, entryId, source, query} for browse jump-to)
  const handleNavigate = React.useCallback((view, opts) => {
    if (view === 'browse' && opts) {
      if (opts.date) setBrowseTargetDate(opts.date);
      setBrowseTargetEntry(opts.entryId || null);
      setBrowseTargetSource(opts.source || (opts.date ? 'on-this-day' : null));
      setBrowseTargetQuery(opts.query || '');
    } else if (view !== 'browse') {
      setBrowseTargetDate(null);
      setBrowseTargetEntry(null);
      setBrowseTargetSource(null);
      setBrowseTargetQuery('');
    }
    setActiveView(view);
  }, []);

  const clearBrowseTarget = React.useCallback(() => {
    setBrowseTargetDate(null);
    setBrowseTargetEntry(null);
    setBrowseTargetSource(null);
    setBrowseTargetQuery('');
  }, []);

  const handleToggleSidebar = React.useCallback(() => {
    setSidebarCollapsed(prev => {
      const next = !prev;
      try { localStorage.setItem('pk.sidebar.collapsed', String(next)); } catch {}
      return next;
    });
  }, []);

  // Notes & tags state — keyed by canonical URL
  const [notesMap, setNotesMap] = React.useState(() => loadMap(NOTES_KEY));
  const [tagsMap, setTagsMap] = React.useState(() => loadMap(TAGS_KEY));

  // Apply theme
  React.useEffect(() => {
    const html = document.documentElement;
    html.setAttribute('data-theme', tweaks.darkMode ? 'dark' : 'light');
    html.style.setProperty('--accent-color', tweaks.accentColor);

    const serifMap = {
      'Newsreader': "'Newsreader', Georgia, 'Noto Serif', serif",
      'Source Serif': "'Source Serif 4', Georgia, serif",
      'System Serif': "Georgia, 'Noto Serif', 'PingFang TC', serif"
    };
    html.style.setProperty('--font-serif', serifMap[tweaks.serifFont] || serifMap['Newsreader']);

    if (!tweaks.paperTexture) {
      html.style.setProperty('--noise-opacity', '0');
      html.style.setProperty('--vignette-opacity', '0');
    } else {
      html.style.removeProperty('--noise-opacity');
      html.style.removeProperty('--vignette-opacity');
    }
  }, [tweaks]);

  // Keyboard: ⌘K
  React.useEffect(() => {
    function handleKey(e) {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setShowSearch((s) => !s);
      }
      if (e.key === 'Escape') {
        if (showSearch) setShowSearch(false);
        else if (showDetail) { setShowDetail(false); setSelectedEntry(null); }
      }
    }
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [showSearch, showDetail]);

  const handleSelectEntry = (entry) => {
    setSelectedEntry(entry);
    setShowDetail(true);
  };

  const handleOpenFullSearch = (q) => {
    setSearchInitialQuery(q);
    setActiveView('search');
  };

  const handleUpdateNotes = (value) => {
    if (!selectedEntry) return;
    const key = selectedEntry.url || selectedEntry.id;
    const next = { ...notesMap, [key]: value };
    setNotesMap(next);
    saveMap(NOTES_KEY, next);
  };

  const handleUpdateTags = (tags) => {
    if (!selectedEntry) return;
    const key = selectedEntry.url || selectedEntry.id;
    const next = { ...tagsMap, [key]: tags };
    setTagsMap(next);
    saveMap(TAGS_KEY, next);
  };

  const allVisits = React.useMemo(() => getAllVisits(), []);

  const currentKey = selectedEntry ? (selectedEntry.url || selectedEntry.id) : null;
  const currentNotes = currentKey ? notesMap[currentKey] : '';
  const currentTags = currentKey ? (tagsMap[currentKey] || []) : [];

  // Page titles
  const PAGE_META = {
    home: { title: 'Home', subtitle: 'Welcome back' },
    browse: { title: 'Browse', subtitle: 'A day at a time' },
    search: { title: 'Search', subtitle: 'Find what you read' },
    insights: { title: 'Intelligence', subtitle: 'Patterns & threads' },
    assistant: { title: 'Assistant', subtitle: 'Chat about your past' },
    import: { title: 'Import', subtitle: 'Recover what was lost' },
    archive: { title: 'Archive', subtitle: 'Audit & integrity' },
    schedule: { title: 'Schedule', subtitle: 'Backup cadence' },
    settings: { title: 'Settings', subtitle: 'Preferences' }
  };

  const meta = PAGE_META[activeView] || PAGE_META.home;

  const renderContent = () => {
    switch (activeView) {
      case 'home':
        return <HomeView onNavigate={handleNavigate} onSelectEntry={handleSelectEntry} />;
      case 'browse':
        return <ContactSheetView
          onSelectEntry={handleSelectEntry}
          selectedId={selectedEntry?.id}
          targetDate={browseTargetDate}
          targetEntry={browseTargetEntry}
          targetSource={browseTargetSource}
          targetQuery={browseTargetQuery}
          onClearTarget={clearBrowseTarget} />;
      case 'search':
        return <SearchView
          onSelectEntry={handleSelectEntry}
          onNavigate={handleNavigate}
          initialQuery={searchInitialQuery}
          searchData={allVisits} />;
      case 'insights':
        return <IntelligenceView onNavigate={handleNavigate} onSelectEntry={handleSelectEntry} />;
      case 'assistant':
        return <AssistantView onSelectEntry={handleSelectEntry} />;
      case 'import':
        return <ImportView />;
      case 'archive':
        return <AuditView onNavigate={handleNavigate} />;
      case 'schedule':
        return <PlaceholderView description="Automatic backups, on your terms. Quiet by design." />;
      case 'settings':
        return <PlaceholderView description="Archive path, encryption, AI providers, source profiles." />;
      default:
        return <HomeView onNavigate={setActiveView} />;
    }
  };

  return (
    <div className="pk-shell">
      <PKSidebar
        activeView={activeView}
        onNavigate={handleNavigate}
        darkMode={tweaks.darkMode}
        onToggleTheme={() => setTweak('darkMode', !tweaks.darkMode)}
        collapsed={sidebarCollapsed}
        onToggleCollapse={handleToggleSidebar} />

      <div className="pk-main">
        {/* Topbar */}
        <div className="pk-topbar">
          <div className="pk-topbar-left">
            <h1 className="pk-page-title">{meta.title}</h1>
            <span className="pk-page-subtitle">{meta.subtitle}</span>
          </div>
          <div className="pk-topbar-right">
            <button className="pk-search-trigger" onClick={() => setShowSearch(true)}>
              <svg width="14" height="14" viewBox="0 0 20 20" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                <circle cx="9" cy="9" r="5.5" /><path d="M13.5 13.5L17.5 17.5" />
              </svg>
              Find a page…
              <kbd>⌘K</kbd>
            </button>
            <button className="pk-btn-backup">
              <span style={{ fontSize: 10 }}>▶</span>
              Back up now
            </button>
          </div>
        </div>

        {/* Content */}
        <div className="pk-content" data-comment-anchor="09b4f1f50a-div-145-9">
          {renderContent()}
        </div>

        {/* Status Bar */}
        <PKStatusBar
          backupRunning={false}
          onNavigate={handleNavigate}
          sourceFilter={sourceFilter}
          onSourceFilterChange={setSourceFilter}
        />
      </div>

      {/* Detail panel */}
      {showDetail && selectedEntry &&
        <PKDetailPanel
          entry={selectedEntry}
          onClose={() => { setShowDetail(false); setSelectedEntry(null); }}
          onNavigate={(v) => { setShowDetail(false); setSelectedEntry(null); handleNavigate(v); }}
          notes={currentNotes}
          tags={currentTags}
          onUpdateNotes={handleUpdateNotes}
          onUpdateTags={handleUpdateTags}
        />
      }

      {/* Search palette */}
      {showSearch &&
        <PKSearchPalette
          onClose={() => setShowSearch(false)}
          onSelect={handleSelectEntry}
          searchData={allVisits}
          onOpenFullSearch={handleOpenFullSearch} />
      }

      {/* Tweaks Panel */}
      <TweaksPanel title="Tweaks">
        <TweakSection label="Appearance">
          <TweakToggle label="Dark Mode" value={tweaks.darkMode} onChange={(v) => setTweak('darkMode', v)} />
          <TweakToggle label="Paper Texture" value={tweaks.paperTexture} onChange={(v) => setTweak('paperTexture', v)} />
        </TweakSection>
        <TweakSection label="Palette">
          <TweakColor
            label="Accent"
            value={tweaks.accentColor}
            onChange={(v) => setTweak('accentColor', v)}
            options={['#3d5a80', '#6b4c3b', '#8b4049', '#2d6a4f']} />
        </TweakSection>
        <TweakSection label="Typography">
          <TweakSelect
            label="Serif Font"
            value={tweaks.serifFont}
            onChange={(v) => setTweak('serifFont', v)}
            options={['Newsreader', 'Source Serif', 'System Serif']} />
        </TweakSection>
        <TweakSection label="Density">
          <TweakRadio
            label="Layout"
            value={tweaks.density}
            onChange={(v) => setTweak('density', v)}
            options={['comfortable', 'compact']} />
        </TweakSection>
      </TweaksPanel>
    </div>);
}

Object.assign(window, { PathKeepApp });
