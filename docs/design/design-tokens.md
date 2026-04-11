# Design Tokens

> Source of truth for the M0 product shell token layer.  
> Visual direction comes from `reference/PathKeep — Desktop UI Design/style.css`, then gets normalized here for production CSS variables and TS token helpers.
> Typography trade-off 與 fallback policy 見 [typography-and-font-fallback.md](./typography-and-font-fallback.md)。

---

## Theme Contract

- Default theme: `dark`
- Secondary theme: `light`
- Accent policy: one action color only (`--accent`)
- Shape policy: brutalist, square geometry (`--radius: 0px`)
- Typography policy:
  - primary UI chrome、dense labels 與 body copy 使用 `--font-ui` / `--font-body`
  - true monospace 只保留給 path、ID、command 與純 evidence values，使用 `--font-code`
  - `--font-mono` 僅保留為 legacy shell alias；新 UI 不再把 monospace 當預設字體
  - runtime 不可再依賴 remote font import；`html[lang]` 必須與目前 locale 對齊

## Color Tokens

### Dark

- `--bg` `#0A0A0A`
- `--bg-elevated` `#111111`
- `--bg-surface` `#161616`
- `--bg-hover` `#1A1A1A`
- `--border` `#2A2A2A`
- `--border-active` `#3A3A3A`
- `--text` `#C8C8C8`
- `--text-muted` `#6A6A6A`
- `--text-faint` `#3E3E3E`
- `--text-bright` `#E8E8E8`
- `--accent` `#FF7832`
- `--accent-dim` `rgba(255, 120, 50, 0.15)`
- `--accent-hover` `#FF944D`
- `--accent-glow` `rgba(255, 120, 50, 0.08)`

### Light

- `--bg` `#F3F0EA`
- `--bg-elevated` `#FAF8F4`
- `--bg-surface` `#FFFFFF`
- `--bg-hover` `#EBE5DC`
- `--border` `#D6CFC3`
- `--border-active` `#BDB3A5`
- `--text` `#2D251D`
- `--text-muted` `#6C6256`
- `--text-faint` `#9F9384`
- `--text-bright` `#16110C`
- `--accent` `#D85F21`
- `--accent-dim` `rgba(216, 95, 33, 0.14)`
- `--accent-hover` `#EC7437`
- `--accent-glow` `rgba(216, 95, 33, 0.08)`

## Semantic Tokens

- `--success` `#4ADE80` / light `#2F8F4B`
- `--warning` `#FBBF24` / light `#B37A00`
- `--error` `#F87171` / light `#C84D4D`
- `--info` `#60A5FA` / light `#0D73C7`

## Spacing And Density

- Grid stays on the prototype's 4px rhythm:
  - `--space-1` `4px`
  - `--space-2` `8px`
  - `--space-3` `12px`
  - `--space-4` `16px`
  - `--space-5` `20px`
  - `--space-6` `24px`
  - `--space-8` `32px`
  - `--space-10` `40px`
  - `--space-12` `48px`
- Desktop shell density tokens:
  - `--layout-sidebar-width` `220px`
  - `--layout-topbar-height` `52px`
  - `--density-panel-padding` `20px`
  - `--density-content-gap` `20px`

## Motion

- Shared transition token: `--transition: 120ms ease`
- Motion remains restrained:
  - navigation hover / active state
  - shell and page fade-in
  - CTA state emphasis only when it improves affordance

## Implementation Files

- CSS variables: `src/styles/tokens.css`
- App shell styling: `src/styles/app.css`
- TypeScript token helpers: `src/lib/tokens.ts`

## Usage Rules

- New shell/page work should consume CSS variables directly or through `src/lib/tokens.ts`.
- Do not reintroduce per-page color constants into components.
- If a new feature needs another token, update this document and `src/styles/tokens.css` together.
