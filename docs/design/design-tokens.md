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

### Light ("paper")

- `--bg-paper` `#ece7de` — main page background (cream paper)
- `--bg-card-paper` `#f6f3ed` — paper-card backgrounds
- `--bg-page` `#fdfcf9` — elevated surface / palette / detail panel
- `--bg-hover` `rgba(0, 0, 0, 0.04)` — hover overlay
- `--bg-accent-soft` `rgba(61, 90, 128, 0.12)` — accent tint / highlight chip
- `--border-light` `#e6e2d8` — paper card subtle border
- `--border-default` `#d6d1c4` — main border
- `--ink` `#171513` — primary text
- `--ink-secondary` `#3c3a36` — secondary body text
- `--ink-muted` `#6a655c` — muted helper text
- `--ink-faint` `#9a948a` — faint metadata
- `--ink-ghost` `#bdb6a9` — ghost text
- `--accent` `#3d5a80` — slate blue, single accent color
- `--accent-text` `#2c4360` — accent text color (slightly darker for legibility on paper)
- `--accent-soft` (alias for `--bg-accent-soft`)
- `--danger` `#a03821` — destructive action / inline error
- `--danger-soft` `rgba(160, 56, 33, 0.12)`

### Dark ("darkroom")

- `--bg-paper` `#110f0d` — main page background
- `--bg-card-paper` `#191614` — paper-card backgrounds
- `--bg-page` `#201c18` — elevated surface / palette
- `--bg-hover` `rgba(255, 255, 255, 0.06)`
- `--bg-accent-soft` `rgba(122, 156, 199, 0.18)`
- `--border-light` `#26221e` — paper card subtle border
- `--border-default` `#322c26` — main border
- `--ink` `#ede7d8` — primary text
- `--ink-secondary` `#c7c0b1` — secondary body text
- `--ink-muted` `#8a8478` — muted helper text
- `--ink-faint` `#5d574d` — faint metadata
- `--ink-ghost` `#3f3a32` — ghost text
- `--accent` `#7a9cc7` — slate blue (lighter for darkroom)
- `--accent-text` `#9bb6d6`
- `--danger` `#d65f3f`
- `--danger-soft` `rgba(214, 95, 63, 0.18)`

> The exact runtime values live in `src/styles/tokens.css`; this document is the contract those values implement, not a duplicate copy. If `tokens.css` and this file disagree, fix `tokens.css` to match this contract — not the other way around.

## Semantic Tone Tokens

Used by `StatusCallout`, paper alerts, and badge primitives:

- success — accent-soft tint over `--ink` (success copy uses neutral text on tinted paper, not a green accent)
- info — same as accent-soft tint
- warning — `--danger-soft` background, `--danger` text/border
- danger / blocked — `--danger` border + `--danger-soft` background

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
- Paper-specific texture / noise / animation tokens: `src/styles/paper.css`
- Tailwind v4 `@theme` mapping → token bridge: `src/styles/tailwind.css`
- Bundled fonts CSS: `src/styles/fonts.css`
- Paper preferences (theme / font / density / paperTexture) persistence: `src/lib/paper-preferences.ts`

## Usage Rules

- New shell / page work consumes paper tokens via Tailwind utilities (`bg-paper`, `text-ink`, `border-border-light`, `text-accent`, `bg-accent-soft`, `font-serif`, `font-mono`) — **not** the raw CSS variables.
- Tailwind utilities map to paper tokens through the `@theme` block in `tailwind.css`. New tokens always update both files together.
- Do not reintroduce per-page color constants into components.
- v0.2 routes that still consume legacy aliases (`--bg`, `--text`, `--font-ui`, `--border-active`, etc.) are migration debt; aliases live at the bottom of `tokens.css` and get deleted once the last consumer is rewritten.
- Settings → Appearance is the single user-facing surface that writes paper preferences (theme / fonts / density / paperTexture); it goes through `applyPaperPreferences()` which is idempotent on `<html>` attributes.

## Status

Accepted (2026-05-19) — paper redesign supersedes the v0.2 brutalist tokens.
