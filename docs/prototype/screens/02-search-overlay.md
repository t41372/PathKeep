# 02 — Global Search Overlay (⌘K)

> Replaces the previous standalone Advanced Search page.
> Image: [`../images/02-search-overlay.jpg`](../images/02-search-overlay.jpg)

## Purpose

The previous design promoted a dedicated `Advanced Search` page to top-level navigation. This created three competing search surfaces (sidebar entry, basic search page, advanced search page) and made search feel like a heavy specialist mode.

The redesign collapses all of them into one **centered overlay reachable from anywhere via ⌘K**. The overlay is the *only* search surface. Recent queries, saved filters, the Advanced filter chips, and execution all live inside it. The Sidebar's `Search` entry is preserved as a discoverable affordance for users who never press ⌘K — but it opens the same overlay.

## Where it lives

- **Trigger**: `⌘K` (mac) / `Ctrl K` (other) anywhere in the app, or click the inline topbar search input, or click `Search` in the sidebar.
- **Closes via**: `Esc` · click outside · pressing `Enter` (which routes to `/explorer?q=…&…filters` and closes).
- **Submits to**: the Timeline route at `/explorer` with the resolved deep-link query string. The overlay never has its own URL.

## Layout

The overlay is centered on the viewport. The screen behind it is *not* dimmed to black; it is dimmed with a 30% paper-tinted overlay (`rgba(26, 22, 18, 0.30)` over the existing paper background) so the editorial atmosphere is preserved.

The overlay container is 640 px wide, vellum surface (`#FFFEFB`), 6 px radius, with the overlay shadow token. A 1-px hairline runs around it. There is no glow.

### Structure (top-to-bottom)

1. **Search input row** — full width, 56 px high.
   - 20 px lucide search icon left, ink-faint.
   - Text input: serif 18px ink-primary, placeholder `Search across 7,842 pages…` in ink-faint.
   - Right side: a small mono kbd showing `⌘K` and a close `Esc` chip.
   - A 1-px hairline below.

2. **Filter chip row** — sans 13px chips on a single horizontal line. Each chip is hairline by default, toggles to oxblood-tint when active.
   - `Date: This week ▾` (default)
   - `Source: All ▾`
   - `Type: All ▾`
   - `Domain: Any ▾`
   - `Has note` (toggle)
   - `Has snippet` (toggle)
   - `Regex` (toggle)
   - Trailing `+ More` reveals the rest of the advanced fields in an inline disclosure (matches `Advanced` in the brief — this is the "secondary menu after clicking the global search box" the user requested).

3. **Sectioned result list** — sans/serif mix. Sections appear in order:

   **Recent searches** (3–5 rows). Each row: a clock icon, the query text in serif 15px, mono `2 days ago` on the right.

   **Suggested filters** (2–3 chip rows). Smart filters offered based on what the user has been viewing — e.g., `Pages on focus from aeon.co`, `Articles you saved this month`. Click applies the filter and runs the search.

   **Live results** — appears after the user types. Each row is a compressed history-entry: 20 px favicon · domain · serif 15px title · mono date on the right. Up to 6 rows. Below: `See all 142 results in Timeline →` (oxblood link).

   **Commands** (always shown if the input is empty). Three quiet entries:
   - `Open Today's Timeline` (g then t)
   - `Jump to date…`
   - `Export search results` (disabled until results exist)

4. **Footer hint row** — mono 12px ink-muted: `↑ ↓ to navigate · ↵ to open · ⌘↵ to open in detail · Esc to close`.

## Components used

From `DESIGN.md`:
- `bg-vellum` for the overlay surface, `bg-paper` dimmed at 30 % behind
- `chip` (filter chips) · `pill` radius
- `meta` mono for kbds and timestamps, `entry` serif for result titles
- `overlay` shadow (the only shadow token allowed at this elevation)

## Content shown in the mockup

- The user has typed `productivity` in the input.
- Filter chips: `Date: This week ▾` and `Source: All ▾` shown as hairline; `Has note` shown as active oxblood.
- Recent searches: `slow productivity`, `building a second brain`, `goodharts law`.
- Live results: 4 entries from aeon.co, nesslabs.com, stratechery.com, ribbonfarm.com.
- Footer keyboard hints visible.

## States not covered by this image

- Empty input + no recent queries (first-launch overlay)
- Zero results state (`Nothing matches. Try widening the date range.`)
- Regex error state (chip turns warning-ink with inline message)
- Long query wrapping
- Compact density variant

## Image generation prompt

> A high-fidelity desktop application screenshot mockup of a centered command-palette / search overlay. The aesthetic is editorial library / archive — warm paper, oxblood accent — *not* a typical Raycast / Linear command palette. The overlay floats over a slightly dimmed but still visible PathKeep timeline view; the warm paper background and timeline entries remain readable beneath a 30% paper-tinted veil so the atmosphere stays editorial.
>
> The overlay container is 640 px wide, centered, sitting on a vellum-cream surface (#FFFEFB) with a 6 px radius and a soft natural shadow. A 1-px hairline border runs around it.
>
> TOP: a 56 px search input row. A small lucide search icon on the left in light grey, then a serif (Newsreader) 18px input field with the user's typed query "productivity" in ink-warm black, with a thin caret. Right side: a small "⌘K" kbd in mono on a hairline pill, and an "Esc" hint chip. A thin hairline divider below.
>
> NEXT: a single horizontal row of filter chips in sans 13px, hairline-bordered pills. Chips read: "Date: This week ▾", "Source: All ▾", "Type: All ▾", "Domain: Any ▾", a toggle pill "Has note" shown active in oxblood-tint background with oxblood ink, "Has snippet", "Regex", trailing "+ More" disclosure chip.
>
> BELOW: sectioned results.
> Section header "Recent searches" in micro-mono uppercase grey. Three rows: clock icon · "slow productivity" · "2 days ago" on the right in mono; "building a second brain" · "5 days ago"; "goodharts law" · "1 week ago".
> Section header "Suggested filters" — two chip-row suggestions: "Pages on focus from aeon.co" and "Articles you saved this month".
> Section header "Results · 4 of 142". Four compressed history entries, each: 20 px favicon tile + domain in mono grey + serif 15px title in ink-black + mono date on right. Titles: "The Case for Slow Productivity (aeon.co)", "Building a Second Brain, Step by Step (nesslabs.com)", "The Platform Trap (stratechery.com)", "Notes on Productivity Theatre (ribbonfarm.com)". Below the list a small oxblood link "See all 142 results in Timeline →".
>
> Section header "Commands". Three quiet rows with small icons: "Open Today's Timeline", "Jump to date…", "Export search results" (last one greyed/disabled).
>
> FOOTER: a slim 32 px row with mono 12px grey hints: "↑ ↓ to navigate · ↵ to open · ⌘↵ to open in detail · Esc to close".
>
> Background: blurred / paper-veiled view of the timeline behind it — warm paper #FAF7F2, hairline rail, oxblood disc markers, serif entry titles barely legible. Paper grain texture across both layers.
>
> Typography: Newsreader serif for the input value and result titles; Inter sans for chip labels, section headers, navigation hints; JetBrains Mono for timestamps, kbd hints, byte counts. Color palette: vellum #FFFEFB overlay, warm paper #FAF7F2 background, ink-black #1A1612, oxblood #7B1F2A accent (active chip, "More" link, brand). No gradients, no glassmorphism, no neon. Render at 1440×1024.
