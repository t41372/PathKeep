# PathKeep — Editorial Atlas Prototype

> Visual prototype for the v0.2 frontend redesign.
> Status: **written design only — no implementation, no rendered UI screenshots**.
> Source of design language: [`/DESIGN.md`](../../DESIGN.md).

This directory captures the new visual direction for PathKeep. It is intended to be readable by someone who has never opened the codebase: every screen has a careful written description, a literal layout, and the exact content that should appear in the mockup. The mood is established by three photographic plates in `images/`. **Pixel-faithful UI mockups are deliberately not included** (see "On images" below).

## Why a redesign

The current shell defaults to dark mode, monospace UI chrome, and a developer-tool aesthetic. That language signals "tool for working" — but PathKeep's emotional core is "place for remembering". The reference brief (`美術風格原形.md`) argues for the opposite tradition: editorial, paper-feeling, library-quiet. This prototype is the first concrete attempt at that direction.

The single sentence the design should answer is:

> **A reading-room for one's own browsing history.**

If a screen does not feel like that, it is wrong.

## How to read this directory

```
docs/prototype/
├── README.md              ← you are here
├── overview.md            ← design philosophy, screen map, how everything connects
├── screens/               ← one markdown file per surface
│   ├── 01-timeline-light.md   ← the hero screen
│   ├── 02-search-overlay.md   ← ⌘K recall surface
│   ├── 03-dashboard.md        ← Atlas of Your Reading
│   ├── 04-intelligence.md     ← Ask your atlas (local AI)
│   ├── 05-collections.md      ← curated stacks of pages
│   ├── 06-sources.md          ← domain gazetteer
│   ├── 07-settings.md         ← typeset-manual preferences
│   └── 08-timeline-dark.md    ← the reading-room at night
└── images/                ← three photographic mood plates
    ├── mood-01-palette.jpg    ← desk flat-lay with the exact app palette
    ├── mood-02-paper.jpg      ← cream archival paper macro
    └── mood-03-night.jpg      ← warm-tungsten reading desk
```

Each `screens/NN-*.md` follows the same structure:

1. **Purpose** — what this screen exists to do.
2. **Layout** — the literal anatomy of the screen, region by region.
3. **Sample content** — the exact strings, times, and counts that should appear.
4. **Typography & colour rules** — references to tokens defined in `DESIGN.md`.
5. **Why this works** — the editorial thesis the screen embodies.

## Reading order

1. [`overview.md`](./overview.md) — design philosophy and screen relationships
2. [`screens/01-timeline-light.md`](./screens/01-timeline-light.md) — the hero screen, this is the one to judge the language by
3. [`screens/02-search-overlay.md`](./screens/02-search-overlay.md) — recall and advanced filters without a permanent search page
4. [`screens/03-dashboard.md`](./screens/03-dashboard.md) — landing surface, ties archival status and intelligence together
5. [`screens/04-intelligence.md`](./screens/04-intelligence.md) — local-AI surface, "ask your atlas"
6. [`screens/05-collections.md`](./screens/05-collections.md) — curated stacks as index cards
7. [`screens/06-sources.md`](./screens/06-sources.md) — domain-level gazetteer
8. [`screens/07-settings.md`](./screens/07-settings.md) — preferences as a typeset manual
9. [`screens/08-timeline-dark.md`](./screens/08-timeline-dark.md) — the warm reading-room dark variant

## On images

This prototype intentionally **does not** ship rendered pixel-faithful UI mockups. We tried; current text-to-image models cannot reliably render dense, multi-column desktop UIs with legible typography — every attempt produced gibberish text, off-brand colour usage, or hallucinated chrome. Faking a mockup would have done more harm than good.

Instead, the `images/` folder contains three **photographic mood plates** that the model *can* render faithfully:

| Plate | Purpose |
| --- | --- |
| `mood-01-palette.jpg` | Overhead flat-lay of a private librarian's desk. Establishes the exact palette (cream, ink, taupe, claret, olive) and the tactile vocabulary (paper, wax seal, fountain pen, claret ink, brass paperclip, leather notebook). |
| `mood-02-paper.jpg` | Macro of layered archival cream paper sheets with a single deep-claret pen stroke. The literal substrate of the app. |
| `mood-03-night.jpg` | Warm-tungsten reading-desk scene with green banker's-lamp glass and a glowing claret ink drop. The brief for the dark variant. |

These plates are meant to be pinned next to the spec while reading. They answer the question "what does this app *feel* like?" — the markdown answers "what does it *do*?".

For the literal pixel-level reference of the proposed UI, see the original screenshot the user attached at the top of the conversation that introduced this prototype. That image is the closest thing we have to a true mockup, and the spec in this directory was written to be consistent with it.

## What this prototype does not yet cover

These exist in the product, but dedicated visual treatment is deferred to a follow-up pass:

- Import / Audit / Jobs PME (Preview / Manual / Execute / Verify) flows
- Maintenance derived-state / rebuild surfaces
- App Lock / biometric unlock screen
- AI Assistant disabled / coming-in-v0.2 surface
- Notification queue panel
- Schedule install / repair states
- Onboarding (a separate `screens/09-onboarding.md` may be added in a follow-up)

The design language for these will follow the conventions established in the screens above. None of them should require new tokens or a new component vocabulary.

## Implementation status

**Nothing in this directory has been implemented.** The codebase still uses the previous dark-default, square, mono-heavy language documented in `docs/design/design-tokens.md` and `src/styles/tokens.css`. Implementation is a separate workstream that will:

1. Extend `DESIGN.md` tokens into `src/styles/tokens.css` with new theme names (`day` / `darkroom`).
2. Add the three font families with proper local fallbacks (no CDN, no large bundled fonts).
3. Rebuild the shell, sidebar, status bar, and timeline against this language.
4. Migrate one route at a time behind a feature flag, beginning with the History Explorer (Timeline).

Until that work begins, this directory is the canonical reference for what the redesign is supposed to feel like.
