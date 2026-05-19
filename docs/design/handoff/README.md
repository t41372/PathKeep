# Design Handoff — Source of Truth

This directory holds **immutable design source files** received from external
design tooling. They are the visual contract that the v0.3 paper redesign
implements. Treat them as **read-only reference**.

## Current handoff

- [`paper-redesign/`](./paper-redesign/) — Paper + Archival aesthetic (M16).
  Original Claude Design (claude.ai/design) handoff bundle. Contains:
  - `README.md` — handoff cover sheet from the design tool
  - `project/PathKeep Redesign.html` — entry HTML loading all JSX
  - `project/pk-tokens.css` — every CSS class used in the prototype (3,978 lines)
  - `project/pk-app.jsx` — root shell composition + routing + tweaks
  - `project/pk-components.jsx` — `PKSidebar` / `PKStatusBar` / `PKDetailPanel` /
    `PKSearchPalette` / `PKHeatmap` / glyph set + domain colour helpers
  - `project/pk-views.jsx` — `HomeView` (Dashboard) editorial layout
  - `project/pk-contactsheet.jsx` — Browse view (contact sheet, day sticky,
    domain stacks, session insights, hourly sparkline)
  - `project/pk-browse-nav.jsx` — `CalendarPopover` / `DayNavControl` /
    `YearRail` / archive density helper / placeholder day skeleton
  - `project/pk-search.jsx` — three-mode search hero, filter chips, day-grouped
    results, "See in context" jump
  - `project/pk-intelligence.jsx` — KPI strip, topic timeline, domain rank
    list, sessions, refind shelf, LLM-needed callouts
  - `project/pk-assistant.jsx` — chat surface with evidence panel + sample
    prompts (provider-gated in production)
  - `project/pk-import.jsx` — method picker + stepper wizard + preview stats
  - `project/pk-audit.jsx` — manifest chain visualization, runs table, storage
    breakdown, snapshots, export panel
  - `project/tweaks-panel.jsx` — design-tool tweaks panel (development only,
    not shipped)
  - `project/pathkeep-mark.svg` — brand mark

## How agents should use this

1. **Treat the HTML/JSX as visual law, not code to copy.** The handoff renders
   in a browser via Babel-standalone; production builds with React 19 + TS +
   Tailwind v4 + shadcn primitives. Re-derive class names, structure, and
   tokens — don't transplant the prototype's React-18-via-UMD plumbing.
2. **`pk-tokens.css` is the visual rule book.** Every measurement, every
   colour, every animation curve. When in doubt, grep here first; it lists
   exact pixel sizes (e.g. sidebar `216px`, topbar `52px`, statusbar `28px`,
   day sticky offset `var(--cs-toolbar-h, 44px)`).
3. **Functional depth may exceed the prototype.** The prototype mocks data
   and skips many production concerns (i18n, AI provider gating, regex
   validation, locked archive states, error boundaries). Backfill these
   using the existing PathKeep production patterns.
4. **The Settings page is not in the prototype.** Build it with the same
   visual language; see `src/pages/settings/` for existing sections.

## Authoritative project docs (override these as the redesign progresses)

- `docs/design/design-tokens.md` — token catalogue (paper palette is current)
- `docs/design/screens-and-nav.md` — route contract per screen
- `docs/design/ux-principles.md` — PME, trust grammar, loading rules
- `docs/design/ui-review-guardrails.md` — review heuristics
- `docs/design/typography-and-font-fallback.md` — three-font policy

The v0.3 redesign is **authorised to override** these docs where it
contradicts the brutalist v0.2 design system; update each accepted doc as
the route sweep lands.
