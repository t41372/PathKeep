# Screen 06 — Sources

> The atlas's gazetteer. A directory of every domain you've ever visited, ranked, weighted, and rendered as a calm tabular index.

---

## Purpose

Show the user the **shape of their reading diet** — which domains dominate their attention, how often each is visited, and how each domain has trended over time. Sources are the rivers; pages are the water.

---

## Layout

Three columns again — Sources nav item active in the left rail. Main canvas dominated by a single **scrollable table** of domains, with a right-rail detail panel for the selected source.

### Header (above table)

- Display title `Sources` (serif 32px) + sub-line `412 domains · 7,842 pages indexed`.
- Right-aligned controls: a small bar chart mini-graph (last 30 days of new-source discovery), a sort-by dropdown (`Most visited · Recently active · Alphabetical · Time spent`), and a search input scoped to domains.

---

## The table

Five columns, mono-spaced numerics, hairline row dividers, no fills:

| Domain | Pages | Last visit | Time spent | 30-day trend |
|---|---|---|---|---|

Each row is 56px tall.

### Per row

- **Domain** column: 16px favicon + domain name (`font-sans`, 14px, weight 500). Below it, a one-line description in `--text-faint` 12px (e.g. "Long-form essays on slow thinking").
- **Pages** — `font-mono`, 13px, right-aligned. e.g. `184`.
- **Last visit** — relative time (`2h ago`, `yesterday`, `Mar 14`).
- **Time spent** — total cumulative attention (e.g. `14h 22m`).
- **30-day trend** — a tiny inline sparkline (60×16px), claret stroke on cream, no fill.

### Sample rows (top of list)

1. `aeon.co` — 184 pages — 2h ago — 14h 22m — *(rising sparkline)*
2. `nesslabs.com` — 142 pages — yesterday — 11h 03m
3. `github.com` — 312 pages — 3h ago — 8h 41m
4. `stratechery.com` — 68 pages — yesterday — 9h 12m
5. `noahpinion.substack.com` — 54 pages — 4d ago — 6h 28m
6. `arxiv.org` — 41 pages — 1w ago — 5h 02m
7. `lesswrong.com` — 38 pages — 2d ago — 4h 47m
8. `news.ycombinator.com` — 287 pages — 1h ago — 7h 14m
9. `notion.so` — 91 pages — today — 12h 06m
10. `wikipedia.org` — 422 pages — today — 9h 33m

(plus more visible below the fold, faded)

### Hovering a row

Reveals a soft cream highlight (`--surface-hover`), and three icon-only actions appear at the right edge: `Open all in collection`, `Mute source`, `View details`.

---

## Right rail — Source detail

When a row is selected, the right rail shows:

- **Domain header** — large favicon (32px) + domain (serif 20px) + a "Visit site" external-link icon.
- **About** — a short description of the domain (when known via metadata).
- **Stats grid** — 2×2 of mono numerics:
  - Pages indexed
  - Total time
  - First visit (date)
  - Avg session
- **Top pages from this source** — list of 5 page titles with mini visit counts.
- **Visit cadence** — small bar chart, last 8 weeks, claret bars on cream.
- **Tags inferred** — chips showing dominant tags across pages from this source.
- **Actions** — `Pin source`, `Add all to collection…`, `Mute`.

---

## Typography & colour rules

- Tabular numerics always in `font-mono` so columns align without effort.
- Trend sparklines use a single 1.25px claret stroke; baseline implied, no axis.
- Row dividers are 1px `--border-hairline`. No alternating row fills — alignment carries the eye, not stripes.
- The selected row gets a 2px claret border-left (the same "wax-seal margin mark" used on the timeline today-row).

---

## Empty / new-user state

If fewer than 5 sources are tracked, the table is hidden and a placeholder card reads: *"Your atlas is just beginning to take shape. After about a week of browsing, this is where the geography of your reading appears."*

---

## Why this works

Sources = the domain-level abstraction users actually think about ("I read a lot of Aeon"). Surfacing it as a calm, tabular gazetteer turns vague intuition ("I think I spend too much time on HN") into legible, gentle data — without shaming the user with red bars or productivity scolds.
