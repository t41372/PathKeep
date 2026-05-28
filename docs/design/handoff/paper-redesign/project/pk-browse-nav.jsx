/* ═══════════════════════════════════════════════════════════
   PathKeep Redesign — Browse Navigation
   Calendar popover, prev/next day, year rail, density.
   Built to feel calm at 21,900-day scale.
   ═══════════════════════════════════════════════════════════ */

/* ── Anchor "today" to the demo's archive (May 18, 2026) ── */
const ARCHIVE_TODAY = '2026-05-18';
const ARCHIVE_LATEST = '2026-05-16'; // most recent loaded day
const ARCHIVE_FIRST_YEAR = 1966;
const ARCHIVE_LAST_YEAR = 2026;

/* ── ISO date helpers (work in local time, no UTC drift) ── */
function dateToISO(d) {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}
function isoToDate(s) {
  const [y, m, d] = s.split('-').map(Number);
  return new Date(y, m - 1, d);
}
function addDays(iso, n) {
  const d = isoToDate(iso);
  d.setDate(d.getDate() + n);
  return dateToISO(d);
}
function prettyDay(iso, opts = {}) {
  const d = isoToDate(iso);
  return d.toLocaleDateString('en-US', {
    weekday: opts.short ? 'short' : 'long',
    month: 'short',
    day: 'numeric',
    year: opts.year !== false ? 'numeric' : undefined
  });
}

/* ═══ ARCHIVE DENSITY — deterministic sparse map across 60 years ═══ */
function generateArchiveDensity() {
  const byDate = new Map();
  const byYear = new Map();
  const byMonth = new Map();

  // Deterministic pseudo-random
  const rng = (seed) => {
    const x = Math.sin(seed * 12.9898 + 78.233) * 43758.5453;
    return x - Math.floor(x);
  };

  const start = new Date(ARCHIVE_FIRST_YEAR, 0, 1);
  const end = isoToDate(ARCHIVE_LATEST);

  let total = 0;
  let activeDays = 0;
  let peakCount = 0;
  let peakDate = null;

  for (let d = new Date(start); d <= end; d.setDate(d.getDate() + 1)) {
    const year = d.getFullYear();
    const month = d.getMonth();
    const dow = d.getDay();
    const dayOfYear = Math.floor((d - new Date(year, 0, 1)) / 86400000);
    const seed = year * 1000 + dayOfYear;

    // Era curve — sparse early years, dense modern
    const yearsSinceStart = year - ARCHIVE_FIRST_YEAR;
    const eraScale = Math.pow(yearsSinceStart / 60, 1.8) * 0.95 + 0.02;

    // Weekend bias
    const dowScale = (dow === 0 || dow === 6) ? 0.55 : 1.0;

    // Some days simply blank
    const noise = rng(seed);
    const skipChance = Math.max(0.05, 0.65 - eraScale * 0.6);

    let count = 0;
    if (noise > skipChance) {
      const intensity = rng(seed * 1.7 + 0.3) * eraScale * dowScale;
      count = Math.floor(intensity * 2400);
      // Occasional binge days
      if (rng(seed * 3.1) > 0.985) count = Math.floor(count * 2.2);
    }

    const ds = dateToISO(d);
    byDate.set(ds, count);
    byYear.set(year, (byYear.get(year) || 0) + count);
    const mk = `${year}-${String(month + 1).padStart(2, '0')}`;
    byMonth.set(mk, (byMonth.get(mk) || 0) + count);

    if (count > 0) activeDays++;
    total += count;
    if (count > peakCount) { peakCount = count; peakDate = ds; }
  }

  return {
    byDate, byYear, byMonth,
    total, activeDays, peakCount, peakDate,
    bounds: {
      first: dateToISO(start),
      last: ARCHIVE_LATEST,
      firstYear: ARCHIVE_FIRST_YEAR,
      lastYear: ARCHIVE_LAST_YEAR,
      totalDays: Math.floor((end - start) / 86400000) + 1
    }
  };
}

/* ── Density tier helpers ── */
function dayTier(count) {
  if (!count) return 0;
  if (count < 30) return 1;
  if (count < 150) return 2;
  if (count < 500) return 3;
  return 4;
}
function yearTier(count) {
  if (!count) return 0;
  if (count < 5000) return 1;
  if (count < 30000) return 2;
  if (count < 90000) return 3;
  return 4;
}

/* ═══ CALENDAR POPOVER ═══ */
function CalendarPopover({ value, density, bounds, onSelect, loadedDates }) {
  const initial = value ? isoToDate(value) : isoToDate(ARCHIVE_LATEST);
  const [viewYear, setViewYear] = React.useState(initial.getFullYear());
  const [viewMonth, setViewMonth] = React.useState(initial.getMonth());
  const [hover, setHover] = React.useState(null);
  const [showYearPicker, setShowYearPicker] = React.useState(false);

  const firstDow = new Date(viewYear, viewMonth, 1).getDay();
  const daysInMonth = new Date(viewYear, viewMonth + 1, 0).getDate();
  const leading = (firstDow + 6) % 7; // Monday-first

  const cells = [];
  for (let i = 0; i < leading; i++) cells.push(null);
  for (let day = 1; day <= daysInMonth; day++) {
    const iso = `${viewYear}-${String(viewMonth + 1).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
    cells.push({ iso, day, count: density.byDate.get(iso) || 0 });
  }
  while (cells.length % 7 !== 0) cells.push(null);

  const monthNames = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
  const dowLabels = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];

  const stepMonth = (delta) => {
    let nm = viewMonth + delta;
    let ny = viewYear;
    while (nm < 0) { nm += 12; ny--; }
    while (nm > 11) { nm -= 12; ny++; }
    if (ny < bounds.firstYear || ny > bounds.lastYear) return;
    setViewYear(ny);
    setViewMonth(nm);
  };

  const monthTotal = density.byMonth.get(`${viewYear}-${String(viewMonth + 1).padStart(2, '0')}`) || 0;
  const monthActive = cells.filter((c) => c && c.count > 0).length;

  // Hover preview values
  const previewCount = hover ? hover.count : monthTotal;
  const previewLabel = hover
    ? prettyDay(hover.iso, { short: true })
    : `${monthNames[viewMonth]} ${viewYear}`;
  const previewSub = hover
    ? `${hover.count.toLocaleString()} pages archived`
    : `${monthActive} active days · ${monthTotal.toLocaleString()} pages`;

  return (
    <div className="cs-cal" onClick={(e) => e.stopPropagation()}>
      {/* Header */}
      <div className="cs-cal__head">
        <button
          type="button"
          className="cs-cal__nav"
          onClick={() => stepMonth(-1)}
          aria-label="Previous month">
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M10 3l-5 5 5 5" /></svg>
        </button>
        <button
          type="button"
          className={'cs-cal__title' + (showYearPicker ? ' cs-cal__title--open' : '')}
          onClick={() => setShowYearPicker((s) => !s)}>
          <span className="cs-cal__title-month">{monthNames[viewMonth]}</span>
          <span className="cs-cal__title-year">{viewYear}</span>
          <svg width="9" height="9" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" style={{ marginLeft: 4, opacity: 0.7 }}><path d="M4 6l4 4 4-4" /></svg>
        </button>
        <button
          type="button"
          className="cs-cal__nav"
          onClick={() => stepMonth(1)}
          aria-label="Next month">
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M6 3l5 5-5 5" /></svg>
        </button>
      </div>

      {showYearPicker ? (
        <YearPicker
          density={density}
          bounds={bounds}
          currentYear={viewYear}
          onSelect={(y) => { setViewYear(y); setShowYearPicker(false); }} />
      ) : (
        <>
          {/* Day-of-week labels */}
          <div className="cs-cal__dow">
            {dowLabels.map((l, i) => <span key={i}>{l}</span>)}
          </div>

          {/* Day grid */}
          <div className="cs-cal__grid">
            {cells.map((cell, i) => {
              if (!cell) return <div key={i} className="cs-cal__cell cs-cal__cell--empty"></div>;
              const t = dayTier(cell.count);
              const isToday = cell.iso === ARCHIVE_TODAY;
              const isSelected = cell.iso === value;
              const isLoaded = loadedDates && loadedDates.has(cell.iso);
              const isFuture = cell.iso > ARCHIVE_LATEST;
              return (
                <button
                  key={cell.iso}
                  type="button"
                  className={
                    'cs-cal__cell cs-cal__cell--t' + t +
                    (isToday ? ' cs-cal__cell--today' : '') +
                    (isSelected ? ' cs-cal__cell--selected' : '') +
                    (isLoaded ? ' cs-cal__cell--loaded' : '') +
                    (isFuture ? ' cs-cal__cell--future' : '')
                  }
                  disabled={isFuture}
                  onMouseEnter={() => setHover(cell)}
                  onMouseLeave={() => setHover(null)}
                  onClick={() => !isFuture && onSelect(cell.iso)}>
                  <span className="cs-cal__cell-day">{cell.day}</span>
                </button>);

            })}
          </div>

          {/* Preview row — month total OR hovered day */}
          <div className="cs-cal__preview">
            <div className="cs-cal__preview-main">
              <span className="cs-cal__preview-label">{previewLabel}</span>
              <span className="cs-cal__preview-sub">{previewSub}</span>
            </div>
            <DensitySpark
              count={previewCount}
              max={Math.max(density.peakCount, 1)} />
          </div>
        </>
      )}

      {/* Footer */}
      <div className="cs-cal__foot">
        <button
          type="button"
          className="cs-cal__foot-btn"
          onClick={() => onSelect(ARCHIVE_TODAY)}>
          <kbd>T</kbd>
          <span>Today</span>
        </button>
        <button
          type="button"
          className="cs-cal__foot-btn"
          onClick={() => {
            const d = isoToDate(value || ARCHIVE_LATEST);
            const yearAgo = new Date(d.getFullYear() - 1, d.getMonth(), d.getDate());
            onSelect(dateToISO(yearAgo));
          }}>
          <span>1y ago</span>
        </button>
        <span className="cs-cal__foot-meta">
          {bounds.firstYear}–{bounds.lastYear} · {bounds.totalDays.toLocaleString()} days
        </span>
      </div>
    </div>);

}

/* ── Density spark (tiny bar for a day or month) ── */
function DensitySpark({ count, max }) {
  const pct = Math.min(100, Math.sqrt(count / max) * 100); // sqrt for soft scale
  const tier = count === 0 ? 0 : count < 30 ? 1 : count < 200 ? 2 : count < 800 ? 3 : 4;
  return (
    <div className="cs-cal__spark">
      <div className={'cs-cal__spark-fill cs-cal__spark-fill--t' + tier} style={{ width: `${pct}%` }}></div>
    </div>);

}

/* ═══ YEAR PICKER (inside calendar) ═══ */
function YearPicker({ density, bounds, currentYear, onSelect }) {
  const listRef = React.useRef(null);
  const years = [];
  for (let y = bounds.lastYear; y >= bounds.firstYear; y--) years.push(y);
  const maxYearCount = Math.max(...density.byYear.values(), 1);

  // Manually scroll the active year into the visible window
  React.useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const active = list.querySelector('.cs-cal__year--current');
    if (!active) return;
    const lRect = list.getBoundingClientRect();
    const aRect = active.getBoundingClientRect();
    list.scrollTop += aRect.top - lRect.top - lRect.height / 2 + aRect.height / 2;
  }, []);

  return (
    <div className="cs-cal__yearpicker" ref={listRef}>
      {years.map((y) => {
        const count = density.byYear.get(y) || 0;
        const t = yearTier(count);
        const pct = (count / maxYearCount) * 100;
        const label = count === 0 ? '—' :
        count >= 1000 ? `${(count / 1000).toFixed(count >= 10000 ? 0 : 1)}k` :
        count.toLocaleString();
        return (
          <button
            key={y}
            type="button"
            className={'cs-cal__year' + (y === currentYear ? ' cs-cal__year--current' : '')}
            onClick={() => onSelect(y)}>
            <span className="cs-cal__year-num">{y}</span>
            <span className="cs-cal__year-bar">
              <span className={'cs-cal__year-fill cs-cal__year-fill--t' + t} style={{ width: `${pct}%` }}></span>
            </span>
            <span className="cs-cal__year-count">{label}</span>
          </button>);

      })}
    </div>);

}

/* ═══ YEAR RAIL — vertical mini-map of the whole archive ═══ */
function YearRail({ density, bounds, currentDate, onJump }) {
  const years = [];
  for (let y = bounds.lastYear; y >= bounds.firstYear; y--) years.push(y);
  const currentYear = currentDate ? parseInt(currentDate.slice(0, 4), 10) : bounds.lastYear;
  const currentMonth = currentDate ? parseInt(currentDate.slice(5, 7), 10) - 1 : 0;

  return (
    <div className="cs-rail" aria-label="Year scrubber">
      <div className="cs-rail__head">
        <span>{bounds.lastYear}</span>
        <span className="cs-rail__head-sub">now</span>
      </div>
      <div className="cs-rail__inner">
        {years.map((y) => {
          const count = density.byYear.get(y) || 0;
          const t = yearTier(count);
          const isCurrent = y === currentYear;
          const isDecade = y % 10 === 0;
          // Pick mid-year date, but if that month is empty, walk backward
          let jumpDate = `${y}-06-15`;
          if (y === bounds.lastYear) jumpDate = bounds.last;

          return (
            <div
              key={y}
              className={
                'cs-rail__year cs-rail__year--t' + t +
                (isCurrent ? ' cs-rail__year--current' : '') +
                (isDecade ? ' cs-rail__year--decade' : '')
              }
              title={`${y} · ${count.toLocaleString()} pages`}
              onClick={() => onJump(jumpDate)}>

              {isCurrent &&
              <div
                className="cs-rail__month-indicator"
                style={{ top: `${currentMonth / 12 * 100}%` }}
                aria-hidden="true"></div>
              }
              {isDecade && <span className="cs-rail__label">{y}</span>}
            </div>);

        })}
      </div>
      <div className="cs-rail__foot">
        <span>{bounds.firstYear}</span>
        <span className="cs-rail__head-sub">first</span>
      </div>
    </div>);

}

/* ═══ DAY-NAV CONTROL — prev / current-day pill / next / today ═══ */
function DayNavControl({
  currentDate, density, loadedDates, calOpen, onToggleCal,
  onPrev, onNext, onToday, onJump
}) {
  const popRef = React.useRef(null);
  const count = density.byDate.get(currentDate) || 0;
  const isLoaded = loadedDates && loadedDates.has(currentDate);

  // Day-of-week + date
  const d = currentDate ? isoToDate(currentDate) : null;
  const dow = d ? d.toLocaleDateString('en-US', { weekday: 'short' }) : '';
  const md = d ? d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }) : '';
  const yr = d ? d.getFullYear() : '';

  const t = dayTier(count);

  // How far back?
  const daysAgo = d ? Math.round((isoToDate(ARCHIVE_TODAY) - d) / 86400000) : 0;
  let agoLabel = '';
  if (daysAgo === 0) agoLabel = 'today';
  else if (daysAgo === 1) agoLabel = 'yesterday';
  else if (daysAgo < 7) agoLabel = `${daysAgo}d ago`;
  else if (daysAgo < 60) agoLabel = `${Math.round(daysAgo / 7)}w ago`;
  else if (daysAgo < 730) agoLabel = `${Math.round(daysAgo / 30)}mo ago`;
  else agoLabel = `${(daysAgo / 365).toFixed(1)}y ago`;

  return (
    <div className="cs-daynav" ref={popRef}>
      <button
        type="button"
        className="cs-daynav__btn cs-daynav__btn--arrow"
        onClick={onPrev}
        title="Previous day (←)"
        aria-label="Previous day">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M10 3l-5 5 5 5" /></svg>
      </button>

      <button
        type="button"
        className={'cs-daynav__pill' + (calOpen ? ' cs-daynav__pill--open' : '')}
        onClick={onToggleCal}
        aria-haspopup="dialog"
        aria-expanded={calOpen}
        title="Open calendar (G)">
        <svg className="cs-daynav__pill-icon" width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
          <rect x="2.2" y="3.2" width="11.6" height="10.6" rx="1.2" />
          <path d="M2.2 6.4h11.6" />
          <path d="M5.4 1.8v2.6" />
          <path d="M10.6 1.8v2.6" />
        </svg>
        <span className="cs-daynav__pill-main">
          <span className="cs-daynav__pill-dow">{dow}</span>
          <span className="cs-daynav__pill-md">{md}</span>
          <span className="cs-daynav__pill-yr">{yr}</span>
        </span>
        <span className="cs-daynav__pill-aside">
          <span className={'cs-daynav__pill-dot cs-daynav__pill-dot--t' + t} aria-hidden="true"></span>
          <span className="cs-daynav__pill-count">
            {count > 0 ? `${count.toLocaleString()}p` : 'empty'}
          </span>
          <span className="cs-daynav__pill-ago">{agoLabel}</span>
        </span>
        <svg width="9" height="9" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: 0.55 }}><path d="M4 6l4 4 4-4" /></svg>
      </button>

      <button
        type="button"
        className="cs-daynav__btn cs-daynav__btn--arrow"
        onClick={onNext}
        title="Next day (→)"
        aria-label="Next day">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="M6 3l5 5-5 5" /></svg>
      </button>

      <span className="cs-daynav__sep" aria-hidden="true"></span>

      <button
        type="button"
        className={'cs-daynav__btn cs-daynav__btn--today' + (currentDate === ARCHIVE_TODAY ? ' cs-daynav__btn--today-current' : '')}
        onClick={onToday}
        title="Jump to today (T)">
        Today
      </button>
    </div>);

}

/* ═══ LOADING SKELETON — page-of-results placeholder ═══ */
function LoadingSkeleton({ direction = 'down', label }) {
  const lines = [0, 1, 2];
  return (
    <div className={'cs-skeleton cs-skeleton--' + direction}>
      <div className="cs-skeleton__pulse"></div>
      <div className="cs-skeleton__label">
        <span className="cs-skeleton__spinner"></span>
        <span>{label || (direction === 'down' ? 'Loading older days…' : 'Loading newer days…')}</span>
      </div>
      <div className="cs-skeleton__rows">
        {lines.map((i) =>
        <div key={i} className="cs-skeleton__row">
            <div className="cs-skeleton__row-icon"></div>
            <div className="cs-skeleton__row-bar" style={{ width: `${60 + i * 8}%` }}></div>
            <div className="cs-skeleton__row-time"></div>
          </div>
        )}
      </div>
    </div>);

}

/* ═══ PLACEHOLDER DAY — when a jumped-to date isn't loaded yet ═══ */
function PlaceholderDay({ iso, count }) {
  const d = isoToDate(iso);
  const label = d.toLocaleDateString('en-US', { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' });
  const [loading, setLoading] = React.useState(true);
  React.useEffect(() => {
    const t = setTimeout(() => setLoading(false), 900);
    return () => clearTimeout(t);
  }, [iso]);

  return (
    <div className="cs-placeholder-day">
      <div className="cs-day-sticky">
        <div className="cs-day-sticky__inner" style={{ borderBottom: '2px dashed var(--accent)' }}>
          <div>
            <span style={{ fontFamily: 'var(--font-serif)', fontSize: 22, fontWeight: 400, color: 'var(--ink)', letterSpacing: '-0.02em' }}>
              {label}
            </span>
            <span style={{ fontFamily: 'var(--font-sans)', fontSize: 12.5, color: 'var(--ink-faint)', marginLeft: 14 }}>
              {count.toLocaleString()} pages archived
            </span>
          </div>
          <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--accent-text)' }}>
            ▸ Jumped here
          </span>
        </div>
      </div>
      <div className="cs-placeholder-card">
        {loading ?
        <>
            <div className="cs-placeholder-card__pulse"></div>
            <div className="cs-placeholder-card__text">
              <span className="cs-skeleton__spinner"></span>
              <span>Fetching this day from local archive…</span>
            </div>
            <div className="cs-placeholder-card__meta">
              <span>~{count.toLocaleString()} records · ~{Math.max(1, Math.round(count / 60))} sessions</span>
              <span>local · 0 network</span>
            </div>
          </> :

        <>
            <div className="cs-placeholder-card__text" style={{ color: 'var(--ink-secondary)' }}>
              <span style={{ fontFamily: 'var(--font-serif)', fontStyle: 'italic' }}>
                This day isn't on-screen yet — but it's in the archive. In the production app it would render now.
              </span>
            </div>
            <div className="cs-placeholder-card__meta">
              <span>{count.toLocaleString()} records waiting</span>
              <span>scroll to neighbours to keep going</span>
            </div>
          </>
        }
      </div>
    </div>);

}

Object.assign(window, {
  generateArchiveDensity,
  CalendarPopover, YearRail, DayNavControl,
  LoadingSkeleton, PlaceholderDay,
  dayTier, yearTier, dateToISO, isoToDate, addDays, prettyDay,
  ARCHIVE_TODAY, ARCHIVE_LATEST, ARCHIVE_FIRST_YEAR, ARCHIVE_LAST_YEAR
});
