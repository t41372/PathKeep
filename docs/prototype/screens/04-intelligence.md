# 04 — Intelligence

> The analytical surface. Calendar heatmap at full fidelity, top sites, search activity, recap.
> Image: [`../images/04-intelligence.jpg`](../images/04-intelligence.jpg)

## Purpose

Intelligence is where the user steps back from individual entries and looks at the *shape* of their browsing across time. It must answer:

- *What does my year look like in concentration and rhythm?*
- *Which domains do I actually return to?*
- *What have I been searching for, and where?*
- *What stable concepts and topics show up across months?*

This is where the contact-sheet metaphor reaches its full expression. The page is read as a *bound special edition* of the archive: a year's contact sheet across the top, supporting tables and small charts below.

The visual contract is documented in `docs/design/intelligence-ui-redesign-brief.md` and the various tradeoffs in `docs/design/intelligence-*-tradeoff.md`. This screen *reskins* that information architecture in the editorial language; it does not redesign the IA.

## Where it lives

- **Route**: `/intelligence`
- **Sidebar entry**: `Intelligence` (CORE section)
- **Time scope**: top-level segmented control `Today / Week / Month / Year / All time / Custom range`. Default `Year`.
- **Insight Access strip**: a compact strip at the top of the page allowing direct entry to `/intelligence/day/:date` and `/intelligence/domain/:domain`. Takes one row, never expands into a feature page.

## Layout

Same shell. The center column is a single-column scrollable list of analytical cards.

### Topbar

- Title: serif 24px `Intelligence`.
- Beneath the title, a single-line `Insight Access` strip in sans 13px:
  `Insight Access · Open day [calendar icon] · Open domain [host input chip]`
- Right-side: `Range: 2026 ▾` chip + scope switcher + `Backup now`.

### Center column — analytical cards

Reading order is enforced by the brief: **analysis snapshot → time-period overview → browsing rhythm → spotlight → research signals → evidence / health → runtime digest**. Only the first three may take full main-column width; the rest live in half-width rows or a secondary grid.

1. **Analysis Snapshot** (full-width)
   - Serif 24px title `Analysis Snapshot · 2026 so far`.
   - Three stat clusters in a row: `1,124 active days`, `7,842 pages archived`, `412 unique domains`.
   - Sentence in serif italic 15px ink-secondary summarising the period: *"Your most-read week was March 9. Your most stable source over the year is craigmod.com (read on 47 distinct days)."*

2. **Time Period Overview** (full-width)
   - Serif title `2026 at a glance`.
   - Compact mixed chart: stacked horizontal bar chart of *type* breakdown (Article / Repository / Search / Doc / Video / Other) — bars in heatmap-2 / heatmap-3 / heatmap-4 / hairline grey for the long tail.
   - Right side: a small 4-cell stat block — *avg pages/day*, *median time on page*, *busiest weekday*, *quietest week*.

3. **Browsing Rhythm — calendar heatmap** (full-width, the centerpiece)
   - Serif title `Browsing Rhythm`.
   - Real-date heatmap, 53 cols × 7 rows, identical to Dashboard but larger (cells 14 px). Month labels along the top.
   - **Preview-first interaction** (described by `screens-and-nav.md` and the `intelligence-rhythm-calendar-heatmap-tradeoff.md`):
     - Click a day → expand a *preview drawer* directly below the heatmap with: that day's summary line, a 24-hour distribution strip, top sites of the day, proportion bar of activity composition, and a `View full day insights →` CTA that navigates to `/intelligence/day/:date`.
     - The mockup shows the drawer expanded for `May 12`.

4. **Top Sites** (half-width)
   - Title `Top Sites · 2026`.
   - Bounded list of 8 rows. Each: 28 px favicon · domain in mono 13px · serif 14px host title · mono visit count + mono active-days count + mono total time. Right end: an oxblood `Open ↗` chip routing to `/intelligence/domain/:domain`.

5. **Search Activity** (half-width)
   - Title `Search Activity · 2026`.
   - Tabbed sub-area: `Engines / Concepts / Search keywords / Families`. The mockup shows `Concepts`.
   - `Concepts` is a horizontal bar chart (per the brief — *not* a word cloud). 8 bars, sans 13px concept labels on the left, mono counts on the right.

6. **Stable Sources** (half-width)
   - Sources active across many distinct days. List of 6 rows with a small bar showing days-active-out-of-total.

7. **Refind** (half-width)
   - Pages the user has returned to multiple times. List of 6 rows.

8. **Research Signals · Evidence & Health · Runtime Digest** (compact stacked row)
   - A single full-width row of three small cards. Each is a hairline card with a serif title, two mono stats, and a single CTA to its dedicated route.

### Right detail panel

For Intelligence, the panel becomes a **scope summary**:
- Active scope: `Year · 2026`.
- Profile scope: `Daily Driver`.
- Last computed: mono timestamp + small `Open Jobs ↗`.
- A compact freshness / evidence chip strip explaining which deterministic modules are stale, which are current.

### Status bar

Same as the rest of the shell.

## Components used

- All cards on `bg-leaf` with hairline borders.
- Heatmap palette for the calendar.
- Horizontal bar charts use heatmap-3 fill on a hairline-grey track.
- The scope-summary right panel uses the small evidence-chip primitive shared with `/intelligence/day/:date` and `/intelligence/domain/:domain`.

## Content shown in the mockup

- Range: `2026` so far.
- Snapshot: `1,124 active days · 7,842 pages · 412 domains`.
- Calendar heatmap with `May 12` highlighted and the preview drawer open.
- Top Sites: craigmod.com, aeon.co, github.com, wikipedia.org, lwn.net, stratechery.com, ribbonfarm.com, news.ycombinator.com.
- Concepts: `productivity`, `note-taking`, `linux kernel`, `mathematics`, `philosophy of mind`, `architecture`, `local-first software`, `interface history`.

## States not covered by this image

- Empty / first-week archive (large empty heatmap with onboarding hint)
- Stale / disabled deterministic modules (freshness badges in warning-ink)
- Custom date range selection
- All time scope (heatmap collapses to the year-by-year strip described in `intelligence-rhythm-calendar-heatmap-tradeoff.md`)
- Day-detail (`/intelligence/day/:date`) — separate sketch deferred

## Image generation prompt

> Desktop application screenshot mockup of an analytical "Intelligence" page. Editorial library aesthetic — warm unbleached paper #FAF7F2 background, faint paper grain, three macOS traffic-light dots top-left, deep oxblood (#7B1F2A) accents.
>
> LEFT SIDEBAR (240 px): same as the PathKeep shell — "PathKeep" oxblood Newsreader-serif wordmark, "Editorial Atlas" italic-serif subtitle, vertical nav with line icons (Timeline, Search, Sources, Collections, Intelligence — *active here*, with a 2px oxblood indent bar and oxblood label, Settings), "Browsing Intensity" mini heatmap, "Local First" card.
>
> CENTER COLUMN: top a serif 24px page title "Intelligence". Below it a single quiet line "Insight Access · Open day [calendar icon] · Open domain [host input chip]". Top-right of the toolbar: a "Range: 2026 ▾" chip, a "Profile: Daily Driver ▾" chip, an oxblood "Backup now" button.
>
> Below, a stack of analytical cards (hairline borders only, leaf-cream #F2EDE4 background, no shadows):
>
> CARD 1 (full width) "Analysis Snapshot · 2026 so far": three big serif stats side-by-side — "1,124" labelled "active days", "7,842" labelled "pages archived", "412" labelled "unique domains" — separated by hairlines. Below: italic serif sentence "Your most-read week was March 9. Your most stable source over the year is craigmod.com (read on 47 distinct days)."
>
> CARD 2 (full width) "2026 at a glance": stacked horizontal bar showing type breakdown (Article / Repository / Search / Doc / Video / Other) in oxblood-tinted gradient ramp; on the right, a 2×2 stat block listing avg pages/day, median time on page, busiest weekday (Tuesday), quietest week.
>
> CARD 3 (full width, the centerpiece) "Browsing Rhythm": a large real-date calendar heatmap — 53 columns × 7 rows of 14-px squares, ramp from cream (#F2EDE4) through tan to deep oxblood, month labels Jan–Dec in mono grey across the top. One specific cell labelled May 12 is highlighted with a thin oxblood square outline. Directly below the heatmap, an *expanded preview drawer* on the same card showing: "May 12, 2026 · 47 pages · 5h 22m" in serif; a thin 24-hour activity strip showing oxblood-tinted bars across the day; a "Top sites" mini-row (3 favicons + domains + counts); a horizontal proportion bar showing activity composition; an oxblood "View full day insights →" CTA on the right.
>
> CARD 4 (half width left) "Top Sites · 2026": 8 rows — favicon tile + domain in mono + serif site label + mono visit count + mono active days + mono total time. Domains: craigmod.com, aeon.co, github.com, wikipedia.org, lwn.net, stratechery.com, ribbonfarm.com, news.ycombinator.com. Right edge of each row: a small oxblood "Open ↗" chip.
>
> CARD 5 (half width right) "Search Activity · 2026" with sub-tabs "Engines / Concepts / Search keywords / Families" (Concepts active). A horizontal bar chart of 8 concepts: productivity, note-taking, linux kernel, mathematics, philosophy of mind, architecture, local-first software, interface history. Bars filled in oxblood-tinted color, sans labels left, mono counts right.
>
> CARD 6 (half width left) "Stable Sources" — list of 6 sources with a small days-active bar.
> CARD 7 (half width right) "Refind" — 6 pages the user has revisited multiple times.
>
> CARD 8 (full width compact row) — three small cards inline: "Research Signals", "Evidence & Health", "Runtime Digest" — each with a serif title, two mono stats, and a small oxblood CTA on the right.
>
> RIGHT DETAIL PANEL (360 px): a "Scope" panel — "Year · 2026", "Profile · Daily Driver", "Last computed · 14:23", small "Open Jobs ↗" link. A compact freshness chip strip: 6 small chips listing deterministic module names with green ticks and one warning-ink chip.
>
> BOTTOM STATUS BAR: same as PathKeep shell.
>
> Typography: Newsreader serif for titles and big stats; Inter sans for chrome and body; JetBrains Mono for timestamps, counts, paths. Color palette: warm paper #FAF7F2 background, leaf #F2EDE4 cards, ink #1A1612, oxblood #7B1F2A accent, heatmap palette from cream through tan to deep wine. No gradients (heatmap ramps are stepped, not gradient-blended), no shadows, no neon. Editorial / archival aesthetic. Light mode. Render at 1440×1024.
