# Design Overview

> Companion to [`README.md`](./README.md). Read this once, then dive into the per-screen files.

## 1. The single sentence

> **A reading-room for one's own browsing history.**

Every visual decision in this prototype is downstream of that sentence. If a screen makes the user feel like they are *operating a dashboard*, the screen has drifted. If it makes them feel like they are *opening a folder of their own past*, the screen is correct.

## 2. Mood references

The redesign deliberately steps away from the canonical 2024–2025 developer-tool aesthetic. The reference set is editorial and archival rather than productive:

| Aspires toward | Why |
| --- | --- |
| Are.na | Treats "saved things" as a serious cultural object, not a CRM record. |
| iA Writer | Religious commitment to typography and restraint. |
| Readwise Reader / Matter | Long-form reading apps with confident serif body type. |
| Internet Archive | The institution this product is a personal mirror of. |
| Folger Shakespeare Library | What "special collection" feels like in software. |
| Reeder 5 | Mac-native quietness without retro skeuomorphism. |
| craigmod.com | Walking-pace pacing, paper-feeling layout, generous measure. |

| Deliberately avoided | Why |
| --- | --- |
| Linear | Has become the new Material Design — "good but interchangeable". |
| Vercel dashboard | Mono-heavy, neon-on-near-black; signals dev tool. |
| Raycast | Command-palette-first; PathKeep is browse-first. |
| Notion / Granola | Amber/coral accent on cream is now visual default for "paper-feel SaaS". PathKeep takes oxblood instead. |

## 3. The visual signature

Three things, when seen together, must read as PathKeep:

1. **Unbleached paper background with hairline structure.**
2. **Serif page titles + mono timestamps + sans UI chrome — never collapsed into a single family.**
3. **A single oxblood accent — used like a rubber stamp, not like a button color.**

Take any one of those away and the design collapses into a generic light-mode SaaS app.

## 4. Two themes, equal status

| Theme | Mood | When it earns its keep |
| --- | --- | --- |
| **Day** (light, default) | Reading-room. Unbleached paper, ink-warm blacks. | Daytime browsing of past weeks; the canonical mode. |
| **Darkroom** (dark) | Single warm lamp over an espresso desk. Cream ink on warm near-black, faint film grain. | Late-night solo sessions; never used as "developer mode". |

Neither theme is the canonical one. The dark theme is *not* an inverted Day; it is its own atmosphere. See `screens/08-timeline-dark.md` for the specific contract.

## 5. Two density modes, that is all

There is no font-size slider, no accent picker, no compact/cozy/spacious tri-state. The user gets:

- **Reading** (default) — generous measure, serif body sized to be read.
- **Compact** — same family, smaller line-height and tighter rail; for users with thousands of entries per day.

Everything else is fixed. This is the Apple-school decision: give the user a real choice once, not a configuration page.

## 6. Information architecture

### Sidebar (left rail, ~240 px)

The rail follows the existing IA from `docs/design/screens-and-nav.md`, restated in editorial vocabulary:

- **CORE**: Timeline · Search · Intelligence · (Assistant — disabled, v0.2 honesty copy)
- **OPERATIONS**: Sources · Collections · Audit · Jobs · Integrations · Import
- **SYSTEM**: Schedule · Security · Settings · Maintenance

For the prototype, the sidebar shown in the mockups uses a condensed editorial wording — `Timeline / Search / Sources / Collections / Intelligence / Settings` — to keep first-impression density low. The full IA above is preserved through grouped sections that the user can expand. The condensed wording is *not* a removal of features; it is a first-page-of-the-book table of contents.

### Topbar

- Back / forward (history of routes within this session, never the browser history under analysis).
- Inline search (becomes the global ⌘K overlay on focus / press).
- Date range chip (resolves to a calendar popover).
- Source filter chip · Type filter chip · Filter icon (opens advanced filter drawer).
- Right cluster: notifications · profile-scope switcher · `Backup now` primary CTA.

### Detail panel (right, ~360 px)

Mirrors current selection. Collapses on narrow windows. Holds the page summary, metadata table, tags, connections, local-first status, visit-history mini-chart.

### Status bar

Always visible. Mono 12px. `7,842 pages · 1.3 GB · Synced never (local only)` left, `Indexed on May 24, 2026 at 6:03 AM` right. This is where background work resolves.

## 7. Where each prototype screen sits

```
                                    ┌──────────────────────┐
                                    │    Onboarding (05)   │  ← first launch only, exits to Dashboard
                                    └──────────┬───────────┘
                                               │
                       ┌───────────────────────▼────────────────────────┐
                       │                                                │
              ┌────────▼────────┐                            ┌──────────▼─────────┐
              │   Dashboard (03) │  ← landing, summarises    │   Timeline (01/08)  │  ← hero, the canonical surface
              │  archive + day  │     archive state          │  light + darkroom   │
              └────────┬────────┘                            └──────────┬─────────┘
                       │                                                │
            ┌──────────┼──────────┐                                     │
            │          │          │                                     │
   ┌────────▼──┐ ┌─────▼─────┐ ┌──▼──────────┐                ┌─────────▼──────────┐
   │ Intelli-  │ │ Sources   │ │ Settings    │                │  Search Overlay (02) │  ← ⌘K from anywhere
   │ gence (04)│ │ (06)      │ │ (07)        │                │  advanced filters    │
   └───────────┘ └───────────┘ └─────────────┘                └──────────────────────┘
```

- **Onboarding (05)** is encountered exactly once and hands off to the Dashboard.
- **Dashboard (03)** is the landing surface and the only screen that mixes archival status with intelligence at-a-glance.
- **Timeline (01)** is the hero — the screen the user spends most hours inside, and the one new users send screenshots of. Its dark variant is **(08)**.
- **Search Overlay (02)** replaces a dedicated search page. It is reachable from every screen via ⌘K and absorbs the previous "advanced search" page.
- **Intelligence (04)** is the analytical surface — calendar heatmap, top sites, search activity, recap.
- **Sources (06)** manages browsers and profiles with retention honesty.
- **Settings (07)** is the colophon — language, theme, density, app lock, archive location.

## 8. Search and advanced filters — the simplification

The previous design promoted advanced search to a top-level navigation item with its own page. This prototype demotes it to a **secondary surface inside the global search overlay**:

- Global ⌘K opens a centered overlay with a single input.
- Below the input, recent queries and a small "Advanced" disclosure.
- Expanding "Advanced" reveals chips for *Date range / Source / Type / Domain / Has note / Has snippet / Regex*. No new page, no new route.
- The Sidebar's `Search` entry is preserved as a discoverable affordance for users who have never pressed ⌘K, but it opens the same overlay.

This collapses what used to be three search surfaces (sidebar entry, page, advanced page) into one overlay with a single advanced disclosure. See `screens/02-search-overlay.md`.

## 9. The contract for image generation

Each `screens/*.md` file ends with the exact prompt used to generate its concept image, the model's output, and a review note. Concept images are *static targets* — they are not pixel-perfect specs. Implementation should converge toward the language they convey, not their literal pixel values.

Three rules that override anything an image gen model produces:

1. The center column always uses serif for entry titles. If a generated image puts entry titles in sans, that image is wrong.
2. The accent is always oxblood (`#7B1F2A`), never amber, coral, or terracotta. Concept images that drift warm-orange are rejected.
3. The default theme is light. If an image returns dark-as-primary, it is rejected unless explicitly the darkroom variant.

## 10. What "done" looks like for this prototype

- Every screen file in `screens/` has a written description and a paired image.
- `DESIGN.md` at the repo root captures the token contract.
- A reader who never opens the codebase can understand what PathKeep wants to feel like by reading this directory front to back.
- Implementation has not started.
