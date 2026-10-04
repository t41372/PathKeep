# Tradeoff — Chart rendering: a self-built SVG primitive vs. a charting library

> **Superseded 2026-10-02.** For the frontend redesign (M18) the user decided that charts
> use [EvilCharts](https://evilcharts.com), which is built on Recharts 3 (`docs/plan/STATUS.md`,
> "用戶要求"). The section below this note is the July decision, kept for the record; it no
> longer describes the code.
>
> What the code does now:
>
> - **Line, area and bar charts**: EvilCharts components added with
>   `npx shadcn add @evilcharts/...` and vendored under `src/components/evilcharts/`
>   (`charts/recharts-area-chart.tsx`, `charts/recharts-bar-chart.tsx`, `ui/recharts-*.tsx`).
>   They are registry code kept as shipped: lint relaxes the same rules it relaxes for
>   `src/components/ui/`, and we change their look through props (e.g. Home's 30-day trend
>   passes its own gradient via `areaProps`), not by editing the files. Colors come from
>   `--chart-1` … `--chart-5` (see [design-tokens.md](./design-tokens.md)).
>   Used by Home (30-day area trend) and Insights (daily pages / searches bars).
>   The History detail panel's 12-week bars are plain divs in `detail-stats.tsx`.
> - **Heatmaps**: EvilCharts has no heatmap and the amicro mono-chart one is a fixed-data demo,
>   so `src/components/app/heatmap.tsx` is our own small grid in the same orange scale
>   (`--heat-0` … `--heat-4`). Home's year calendar uses it with real dates (click a day →
>   `/history?date=YYYY-MM-DD`); Insights' rhythm uses it as weekday × hour. One shared
>   tooltip, portalled with fixed coordinates so the card edge can't clip it.
> - Recharts is lazy-loaded with the screens that use it; Vite emits it as one ~546 kB chunk
>   (named `heatmap` after the first module in it) shared by Home and Insights.
>
> Still true from the July decision: charts get pre-aggregated, small series from the backend
> and never bucket or sum on the render path; every chart says what it measures and over which
> window ([ui-review-guardrails.md](./ui-review-guardrails.md) §6).
>
> Not met today: heatmap cells are `<span>`s with no `role`, no per-cell `aria-label` and no
> keyboard path, so the year calendar's click-to-open-a-day works with a mouse only. The
> tooltip has `role="tooltip"` but nothing points at it.

> Status (July 2026): **Accepted (2026-07-05)** — Line 2 of the shadcn UI-craft adoption
> (`docs/plan/DISCUSSION-ui-craft-shadcn-gap.md`). Supersedes the stray
> `pk-contactsheet.jsx:396` design-comp note "implement with tailwind + shadcn recharts
> in production" — that was a mockup annotation, never a decision.

## Context

PathKeep's data-viz has no shared primitive, and what exists is either hand-rolled
div/CSS or one-off inline SVG:

- **Year heatmap** (`src/components/heatmap/year-heatmap.tsx`) — a CSS Grid of **365
  `<button>`/`<span>` cells**, no SVG, no scale math. Its 5-level fill is ad-hoc opacity
  fractions (`bg-accent/15`, `/35`, `/60`) instead of the documented accent ramp, and its
  accessibility **regressed** in the paper rewrite: cells carry only `title=` (not reliably
  announced) and the container lost the `role="grid"` + `aria-label` the pre-redesign
  version had.
- **Two hand-rolled inline-SVG sparklines**, duplicated: `HourlySparkline`
  (`explorer-paper/paper-day-insights.tsx`) and `DiscoverySparkline`
  (`intelligence/.../discovery-trend-section.tsx` + a `buildSparklinePath` helper).
- **A div percent-bar** (`.discovery-trend__bar`) still painted with legacy v0.2 vars
  (`--bg-elevated`, `--text-muted`) — a style island.
- `package.json` has **zero** charting libraries.

Binding constraints (AGENTS.md + `docs/design/`):

- **Supply chain** — a new JS dependency needs >6k★ / reputable maintainer, else a written
  risk assessment + explicit approval.
- **選長期最優解** — avoid heavy/temporary solutions; control complexity.
- **14.4M-row scale check** — the frontend must never aggregate on the render path.
- `ui-review-guardrails.md §6` — every chart states what it measures / window / scope /
  freshness and routes back to evidence; legend swatches use **opaque, real** semantic
  tokens (no invented tokens; nothing near-background in either theme).
- `screens-and-nav.md` — the Browsing-Rhythm calendar heatmap is an **accepted real-date
  contract**; Top Concepts must be a **horizontal bar chart**, not a word cloud.

## Options

| Option                                | For                                                                                                                                                                                                                                                                                         | Against                                                                                                                                                  |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Self-built SVG primitive (chosen)** | Total control of the paper aesthetic (accent ramp, `--radius-tight` cells, mono ticks); zero new deps → no supply-chain review; a11y is ours to get right (`role=grid`/`img` + aria); our shapes are simple (calendar / sparkline / horizontal bar); DOM SVG inherits the CSS theme tokens. | We write scale/axis ourselves — but the surface is small and the two existing sparklines already prove the pattern.                                      |
| Recharts (~24k★)                      | Mature, quick start.                                                                                                                                                                                                                                                                        | Bundles D3 (heavy); documented large-data perf pitfalls; default styling must be fought hard to become paper; opinionated API for shapes we barely need. |
| visx (Airbnb ~19k★)                   | Low-level primitives, paper-friendly.                                                                                                                                                                                                                                                       | Still a dep tree; more wiring than today's need warrants.                                                                                                |
| uPlot (~9k★)                          | Canvas; extreme series counts.                                                                                                                                                                                                                                                              | Canvas ≠ DOM → can't inherit the CSS paper aesthetic or a11y cleanly; overkill for ≤ few-thousand pre-aggregated points.                                 |

## Decision

**Self-build a small SVG chart primitive in `src/components/charts/`. No new npm dependency.**

Clearing the star bar is necessary but not sufficient — `選長期最優解` asks which is the best
_long-term fit_. For three simple, paper-styled, pre-aggregated chart shapes the answer is a
thin owned primitive, not a library whose defaults cost more to override than the charts cost
to build. Performance is a non-issue: aggregation is already precomputed server-side
(`daily_summary_rollups`), and the primitive only ever receives an already-small
(≤ few-thousand-point) series as props — it must never bucket/sum on its own render path.

## Scope

- **Shared pure geometry/scale helpers** (generalize the existing `buildSparklinePath`),
  unit-tested to 100%.
- **`CalendarHeatmap`** — real-date SVG calendar; consumes `--accent-soft/medium/strong` +
  `--radius-tight` (2px, the documented "heatmap cells" token); restores `role="grid"` +
  container `aria-label` + per-cell `aria-label`; **preserves the click-to-preview-day
  interaction and keyboard access** (interactive cells stay focusable/operable).
- **`Sparkline`** — SVG polyline/area; `--accent` stroke ~1.5px, low-opacity area, mono
  ticks; `role="img"` + `aria-label`. `HourlySparkline`/`DiscoverySparkline` migrate onto it.
- **Percent bar** — the legacy-var `.discovery-trend__bar` gets repainted onto paper tokens
  (kills the style island).

Every chart answers §6 (what/window/scope/freshness) via labels and keeps its drill-back to
evidence. Charts are static: no new `@keyframes`; any transition is transform/opacity only.

## Revisit trigger

If a genuinely complex, interactive, multi-series chart is later required, re-evaluate **visx**
(low-level, paper-compatible) in a fresh ADR — not recharts. This decision does not
pre-authorize any charting dependency.
