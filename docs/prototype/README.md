# PathKeep — Editorial Atlas Prototype

> Visual prototype for the v0.2 frontend redesign.
> Status: **design only — not implementation**.
> Source of design language: [`/DESIGN.md`](../../DESIGN.md).

This directory captures the new visual direction for PathKeep. It is intended to be readable by someone who has never opened the codebase: every screen has a written description, a generated mockup, and an explanation of how it relates to the rest of the app.

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
├── screens/               ← one markdown file per surface, each paired with an image
│   ├── 01-timeline-light.md
│   ├── 02-search-overlay.md
│   ├── 03-dashboard.md
│   ├── 04-intelligence.md
│   ├── 05-onboarding.md
│   ├── 06-sources.md
│   ├── 07-settings.md
│   └── 08-timeline-dark.md
└── images/                ← rendered concept images, referenced from screen docs
    └── *.jpg
```

Each `screens/NN-*.md` follows the same structure:

1. **Purpose** — what this screen exists to do.
2. **Where it lives** — route, navigation entry, deep-link grammar.
3. **Layout** — the literal anatomy of the screen, region by region.
4. **Components used** — references to tokens / primitives in `DESIGN.md`.
5. **Content shown in the mockup** — exact strings used so reviewers can compare.
6. **States this image does and does not cover** — empty / loading / error variants are listed separately.
7. **Image** — the rendered concept image.

## Reading order

For a first-pass review, read in this order:

1. [`overview.md`](./overview.md) — design philosophy and screen relationships
2. [`screens/01-timeline-light.md`](./screens/01-timeline-light.md) — the hero screen, this is the one to judge the language by
3. [`screens/02-search-overlay.md`](./screens/02-search-overlay.md) — how recall and advanced filters work without a permanent search page
4. [`screens/03-dashboard.md`](./screens/03-dashboard.md) — landing surface, ties archival status and intelligence together
5. [`screens/04-intelligence.md`](./screens/04-intelligence.md) — analytical surface, contact-sheet metaphor at full fidelity
6. [`screens/05-onboarding.md`](./screens/05-onboarding.md) — first impression for new users
7. [`screens/06-sources.md`](./screens/06-sources.md) — browser / profile management with retention honesty
8. [`screens/07-settings.md`](./screens/07-settings.md) — preferences as a colophon page
9. [`screens/08-timeline-dark.md`](./screens/08-timeline-dark.md) — the darkroom variant, evaluated against the light hero

## What this prototype does not yet cover

These exist in the product, but their dedicated visual treatment is deferred to a follow-up pass:

- Import / Audit / Jobs PME (Preview / Manual / Execute / Verify) flows
- Maintenance derived-state / rebuild surfaces
- App Lock / biometric unlock screen
- AI Assistant disabled / coming-in-v0.2 surface
- Notification queue panel
- Schedule install / repair states

The design language for these will follow the conventions established in the screens above. None of them should require new tokens or a new component vocabulary.

## Implementation status

**Nothing in this directory has been implemented.** The codebase still uses the previous dark-default, square, mono-heavy language documented in `docs/design/design-tokens.md` and `src/styles/tokens.css`. Implementation is a separate workstream that will:

1. Extend `DESIGN.md` tokens into `src/styles/tokens.css` with new theme names (`day` / `darkroom`).
2. Add the three font families with proper local fallbacks (no CDN, no large bundled fonts).
3. Rebuild the shell, sidebar, status bar, and timeline against this language.
4. Migrate one route at a time behind a feature flag, beginning with the History Explorer (Timeline).

Until that work begins, this directory is the canonical reference for what the redesign is supposed to feel like.
