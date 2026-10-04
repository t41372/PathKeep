# Design Tokens

> Source of truth: `src/index.css`. This page describes it; when the two disagree, the CSS wins and this page gets fixed.
> The palette comes from the 2026-10 Claude Design prototype (`docs/design/prototype-2026-10/`). It replaced the v0.3 "Paper + Archival" tokens (`src/styles/tokens.css`, Newsreader, 3 px radius), which were deleted with the old frontend.
> Fonts: [typography-and-font-fallback.md](./typography-and-font-fallback.md). Charts: [chart-primitive-tradeoff.md](./chart-primitive-tradeoff.md).

---

## How the layers fit

- Tokens are plain CSS variables on `:root` (light) and `.dark` (dark).
- They use the names shadcn/ui expects (`--background`, `--card`, `--primary`, `--muted-foreground`, …), so stock components in `src/components/ui/` pick them up without overrides.
- `@theme inline` in the same file maps each variable to a Tailwind v4 color/radius/shadow (`bg-card`, `text-muted-foreground`, `bg-brand`, `bg-heat-3`, `shadow-card`, …). Components use those utilities, not raw `var(--…)`.
- `components.json`: style `new-york`, base color `neutral`, CSS variables on, utils alias `@/lib/cn`, icon library `lucide`. The `@evilcharts` registry points at `https://evilcharts.com/r/{name}.json`.

## Theme

- `light`, `dark` or `system`, stored in `localStorage` under `pathkeep-theme` (default `system`). See `src/lib/theme.tsx`.
- Applied as the `.dark` class on `<html>` plus `color-scheme`, before React renders (`applyStoredTheme()` in `src/main.tsx`), so there is no flash.
- Chosen from the nav rail (sun/moon button), Settings → General → Appearance, or the ⌘K palette ("Toggle light / dark").

## Surfaces

The window is translucent glass over a gradient "wall":

| Token                     | Light                                                         | Dark                                                          | Used for                                                    |
| ------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------------- |
| `--wall`                  | radial gradient `#dfe9f7` → `#e9eef6` → `#f7e6dc` → `#f3d9cb` | radial gradient `#2a2840` → `#1a1a2a` → `#1f1622` → `#2a1a1c` | `body` background                                           |
| `--window`                | `rgb(250 250 252 / 0.62)`                                     | `rgb(24 24 34 / 0.62)`                                        | the app frame (`bg-window` + 40 px backdrop blur)           |
| `--panel`                 | `rgb(255 255 255 / 0.66)`                                     | `rgb(30 30 42 / 0.55)`                                        | the rounded panel the active screen sits in (`bg-panel`)    |
| `--background`            | `#f6f6f8`                                                     | `#17171f`                                                     | shadcn default background                                   |
| `--card`                  | `rgb(255 255 255 / 0.78)`                                     | `rgb(255 255 255 / 0.055)`                                    | cards, active nav item                                      |
| `--popover`               | `#ffffff`                                                     | `#22222e`                                                     | menus, popovers, dialogs                                    |
| `--muted` / `--secondary` | `rgb(15 15 30 / 0.045)`                                       | `rgb(255 255 255 / 0.05)`                                     | quiet fills: stat tiles, code blocks, segmented controls    |
| `--accent`                | `rgb(15 15 30 / 0.06)`                                        | `rgb(255 255 255 / 0.07)`                                     | hover fill (shadcn "accent" is a hover tint, not the brand) |

## Text and lines

| Token                | Light                  | Dark                      |
| -------------------- | ---------------------- | ------------------------- |
| `--foreground`       | `#0a0a0b`              | `#f4f4f5`                 |
| `--muted-foreground` | `#6b6b74`              | `#a1a1aa`                 |
| `--primary`          | `#18181b`              | `#f4f4f5`                 |
| `--border`           | `rgb(15 15 30 / 0.08)` | `rgb(255 255 255 / 0.08)` |
| `--input`            | `rgb(15 15 30 / 0.12)` | `rgb(255 255 255 / 0.12)` |
| `--ring`             | brand at 55 %          | brand at 55 %             |

`--primary` is near-black in light mode and near-white in dark: primary buttons are neutral, not orange. The brand color is kept for status, charts and selection.

There is no automated contrast check any more. The old `tokens.contrast.test.ts` went with the Paper tokens. If you change `--muted-foreground` or the card alpha, check WCAG AA (4.5:1) by hand against `--card` over the wall in both themes.

## Brand and status colors

| Token                     | Light                  | Dark                   | Meaning                                                             |
| ------------------------- | ---------------------- | ---------------------- | ------------------------------------------------------------------- |
| `--brand`                 | `oklch(0.7 0.17 48)`   | `oklch(0.74 0.16 50)`  | PathKeep orange: running state, chart series 1, heatmap, focus ring |
| `--brand-soft`            | brand at 12 %          | brand at 16 %          | text selection, soft highlights                                     |
| `--blue`                  | `oklch(0.62 0.15 252)` | `oklch(0.72 0.13 252)` | second series (e.g. searches), search-index segment in disk usage   |
| `--green`                 | `oklch(0.62 0.15 150)` | `oklch(0.72 0.13 150)` | "backed up recently" dot, success                                   |
| `--violet`                | `oklch(0.62 0.15 300)` | `oklch(0.72 0.13 300)` | snapshots segment in disk usage                                     |
| `--red` / `--destructive` | `oklch(0.6 0.2 25)`    | `oklch(0.68 0.18 25)`  | failures, destructive buttons, invalid input                        |

`--chart-1` … `--chart-5` alias brand, blue, green, violet and red, in that order. EvilCharts reads them through its `ChartConfig`.

## Heatmap scale

Five steps used by `src/components/app/heatmap.tsx` (Home's year calendar, Insights' weekly rhythm). `heatLevel()` in `heat-level.ts` buckets a count against the busiest cell: 0, < 15 %, < 40 %, < 70 %, rest.

| Token      | Light                  | Dark                      |
| ---------- | ---------------------- | ------------------------- |
| `--heat-0` | `rgb(15 15 30 / 0.06)` | `rgb(255 255 255 / 0.06)` |
| `--heat-1` | `oklch(0.9 0.05 52)`   | `oklch(0.38 0.07 48)`     |
| `--heat-2` | `oklch(0.82 0.1 50)`   | `oklch(0.5 0.11 48)`      |
| `--heat-3` | `oklch(0.74 0.15 48)`  | `oklch(0.62 0.15 48)`     |
| `--heat-4` | `oklch(0.64 0.18 44)`  | `oklch(0.76 0.16 52)`     |

In dark mode the scale runs from dim to bright, so "more" still reads as more.

## Shape, depth, motion

- Radius: `--radius: 0.75rem` (12 px). Tailwind gets `rounded-sm` = 8 px, `rounded-md` = 10 px, `rounded-lg` = 12 px, `rounded-xl` = 16 px. Cards and the main panel use `rounded-xl`; heatmap cells use 3 px.
- Shadows: `--shadow-card` (hairline lift for cards and the active nav item) and `--shadow-float` (popovers, the palette). Both have dark variants.
- Motion: `animate-rise` (220 ms, `cubic-bezier(0.2, 0.7, 0.2, 1)`, 6 px rise + fade) for screens, settings sections, empty states and the lock screen. Hover and color changes use 100–200 ms transitions. The History detail panel slides by animating its width over 200 ms and turns that off under `motion-reduce`. shadcn components bring their own enter/exit animations from `tw-animate-css`.
- Chat bubbles: `--user-bubble` / `--user-bubble-foreground` (inverted neutral).

## Base styles

- `body`: 14 px Geist, antialiased, `font-synthesis: none`, background `--wall`. `html`, `body`, `#root` fill the window and never scroll; each screen owns its scroll container.
- `.tabular` turns on tabular figures for counts and times.
- `::selection` uses `--brand-soft`.
- Every element defaults to `border-border` and `outline-ring/50`.

## Rules

- Use the Tailwind utilities above. Don't hard-code colors in components; the one exception is `favicon.tsx`, which derives a letter-badge hue from the domain.
- New tokens go into both `:root` and `.dark`, plus the `@theme inline` mapping, in `src/index.css`, and into this page.
- Don't edit files under `src/components/ui/` or `src/components/evilcharts/` to restyle them; pass classes or props. They are registry code we keep as shipped.

## Status

Accepted 2026-10-02 with the frontend redesign (M18). Replaces the 2026-05-19 Paper tokens.
