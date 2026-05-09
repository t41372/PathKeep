# Screen 08 — Timeline (Dark / Reading Room)

> The same atlas, after dusk. Late-night reading on warm parchment under a low desk lamp — not the cold blue-grey of typical "dark modes."

---

## Purpose

Demonstrate the dark theme as a deliberate companion mode, not a tinted inversion. PathKeep's dark variant is a *low-light reading room*, not a "dark UI."

---

## Layout

Identical structure to Timeline (Light). What changes is **only colour and contrast** — type sizes, spacing, components, and the timeline rail are unchanged.

---

## Palette

Replace canvas tokens as follows:

```
--canvas-paper    → #1B1816   (deep walnut, almost black, never #000)
--canvas-soft     → #221E1B   (raised surface)
--surface-card    → #28231F
--text-deep       → #EFE7DA   (cream ink, never pure white)
--text-soft       → #B8AC9C
--text-faint      → #8A7E70
--border-hairline → rgba(239,231,218,0.08)
--accent-claret   → #C77A6E   (warmer, lifted claret so it glows)
--accent-claret-soft → #4A1F1B
```

The page background still has its very faint paper texture overlay — but at lower opacity (3–4%) and warmed toward sepia. Imagine an old leather-bound journal under tungsten light.

---

## Component shifts

### Sidebar

- Brand wordmark `PathKeep` now in cream (`--text-deep`).
- Active nav item: 2px claret left-border + cream label + faint claret tint behind label (4% opacity).
- Inactive nav: `--text-soft`.

### Browsing intensity heatmap

- Empty cells: `--canvas-soft`
- Filled cells: stepped from soft brown `#3A2C26` → warmest claret `#C77A6E`.
  The gradient feels like ember glow rather than data, beautiful even at a glance.

### Local-First card

- Border `--border-hairline` (translucent cream)
- Status dot: warm claret, 6px, with a 12px halo at 25% opacity for a candle-like glow.

### Timeline canvas

- The timeline rail line and the small hollow node circles are now cream (`--text-faint`); today's filled node is claret.
- Time-stamp gutter type is `--text-faint` mono.
- Each entry card sits on `--surface-card` (a couple shades up from canvas), with a 1px hairline (`--border-hairline`).
- Source favicons brighten slightly via a warm-tone overlay so they don't feel grey.
- Bookmark glyph (when active) is claret with a gentle inner glow.

### Right detail panel

- Same structure. The metadata table's labels are `--text-faint`, values are cream.
- The tag chips have transparent fills and a 1px claret border with claret text — they read as ember pills.
- The visit-history chart bars use a vertical claret-to-warm-amber gradient (still subtle, max 2 stops).

### Footer

- "Synced never (local only)" gains a barely-visible candle-glow dot.

---

## What does NOT change

- Typography family, sizes, weights, leading
- All spacing
- All component geometry, hairlines, radii
- Iconography
- Layout proportions

This is the discipline of the system: dark is a **swap of one set of tokens**, never a re-skin.

---

## Use case

Dark mode is shown on this screen at **9:14 PM, May 24** — the timeline scrolled to the evening's reading session, where a single page is selected on the right. The Local-First card faintly glows. A tea-cup ring would not be out of place on the desktop wallpaper behind it.

---

## Why this works

Most apps' "dark mode" is a hostile blue-black. PathKeep's dark variant treats night as a *reading condition*, warming the palette and softening contrasts. Users who read at midnight should feel they've turned a desk lamp on, not a fluorescent tube.
