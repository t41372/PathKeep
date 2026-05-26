# BROWSE-VIRT Spike — DOM/Cache/Prefetch Sizing

> 2026-05-25 — preparation for `WORK-FEEDBACK-0525-BROWSE-VIRT`
> (feedback-2026-05-25 §1.1 sliding-window DOM recycling + §1.2
> directional prefetch).
> Source measurement: `src/components/explorer-paper/paper-contact-sheet.spike.test.tsx`

## Why measure before implementing

The user explicitly rejected the "禁止再往下滑" stopgap and required a
sliding-window recycler with directional prefetch. Picking a window
size, cache cap, and prefetch budget without numbers risks either
(a) virtualisation that still freezes the box, or (b) over-aggressive
caps that thrash backend reads on the target 4-core / 8 GB profile.

The spike grounds those numbers in **measured DOM cost per row** today,
so the implementation has a concrete baseline to beat.

## Measured DOM footprint (jsdom)

| Scenario           | viewMode | total rows | DOM nodes | nodes / row |
| ------------------ | -------- | ---------- | --------- | ----------- |
| 1 day × 50 rows    | list     | 50         | 345       | 6.90        |
| 10 days × 50 rows  | list     | 500        | 3 153     | 6.31        |
| 100 days × 50 rows | list     | 5 000      | 31 233    | 6.25        |
| 100 days × 50 rows | cards    | 5 000      | 71 233    | 14.25       |

- **List mode** stabilises at **~6.25 nodes / row** once per-day chrome
  overhead amortises.
- **Cards mode** is **2.3× heavier per row** (~14.25 nodes / row) because
  PaperContactFrame carries domain swatch + favicon stack + title +
  meta + actions.
- **Per-day chrome** (PaperDayHeader + PaperDayInsights + PaperSessionHeader)
  is ~30 nodes — small as long as days aren't tiny.

The current `MAX_ACCUMULATED_PAGES = 100` × 50 rows / page = 5 000 rows
caps DOM at:

- ~31 k nodes (list mode)
- ~71 k nodes (cards mode)

The 71 k figure is exactly where users see the freeze the feedback
reports — Chrome's compositor + style recalc both go non-linear past
~50 k mounted elements on a 4-core / 8 GB box, even before any layout
animation.

## Sizing decisions

### Window (DOM)

| Mode  | Visible rows (1080p) | Buffer above/below | Mounted rows | Day chrome | **Target DOM nodes** |
| ----- | -------------------- | ------------------ | ------------ | ---------- | -------------------- |
| List  | ~18                  | 18 each side       | ~54          | ~60        | **~400 nodes**       |
| Cards | ~12 (3-col)          | 12 each side       | ~36          | ~60        | **~580 nodes**       |

Even on a 4 K monitor doubling visible-row counts, target stays
< 1 500 mounted nodes — **two orders of magnitude below today's cap**.

### Cache (JS-only, no DOM)

- `HistoryEntry` is ~300 bytes wire-format (id, urls, titles, etc.).
  In-memory React object cost is closer to ~600 bytes.
- **Cap at 50 000 entries** in the page buffer = ~30 MB. Enough for
  the user to scroll back several months on a dense archive without
  re-hitting the worker pool, but bounded so the 8 GB box stays
  comfortable.
- LRU eviction by oldest-accessed page (each page = 50 entries), not
  by individual row — page is the natural granularity since
  `queryHistory` already returns whole pages.

### Prefetch budget (directional)

The existing hook already warms 1 page ahead of the sentinel.
With recycling, we can afford more:

- **Scroll-down (older history)**: warm **page + 1 and page + 2** so
  the sentinel never has to wait. Sequential pages are the dominant
  access pattern.
- **Scroll-up (newer history, after recycling out)**: warm **page − 1**
  in the background. Up-scroll is rare on Browse but recovering from
  it must be instant — cache covers most cases; the +1 background
  fetch covers the cache-miss tail.
- **Direction signal**: 100 ms scroll-velocity sample; flip direction
  on the first 4 consecutive samples in the opposite sense (smooths
  inertial-scroll wobble).
- **Worker-pool politeness**: cap concurrent prefetches at 2 so we
  don't starve user-initiated queries (search, filters).

### Page accumulation cap

Bump `MAX_ACCUMULATED_PAGES` from 100 → **1 000** once virtualisation
is in place. The DOM is no longer the limit; the cache cap above
governs memory.

## What the implementation must preserve

Per `WORK-FEEDBACK-0525-BROWSE-VIRT` 契約:

- A11y / keyboard nav, day-sticky header, per-day insights, sessions
  grouping, infinite-scroll sentinel.
- View toggle (cards / list) must not cause > 50 ms layout jank.
- Existing `IntersectionObserver` sentinel pattern stays — the virt
  layer slots underneath it.
- 100 % JS coverage; new Playwright e2e on a large preview fixture
  verifying smooth scroll.

## Open questions for implementation

- **react-virtual vs custom recycler**: dependency authorisation
  granted for `@tanstack/react-virtual` (Tanner Linsley / TanStack
  org, ~5 k stars on the standalone package, ~80 k on the umbrella —
  passes the AGENTS.md trust gate via "maintainer high-knowledge +
  strict review"). Use react-virtual.
- **Variable row heights**: list rows are uniform-ish, but cards mode
  has wrap-dependent height. react-virtual supports `estimateSize` +
  `measureElement` for this.
- **Sticky day header positioning**: virtualisation typically
  conflicts with CSS `position: sticky` on group separators. Plan
  is to keep day headers as separate "virtual rows" interleaved with
  entry rows (one logical row stream) so sticky still works on the
  scroller, mirroring how react-virtual sticky-headers examples wire
  it.

## Re-running the measurement

```sh
bun run test:unit src/components/explorer-paper/paper-contact-sheet.spike.test.tsx -- --reporter=verbose
```

The verbose reporter prints the measurement table to stdout. Re-run
after the virt implementation lands to confirm the targets (~400
nodes list, ~580 nodes cards) are met.
