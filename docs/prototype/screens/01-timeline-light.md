# 01 — Timeline (Day mode)

> The hero screen. If only one mockup is reviewed, it is this one.
> Image: [`../images/01-timeline-light.jpg`](../images/01-timeline-light.jpg)

## Purpose

The Timeline is the canonical view of one's archive: a day-by-day, hour-by-hour, vertical list of every page that PathKeep has captured. It is the screen the user opens to *re-encounter* the past — not to *search* for a specific result. Search is reached via ⌘K (see `02-search-overlay.md`) and lives over this screen.

It deliberately preserves the vertical-list shape of the existing History Explorer so muscle memory carries over from the current build. What changes is the *aesthetic*: paper background, serif entry titles, mono timestamp gutter, oxblood disc markers on a hairline rail, and the editorial date dividers requested in the brief.

## Where it lives

- **Route**: `/explorer` (the existing History Explorer route, now the application's default landing for returning users)
- **Sidebar entry**: `Timeline` (CORE section, first item)
- **Deep-link grammar** (preserved from current spec):
  - `?q=<query>` — text search
  - `?profileId=<id>` — restrict to one browser profile
  - `?browserKind=<chrome|edge|firefox|safari|atlas|comet>` — restrict to a browser family
  - `?domain=<host>` — restrict to one domain
  - `?start=YYYY-MM-DD&end=YYYY-MM-DD` — explicit window
  - `?view=time|session|trail` — grouping mode (default `time`)
  - `?regex=1` — interpret `q` as regex
  - `?page=`, `?pageSize=`, `?sort=`

## Layout

Three columns plus a status bar.

### Left rail (240 px) — sidebar

Top-down:
- **Brand wordmark** — `PathKeep` in serif 28px, oxblood. Italic-serif subtitle `Editorial Atlas` directly below in ink-muted.
- **Primary navigation** — `Timeline · Search · Sources · Collections · Intelligence · Settings`. Each row is a single line of sans 14px with a 20×20 lucide outline icon. Active row has a 2-px oxblood bar on the left edge and oxblood ink for the label and icon.
- **Browsing Intensity card** — title in micro-mono uppercase. Day/Week/Month/Year segmented control in sans 12px. Below, a 7-column × 24-row heatmap (one column per weekday, rows in 4 bands: 12 AM / 6 AM / 12 PM / 6 PM). Cells use the heatmap-0…heatmap-4 ramp. Caption in italic serif 12px ink-muted: *"Darker tones represent more browsing activity."*
- **Local First card** — small oxblood disc icon, label `Local First` in sans 14px, body in sans 13px ink-secondary: *"All data is stored on this device and never leaves your control."* A `Learn more` link in oxblood.

### Topbar (56 px)

Left-to-right:
- Inline search input — pill-radius, 280 px wide, sans 14px placeholder `Search your history…`, mono `⌘K` hint right-aligned in ink-faint.
- Date-range chip `May 18 – May 24, 2026` with calendar icon.
- `All Sources` dropdown · `All Types` dropdown · filter icon.
- Right cluster: share / message / close icons in ink-muted.

### Center column — the timeline

A two-track layout:
- **Left timestamp gutter** (~112 px wide) — mono 12px, ink-muted, right-aligned.
- **Right entry track** — full-width entries.

A single 1-px hairline runs vertically between the two tracks. At each entry's vertical center the hairline is interrupted by a 6-px oxblood disc.

**Date dividers** introduce each day:
- Format: `Today · May 24, 2026` — `Today` in italic serif oxblood 15px, `· May 24, 2026` in sans 13px ink-muted.
- Followed by a thin oxblood disc on the rail and a hairline that extends to the right margin.
- For non-today days, the relative label is `Yesterday`, `Monday`, `Last Tuesday`, then absolute date.

**Entry anatomy** (top-to-bottom inside the entry block):
1. **Source row** — 28-px favicon tile (4-px radius, 1-px inner hairline) + domain `aeon.co` in mono 12px ink-muted. Right-end: type tag (`Article`, `Repository`, `Search`, `Video`, `Doc`) as a hairline pill with sans 12px label. To the right of that, a bookmark icon (filled oxblood when saved, outline ink-muted when not).
2. **Title** — serif 17px weight 500, ink-primary, text-pretty. Up to two lines.
3. **Body** — sans 14px, ink-secondary, two-line clamp. This is the page summary or a captured excerpt.
4. **Optional inline block** — either:
   - `Note` block: small pencil icon + label `Note` in sans 12px oxblood, then sans 14px ink-secondary (e.g. *"Important read for my writing on focus."*)
   - `Saved Snippet` block: small scissors icon + label `Saved Snippet` in sans 12px oxblood, then serif italic 14px ink-secondary in quotation marks.

Each entry has 24 px of vertical breathing room above and below it. There is no card border around individual entries; the rail and timestamp gutter do all the structural work.

### Right detail panel (360 px)

For the currently selected entry:
- **Header** — large favicon tile + domain in mono 13px + URL on a second line as an oxblood link with external-arrow.
- **Summary** — serif 14px body paragraph, ink-secondary, generous line-height.
- **Metadata** — 2-column key/value list, sans 13px ink-muted on the left, mono 13px ink-primary on the right. Keys: `First Visited / Last Visited / Visit Count / Time Spent / Source / Page Type`.
- **Tags** — chip row. Active chips are outline pills in sans 12px. A `+` chip ends the row.
- **Connections** — `In Collection: <name>` (oxblood link) and `Related: 3 pages` (oxblood link).
- **Local-First Status** — small card with green `✓` and copy: *"This page and all associated data are stored securely on this device."* `View Data Directory ↗` link in oxblood.
- **Visit History** — micro mono label `1` (max), thin oxblood vertical bars across an x-axis spanning the last month. No grid, no fill, no axis line — just the bars and four month tick labels (`Apr 24 / May 4 / May 14 / May 24`).

### Status bar (32 px)

Mono 12px, ink-muted: `7,842 pages · 1.3 GB · Synced never (local only)` left, `Indexed on May 24, 2026 at 6:03 AM ✓` right. Tick is success-ink.

## Components used

From `DESIGN.md`:
- `timeline-rail` · `date-divider` · `history-entry` · `chip` · `callout-localfirst` · `statusbar`
- All paper / ink / oxblood / hairline / heatmap tokens
- Serif `entry` style for titles, sans `body`/`label` for chrome, mono `meta`/`micro` for evidence

## Content shown in the mockup

The week is `May 18 – May 24, 2026`. Today's entries (top-down):
- 9:42 AM · `aeon.co` · `Article` · *The Case for Slow Productivity* · note: *"Important read for my writing on focus."*
- 8:17 AM · `nesslabs.com` · `Article` · saved · *Building a Second Brain, Step by Step* · saved snippet: *"Your second brain is not about storing everything, it's about building trusted pathways."*
- 7:34 AM · `github.com` · `Repository` · *microsoft/markitdown* · note: *"Add to automation toolkit."*

Yesterday (May 23):
- 10:11 PM · `stratechery.com` · `Article` · saved · *The Platform Trap* · saved snippet
- 6:48 PM · `noahpinion.substack.com` · `Article` · *Goodhart's Law in the Wild* · note: *"Connect to article I'm writing on metrics."*

The detail panel shows the `aeon.co` entry as selected.

## States not covered by this image

- Empty state ("Nothing here yet. Memory is patient.")
- Loading state (entry skeletons matching the entry anatomy, 5–8 rows)
- Filtered-to-zero state with a `Reset filters` action
- Long-title wrapping (verifies text-pretty / text-balance behavior)
- Right panel collapsed (window narrower than 1280 px)
- Multi-select / range select for export

## Image generation prompt

> A high-fidelity desktop application screenshot mockup, in the style of an editorial library or archive app — *not* a developer tool. The application is "PathKeep — Editorial Atlas", a local-first browsing-history archive. Three-column layout on a warm unbleached paper background (#FAF7F2) with a barely-perceptible paper grain texture. Three small macOS traffic-light dots in the very top-left corner.
>
> LEFT SIDEBAR (240 px): wordmark "PathKeep" in deep oxblood (#7B1F2A) Newsreader-serif 28px at top, italic serif subtitle "Editorial Atlas" beneath in warm grey. Vertical nav with 1.5px-stroke line icons: Timeline (active, with 2px oxblood bar on left, oxblood label), Search, Sources, Collections, Intelligence, Settings. Below: a "Browsing Intensity" card with a Day/Week/Month/Year segmented control and a 7-column heatmap grid using oxblood-tinted cells from cream through deep wine, labelled M T W T F S S across top and 12 AM / 6 AM / 12 PM / 6 PM down the side; tiny italic-serif caption underneath. Bottom: a "Local First" hairline card with a small oxblood disc icon, body copy and a "Learn more" oxblood link.
>
> CENTER COLUMN: top toolbar with a pill-shaped search input ("Search your history…" with mono "⌘K" hint), a date-range chip "May 18 – May 24, 2026" with calendar icon, "All Sources" and "All Types" dropdown pills, a filter icon, and three quiet utility icons on the far right. Below: editorial date divider "Today · May 24, 2026" — "Today" in italic serif oxblood, "May 24, 2026" in sans grey, with a thin hairline running right and a small oxblood disc. Then a vertical timeline with a left-side mono-typeface timestamp gutter (9:42 AM, 8:17 AM, 7:34 AM…), thin hairline rail with 6px oxblood disc markers at each entry's center. Each entry: 28px rounded-square brand favicon tile + domain in mono grey, a "Article" / "Repository" hairline pill tag at right, a small bookmark icon (filled oxblood when saved). Title in Newsreader serif 17px ink black ("The Case for Slow Productivity", "Building a Second Brain, Step by Step", "microsoft/markitdown"), 2-line sans grey body description, optional Note or Saved Snippet inline block in oxblood ink. Generous vertical breathing room. A second editorial divider "Yesterday · May 23, 2026" introducing further entries (Stratechery's "The Platform Trap" and Noahpinion's "Goodhart's Law in the Wild").
>
> RIGHT DETAIL PANEL (360 px): for the selected aeon.co entry — large brand tile + "aeon.co" + URL with external arrow link in oxblood. Sections: Summary (serif body paragraph), Metadata (key/value: First Visited, Last Visited, Visit Count, Time Spent 4m 28s, Source: Arc Browser, Page Type: Article), Tags row with hairline-pill chips (productivity, focus, writing, mindset, +), Connections (In Collection: Writing Research, Related: 3 pages), Local-First Status hairline card with green check ("Fully Local"), Visit History — small chart of thin oxblood vertical bars over an x-axis labelled Apr 24 / May 4 / May 14 / May 24.
>
> BOTTOM STATUS BAR: 32px high, mono 12px grey, "7,842 pages · 1.3 GB · Synced never (local only)" on the left, "Indexed on May 24, 2026 at 6:03 AM ✓" on the right with a small green tick.
>
> Typography: Newsreader serif for entry titles and brand wordmark; Inter sans for chrome, navigation and body; JetBrains Mono for timestamps, domains, URLs, byte counts. Color: warm paper #FAF7F2 background, darker leaf #F2EDE4 cards, ink-warm near-black text #1A1612, deep oxblood accent #7B1F2A used sparingly. No shadows — separation by hairlines (#1A1612 at 10% opacity) only. Square geometry, 4px radii, no rounded card chrome. No gradients, no glassmorphism, no neon. Editorial library aesthetic, contact-sheet metaphor, generous whitespace, low density. Light mode. Render at 1440×1024.
