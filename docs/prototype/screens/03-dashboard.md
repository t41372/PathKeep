# 03 — Dashboard

> Landing surface for returning users. The only screen that mixes *archive operational status* with *intelligence at a glance*.
> Image: [`../images/03-dashboard.jpg`](../images/03-dashboard.jpg)

## Purpose

The Dashboard answers two questions in one view:
1. *Is my archive healthy and up-to-date?* — last backup, total pages, storage, latest run summary, anything that needs attention.
2. *What did I read recently and on this day in past years?* — last week's reading, On This Day re-encounters, browsing rhythm calendar for the current year.

It is the *commonplace book* page — a single spread that sets the tone of the whole archive before the user dives into the Timeline.

## Where it lives

- **Route**: `/dashboard`
- **Sidebar entry**: not surfaced in the condensed prototype rail, but accessible via topbar `Backup now` ↗ archive panel and via ⌘K `Open Dashboard`.
- **First-launch behavior**: after Onboarding, the user is dropped here once. After that, returning users land on Timeline by default. (The setting is configurable in Settings.)

## Layout

Three columns plus status bar, mirroring the rest of the shell. The center column is a 2-column grid of cards.

### Left rail (240 px)

Same sidebar as Timeline. `Dashboard` would be active if surfaced; in the prototype mockup the active rail item is `Timeline` and the Dashboard is opened via topbar / command. (Easy to swap in implementation.)

### Topbar (56 px)

- Left: page title `Dashboard` in serif 18px.
- Center: scope switcher chip — `Profile: Daily Driver ▾` (shared profile scope).
- Right: notification bell, scope switcher, `Backup now` primary CTA in oxblood.

### Center column — the dashboard cards

A 2-column grid with 24 px gutter. Cards have hairline borders only — no shadows, no fills beyond bg-leaf for callout cards.

Reading order (top-to-bottom, left-to-right):

1. **Archive Status** (full-width row spanning both columns)
   - Three side-by-side mini-stats with hairline dividers:
     - `Total pages` — serif 28px `7,842` + sans 12px ink-muted `across 4 years`
     - `Archive size` — `1.3 GB` + `local · encrypted`
     - `Last archived` — `Today · 6:03 AM` + green tick + `1,847 entries`
   - Below: a single sentence in serif italic 14px ink-secondary: *"PathKeep has captured 7,842 pages across 4 years. They are yours."*

2. **On This Day** (left column, tall card)
   - Title: serif 18px `On This Day · May 24`.
   - 2×3 mini contact-sheet of past pages on the same calendar day across years. Each cell: a 4:3 thumbnail with inner hairline + faint vignette, mono frame number `034 / 412` underneath, serif 13px title clamped to 2 lines, mono 11px year label `2024` in the corner.
   - Footer link: `See all 18 pages from this day →` in oxblood.

3. **Browsing Rhythm — 2026** (right column, tall card)
   - Title: serif 18px `Browsing Rhythm · 2026`.
   - Year pager: `‹ Back to current year` left of `‹ 2025  2026  2027 ›` (only previous years with data are enabled; "back to current" is always present).
   - Real-date calendar heatmap — 53 columns (weeks) × 7 rows (Sun–Sat). Cells use heatmap-0…heatmap-4 ramp. Empty days show heatmap-0. Month labels (`Jan / Feb / Mar / …`) above in mono 11px ink-muted. Hovering a cell would open the Browsing Rhythm preview (out of scope for this still mockup — described in `04-intelligence.md`).
   - Caption in italic serif 12px: *"From Jan 2 — May 24, 2026 · 1,124 active days."*

4. **Recent Runs** (left column, mid card)
   - Title: serif 18px `Recent Runs`.
   - Compact 4-row table: timestamp (mono 12px) · source (`Chrome · Default`) · `+216 entries` · status tick.
   - Footer link: `Open Audit Ledger →` (oxblood).

5. **Storage** (right column, mid card)
   - Title: serif 18px `Storage`.
   - Two horizontal proportion bars labelled `Core history (1.04 GB)` and `Other data (260 MB)`. Bars use heatmap-3 / heatmap-2 fills.
   - Three small key/value rows: `Archive root`, `Audit repo`, `App version` — each with a mono path on the right and a small `Open ↗` action.

6. **Background Work** (full-width row)
   - A single quiet row: `2 jobs queued · 1 derived rebuild scheduled · No errors.` Inline action `Open Jobs →`.
   - This is the *only* surfaced AI / job indicator on Dashboard. Detail belongs to `/jobs`.

### Right detail panel (360 px)

Used here as a sticky **today digest**:
- Header: serif 24px `Today · May 24, 2026`.
- Mini paragraph: *"You spent 4h 12m across 23 pages today, mostly on long-form reading."*
- Top sites mini-list (4 rows): favicon · domain · mono visit count.
- Top tags chips: `productivity · focus · writing · ai-research`.
- Recent notes (3 rows): pencil icon · note text in serif italic 14px clamped to 2 lines · mono time on the right.

### Status bar

Identical to Timeline.

## Components used

- All cards: hairline border on `bg-leaf` over `bg-paper`.
- Numbers in stats: serif display weight 600.
- `contact-sheet-thumbnail` for On This Day cells.
- Calendar heatmap from `DESIGN.md`'s heatmap palette.

## Content shown in the mockup

- Archive Status: `7,842 pages / 1.3 GB / Today · 6:03 AM ✓ / 1,847 entries`.
- On This Day: 6 cells from `aeon.co`, `wikipedia.org`, `nytimes.com`, `craigmod.com`, `lwn.net`, `ribbonfarm.com`, with year labels `2025 / 2024 / 2024 / 2023 / 2022 / 2022`.
- Browsing Rhythm: 2026 calendar, busiest weeks in March and early May.
- Recent Runs: 4 rows.
- Today digest: 23 pages, 4h 12m.

## States not covered by this image

- Zero-archive state ("Connect a browser to begin." — opens Onboarding)
- Single-year archives (Browsing Rhythm has no pager)
- Failed-run state (warning-ink callout above Archive Status)
- Schedule-needs-attention state
- Privacy mode (numbers replaced with `••`)

## Image generation prompt

> Desktop application dashboard screenshot mockup, editorial library aesthetic. Warm unbleached paper background (#FAF7F2) with extremely subtle paper grain. Three macOS traffic-light dots top-left.
>
> LEFT SIDEBAR (240 px) — same as PathKeep timeline: oxblood "PathKeep" wordmark in Newsreader serif at top, italic-serif "Editorial Atlas" subtitle, vertical nav (Timeline, Search, Sources, Collections, Intelligence, Settings — Timeline shown active with 2px oxblood indent bar), "Browsing Intensity" mini heatmap card, and a "Local First" hairline card at the bottom.
>
> CENTER COLUMN — page titled "Dashboard" in serif 18px at the top-left of a topbar, with a "Profile: Daily Driver ▾" chip mid-toolbar and a primary oxblood "Backup now" button on the right with a notification bell.
>
> Below, a 2-column grid of cards on warm leaf-cream backgrounds (#F2EDE4) with hairline borders only — no shadows.
>
> ROW 1 (full width): "Archive Status" card. Three side-by-side stats divided by hairlines: "7,842" (serif 28px) labelled "Total pages · across 4 years" in mono grey; "1.3 GB" labelled "Archive size · local · encrypted"; "Today · 6:03 AM ✓" labelled "Last archived · 1,847 entries" with a small green tick. Below the stats, a single italic-serif sentence: "PathKeep has captured 7,842 pages across 4 years. They are yours."
>
> ROW 2 LEFT: tall "On This Day · May 24" card. 2×3 mini contact-sheet grid of webpage thumbnails — each thumbnail is a small 4:3 captured-page image with a hairline inner stroke and faint vignette, a mono frame number "034 / 412" underneath, a serif 2-line title, and a mono year label ("2024", "2023", "2022") in the corner. Page titles include "On Disquiet (aeon.co)", "Why we sleep (wikipedia.org)", "The Pace Layer Essay (craigmod.com)". Footer oxblood link: "See all 18 pages from this day →".
>
> ROW 2 RIGHT: tall "Browsing Rhythm · 2026" card. A real-date calendar heatmap — 53 columns × 7 rows of small squares using a cream-to-oxblood ramp, with month labels (Jan, Feb, Mar, … through Dec) in mono grey across the top. Year pager arrows above the heatmap: "‹ Back to current year" and arrows around "‹ 2025  2026  2027 ›". Italic serif caption underneath: "From Jan 2 — May 24, 2026 · 1,124 active days."
>
> ROW 3 LEFT: "Recent Runs" card. A compact 4-row table — each row has a mono timestamp, a source label like "Chrome · Default" / "Firefox · Personal" / "Safari", a "+216 entries" delta, and a green tick. Footer oxblood link "Open Audit Ledger →".
>
> ROW 3 RIGHT: "Storage" card. Two horizontal proportion bars: "Core history · 1.04 GB" filled with a deep oxblood-tinted bar; "Other data · 260 MB" with a paler oxblood-tint. Below: three small key/value rows with mono right-aligned paths labelled "Archive root", "Audit repo", "App version", each with a small "Open ↗" action.
>
> ROW 4 (full width): "Background Work" — a single low-key row: "2 jobs queued · 1 derived rebuild scheduled · No errors." with an oxblood "Open Jobs →" link on the right.
>
> RIGHT DETAIL PANEL (360 px): "Today · May 24, 2026" in serif 24px. Italic-serif paragraph "You spent 4h 12m across 23 pages today, mostly on long-form reading." A "Top sites" mini list with 4 rows (favicon · domain · mono visit count). A "Top tags" chip row (productivity, focus, writing, ai-research). A "Recent notes" list of 3 italic-serif notes with a pencil icon and mono time.
>
> BOTTOM STATUS BAR: 32px high, mono 12px grey, "7,842 pages · 1.3 GB · Synced never (local only)" left, "Indexed on May 24, 2026 at 6:03 AM ✓" right.
>
> Typography: Newsreader serif for titles and stat numbers; Inter sans for chrome and body; JetBrains Mono for timestamps, paths, byte counts. Single oxblood accent #7B1F2A used sparingly. No gradients, no shadows beyond the cards' hairline borders, no neon, no glassmorphism. Editorial library aesthetic. Light mode. Render at 1440×1024.
