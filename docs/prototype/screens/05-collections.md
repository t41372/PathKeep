# Screen 05 — Collections

> A reading-room of curated stacks. Index cards laid out on cream paper, each holding a small constellation of saved pages.

---

## Purpose

Collections are user-curated groupings of pages — research projects, reading lists, reference shelves. This screen is the index of all collections (the "shelf" view).

---

## Layout

Three columns, same chrome as Timeline:

- **Left rail (240px)** — primary nav (`Collections` active), with a `+ New Collection` ghost button below the nav, and an "All Collections (12)" / "Pinned (3)" / "Archived (2)" sub-list.
- **Main canvas** — header `Collections` (display serif, 32px) + sub-line `12 collections · 184 pages`. Below: filter chips `All · Pinned · Recent · Archived` and a small grid/list toggle on the right.
- **Right rail (320px)** — collapsed by default into a thin `Quick Stats` card; or hidden entirely. (No selection until a card is clicked.)

---

## The grid

A **3-column responsive grid** of "card-stacks" (≈ 280×200px each, gap 24px). Each card is a flat ivory rectangle (#F4EFE7), 1px hairline border, 6px corner radius — like an index card pulled from a library catalogue.

### Each collection card

```
┌────────────────────────────────┐
│ Writing Research          ⌘    │  ← title (serif 18px) + pin icon
│                                │
│ Sources for the slow-          │  ← description (sans 13px, muted)
│ productivity essay             │
│                                │
│ ──────────────────────────     │  ← hairline divider
│                                │
│   24 pages · 3 sources         │  ← meta (mono 11px)
│   Updated yesterday            │
│                                │
│   [a][n][s][+]                 │  ← favicon stack (4 visible)
└────────────────────────────────┘
```

- **Pinned** collections show a small filled deep-claret pin glyph in the top-right.
- **Hover** tilts the card +0.5° and lifts shadow to `0 4px 16px rgba(60,30,20,0.06)`.
- **Cover preview** — first card per collection optionally shows a 4-square mini-thumbnail of the latest 4 page favicons in the top-left corner.

### Featured row (top)

Above the grid, a single **wide hero card** (full row, 640×140) for "Recently Active" — shows the most-touched collection in landscape format with a horizontal strip of 8 page thumbnails.

---

## Sample collections (for the prototype)

1. **Writing Research** — 24 pages, pinned, updated yesterday
2. **Second Brain** — 41 pages, pinned, updated 2 days ago
3. **AI Reading List** — 18 pages, pinned, updated last week
4. **Design References** — 32 pages
5. **Climate & Systems** — 14 pages
6. **Career Notes** — 9 pages
7. **Recipes & Cooking** — 22 pages
8. **Travel Plans 2026** — 11 pages
9. **Philosophy** — 7 pages
10. **Finance & Investing** — 6 pages, archived (lower opacity)

---

## Typography

- Title — `font-serif`, 18px, weight 500, tracking -0.01em
- Description — `font-sans`, 13px, weight 400, leading-relaxed, color `--text-soft`
- Meta — `font-mono`, 11px, uppercase, color `--text-faint`, tracking +0.04em

---

## Colour & texture

- Card surface a half-shade darker than canvas (`--surface-card` #F4EFE7) to sit on the cream like card stock.
- Pin glyphs and active-collection accents in `--accent-claret`.
- Each card's right edge has a 2px-wide vertical "spine" tint pulled from the dominant colour of its top page (a subtle library-shelf cue) — desaturated to ~30% so it never overpowers the paper.

---

## Empty state

If a user has zero collections, the canvas shows a centered illustration plate — a single quill resting on a folded sheet of paper, hand-drawn line art in claret on cream — and the line: *"A collection is a question waiting for its answers."* Below it: a single `Create your first collection` button.

---

## Interactions

- **Click card** → opens that collection's detail view (its own timeline of pages).
- **Right-click** → context menu: Rename, Pin, Archive, Export, Delete.
- **Drag** card onto another card → merge collections (with confirm).
- **`N`** keyboard shortcut → new collection.

---

## Why this works

Index cards are the iconic vocabulary of pre-digital research. By making collections feel like physical cards on a desk, PathKeep frames the user as a scholar with a working library — not a hoarder of bookmarks.
