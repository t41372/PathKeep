# Design Tokens

> Source of truth for the v0.3 **Paper + Archival** product shell token layer.
> Visual direction comes from `docs/design/handoff/paper-redesign/project/pk-tokens.css` (the design handoff package), then gets normalized here for production CSS variables and Tailwind v4 `@theme` mapping.
> Typography trade-off 與 fallback policy 見 [typography-and-font-fallback.md](./typography-and-font-fallback.md)。
> History: v0.2 brutalist tokens (square geometry, orange accent, dark default) are retired in this branch. The pivot rationale lives in `docs/dev/HANDOFF-2026-05-19-paper-redesign.md`. CSS-variable legacy aliases (`--bg` / `--text` / `--font-ui`) stay in `src/styles/tokens.css` while v0.2 routes get rewritten, then get deleted alongside the last consumer.

---

## Theme Contract

- Default theme: `light` (named "paper")
- Secondary theme: `dark` (named "darkroom")
- Accent policy: one action color only (`--accent` — slate blue, not orange)
- Shape policy: paper geometry — `--radius: 3px`, `--radius-pill` for status chips. **Not** brutalist 0 px anymore (see [[feedback-brutalist-radius]]).
- Typography policy:
  - editorial headings + body copy use `--font-serif` (Newsreader Latin subset bundled at runtime)
  - UI chrome / dense labels use `--font-sans` (system sans for the active locale)
  - ASCII evidence — paths, IDs, commands, mono badges — uses `--font-mono` (JetBrains Mono Latin subset bundled at runtime)
  - **CJK always falls back to the system stack**. Bundled fonts ship Latin subsets only.
  - `html[lang]` must follow the runtime locale (en / zh-CN / zh-TW).
- Material policy: ~2.8 % paper noise (light) / 4 % noise + soft-light blend (dark) + 12 % darkroom vignette in dark mode. Settings → Appearance can disable both.

## Color Tokens (paper palette)

`src/styles/tokens.css` is the visual source of truth for the palette. The values
below mirror it exactly; the **naming scheme is also the runtime one** — three
stacked surfaces are `--bg-page` (desk) < `--bg-paper` (sheet) < `--bg-card`
(raised card). There is no `--bg-card-paper` / `--bg-accent-soft` / `--danger`;
those legacy names were never shipped. Soft accent/semantic tints are derived at
runtime via `color-mix()` rather than hand-written rgba, so the surviving alpha
is expressed as the mix percentage.

### Surfaces

| Token          | Light ("paper")          | Dark ("darkroom")    | Role                                         |
| -------------- | ------------------------ | -------------------- | -------------------------------------------- |
| `--bg-page`    | `#ece7de`                | `#110f0d`            | desk behind cards (cream paper / near-black) |
| `--bg-paper`   | `#f6f3ed`                | `#191614`            | main reading sheet                           |
| `--bg-card`    | `#fdfcf9`                | `#201c18`            | raised card / detail panel / popover         |
| `--bg-hover`   | `#e6e0d5`                | `#292420`            | row / control hover                          |
| `--bg-active`  | `#dbd4c7`                | `#332d27`            | pressed / selected row                       |
| `--bg-sidebar` | `#f0ebe3`                | `#151311`            | sidebar rail                                 |
| `--bg-overlay` | `rgba(28, 24, 20, 0.16)` | `rgba(0, 0, 0, 0.5)` | modal scrim                                  |

`--bg-elevated` aliases `--bg-card` in both themes.

### Ink (text)

Readability is a hard contract: every ink token that can carry copy clears
**WCAG AA (4.5:1)** on the two reading surfaces (`--bg-paper`, `--bg-card`).
`--ink-ghost` is the sole exception — it is **decorative-only and must never
hold readable text** (hairlines, disabled chrome, zero-state dividers). The
WCAG guard in `src/styles/tokens.contrast.test.ts` enforces this against the
shipping values.

| Token             | Light     | Dark      | AA on paper / card                               |
| ----------------- | --------- | --------- | ------------------------------------------------ |
| `--ink`           | `#1c1814` | `#d4cbc0` | primary text — far above AA                      |
| `--ink-secondary` | `#4a4139` | `#a79d8e` | secondary body — above AA                        |
| `--ink-muted`     | `#6e6556` | `#928c80` | muted helper — light 5.2 / 5.6, dark 5.4 / 5.1   |
| `--ink-faint`     | `#726e5a` | `#908374` | faint metadata — light 4.6 / 5.0, dark 4.9 / 4.6 |
| `--ink-ghost`     | `#d5cdc0` | `#2a2520` | decorative only — **no text**                    |

### Borders

| Token             | Light     | Dark      |
| ----------------- | --------- | --------- |
| `--border`        | `#d5cdc0` | `#302b25` |
| `--border-light`  | `#e4ddd2` | `#262220` |
| `--border-strong` | `#c2b8a8` | `#3d362e` |

### Accent (slate blue, user-configurable via `--accent-color`)

| Token             | Light                                | Dark                                                              |
| ----------------- | ------------------------------------ | ----------------------------------------------------------------- |
| `--accent`        | `#3d5a80` (`var(--accent-color)`)    | `#3d5a80` fallback                                                |
| `--accent-soft`   | `color-mix(accent 8%, transparent)`  | `color-mix(accent 10%, transparent)`                              |
| `--accent-medium` | `color-mix(accent 18%, transparent)` | `color-mix(accent 20%, transparent)`                              |
| `--accent-strong` | `color-mix(accent 32%, transparent)` | `color-mix(accent 36%, transparent)`                              |
| `--accent-text`   | `var(--accent)`                      | `color-mix(accent 65%, #b0c8e4)` (lifted for darkroom legibility) |

### Semantic

The semantic family uses `--success` / `--warning` / `--error` / `--info` (plus a
`-soft` `color-mix` tint each). There is no `--danger` token; destructive UI uses
`--error` / `--error-soft`.

| Token       | Light     | Dark      | Soft tint                                   |
| ----------- | --------- | --------- | ------------------------------------------- |
| `--success` | `#4a8c5c` | `#6aac7c` | `color-mix(success 12% / 16%, transparent)` |
| `--warning` | `#b38b2d` | `#d3ab4d` | `color-mix(warning 14% / 16%, transparent)` |
| `--error`   | `#a84040` | `#c86060` | `color-mix(error 12% / 16%, transparent)`   |
| `--info`    | `#4a7aa8` | `#6a9ac8` | `color-mix(info 12% / 16%, transparent)`    |

> `src/styles/tokens.css` is the visual source of truth and the runtime values
> are authoritative. Keep this document in lockstep with it: when the palette is
> deliberately retuned, update both together. The one non-negotiable constraint
> the code must always satisfy is the WCAG AA readability of the ink ramp above —
> if a value here and in `tokens.css` ever diverge on contrast, fix the value
> that fails AA (verified by `tokens.contrast.test.ts`), not the other way
> around.

## Semantic Tone Tokens

Used by `StatusCallout`, paper alerts, and badge primitives. `StatusCallout`
accepts the tone _prop_ names `info | warning | danger | blocked | success`;
those map onto the semantic color tokens above (`--error` is the destructive
color — there is no `--danger` token):

- success — accent-soft tint over `--ink` (success copy uses neutral text on tinted paper, not a green accent)
- info — same as accent-soft tint
- warning — `--warning-soft` background, `--warning` text/border
- danger / blocked — `--error` border + `--error-soft` background

The paper aesthetic intentionally collapses success / info into the same accent-soft surface. The user has reading-life materials in front of them, not a build dashboard.

## Typography Tokens

- `--font-serif` `'Newsreader', Georgia, serif`
- `--font-sans` system sans stack via `system-ui`, with platform fallbacks
- `--font-mono` `'JetBrains Mono', ui-monospace, SFMono-Regular, monospace`
- Bundled font subsets via `@fontsource/newsreader` and `@fontsource/jetbrains-mono` (Latin only).
- `data-fonts="system"` on `<html>` swaps to system fallback (Settings → Appearance toggle).

## Spacing And Density

The paper grid stays on the same 4 px rhythm as v0.2 but adds new layout tokens for the shell:

- Spacing scale (`--space-1` … `--space-12`) unchanged from v0.2: 4 / 8 / 12 / 16 / 20 / 24 / 32 / 40 / 48 px.
- Layout shell density:
  - `--layout-sidebar-width` `216px` (expanded), `56px` (collapsed)
  - `--layout-topbar-height` `52px`
  - `--layout-status-bar-height` `28px`
  - `--density-card-padding` `18px`
  - `--density-card-gap` `16px`
- Settings → Appearance "Density" toggle picks between `comfortable` (default) and `compact` (-2 px on each `--density-*` token).

## Motion

- `--transition` `120ms ease` — shared default. Hover / active state, palette open/close, sidebar collapse.
- `--transition-detail` `200ms cubic-bezier(0.2, 0, 0.2, 1)` — detail panel slide-in.
- Motion stays restrained. No bouncy animations, no spring physics. Editorial restraint matches the paper aesthetic.

## Radius

- `--radius` `3px` — paper card corners, button corners, input corners. Three pixels reads as "deliberate edge" without feeling brutalist.
- `--radius-pill` `9999px` — status chips, source-picker pill, palette mode badge.
- `--radius-tight` `2px` — tiny chips (heatmap cells, source-color swatches).

## Implementation Files

- CSS variables: `src/styles/tokens.css`
- WCAG contrast guard for the ink ramp: `src/styles/tokens.contrast.test.ts`
- Paper-specific texture / noise / animation tokens: `src/styles/paper.css`
- Tailwind v4 `@theme` mapping → token bridge: `src/styles/tailwind.css`
- Bundled fonts CSS: `src/styles/fonts.css`
- Paper preferences (theme / font / density / paperTexture) persistence: `src/lib/paper-preferences.ts`

## Usage Rules

- New shell / page work consumes paper tokens via Tailwind utilities (`bg-paper`, `text-ink`, `border-border-light`, `text-accent`, `bg-accent-soft`, `font-serif`, `font-mono`) — **not** the raw CSS variables.
- Tailwind utilities map to paper tokens through the `@theme` block in `tailwind.css`. New tokens always update both files together.
- Do not reintroduce per-page color constants into components.
- `--ink-ghost` (`text-ink-ghost`) is decorative-only — never apply it to readable text. Use `--ink-faint` or `--ink-muted` for the faintest legible copy; both clear WCAG AA on the paper-card surfaces. `tokens.contrast.test.ts` enforces the ramp.
- v0.2 routes that still consume legacy aliases (`--bg`, `--text`, `--font-ui`, `--border-active`, etc.) are migration debt; aliases live at the bottom of `tokens.css` and get deleted once the last consumer is rewritten.
- Settings → Appearance is the single user-facing surface that writes paper preferences (theme / fonts / density / paperTexture); it goes through `applyPaperPreferences()` which is idempotent on `<html>` attributes.

## Status

Accepted (2026-05-19) — paper redesign supersedes the v0.2 brutalist tokens.
