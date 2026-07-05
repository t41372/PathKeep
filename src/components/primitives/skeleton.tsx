/**
 * This module provides the shared paper-token loading-skeleton primitive plus
 * the two route-level compositions that use it.
 *
 * Why this file exists:
 * - A single `Skeleton` box keeps every loading placeholder on the same
 *   paper-token animation instead of each route hand-rolling its own
 *   `animate-pulse` div (inconsistent motion, inconsistent color).
 * - `DashboardSkeleton` / `SkeletonExplorer` compose that box into the exact
 *   Tailwind shapes their real routes render, so the swap from skeleton to
 *   real content never shifts the layout (the defect recorded in
 *   `docs/review/2026-06-14/phase-1/all-reports.json`, "legacy v0.2 CSS class
 *   names").
 *
 * Main declarations:
 * - `Skeleton`
 * - `DashboardSkeleton`
 * - `SkeletonExplorer`
 *
 * Source-of-truth notes:
 * - Animation + color rules come from `docs/design/ux-principles.md` §4
 *   (match final layout dims, `var(--border)`-family color, ~1.5s
 *   ease-in-out pulse, reduced-motion → static/near-static).
 * - The single canonical keyframe (`pk-skeleton-pulse`) is defined once in
 *   `src/styles/paper.css` and reduced-motion-gated globally there; this file
 *   must never declare a new `@keyframes`.
 * - `DashboardSkeleton` mirrors `src/pages/dashboard/index.tsx`;
 *   `SkeletonExplorer` mirrors the Browse contact-sheet shapes in
 *   `src/components/explorer-paper/paper-day-header.tsx` and
 *   `paper-list-row.tsx`.
 */

import type { CSSProperties, ReactNode } from 'react'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import { cn } from '@/lib/cn'

/**
 * Describes the props accepted by `Skeleton`.
 *
 * There is deliberately no `width`/`height`/`variant` enum: callers compose
 * the exact shape they need with Tailwind sizing classes (matching the real
 * content they stand in for) instead of picking from a fixed set of canned
 * shapes.
 */
interface SkeletonProps {
  className?: string
  style?: CSSProperties
}

/**
 * A single shimmering placeholder box.
 *
 * Purely decorative, so it's `aria-hidden` — the loading region it lives in
 * carries `aria-busy` + an accessible label instead. Uses the one existing
 * `pk-skeleton-pulse` keyframe (opacity-only, so it's compositor-only and
 * never triggers layout) at the ux-principles-specified ~1.5s ease-in-out;
 * the global `prefers-reduced-motion` rule in `paper.css` clamps it to a
 * single non-looping frame automatically.
 */
export function Skeleton({ className, style }: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={cn(
        'bg-border-light/60 rounded-paper animate-[pk-skeleton-pulse_1.5s_ease-in-out_infinite]',
        className,
      )}
      style={style}
    />
  )
}

/**
 * Describes the props accepted by `DashboardSkeleton` and `SkeletonExplorer`.
 */
interface LabeledSkeletonProps {
  label: string
}

interface DashboardCardSkeletonProps {
  testId: string
  accent?: boolean
  bodyClassName?: string
  children: ReactNode
}

/**
 * One `PaperCard` shell with skeleton title/badge + a caller-supplied body.
 *
 * Reusing the real `PaperCard` / `PaperCardHeader` / `PaperCardBody`
 * primitives (rather than hand-copying their border/padding/radius) is what
 * makes the dashboard card skeletons match the real cards' dimensions
 * exactly — they're the same shell, only the content is still loading.
 */
function DashboardCardSkeleton({
  testId,
  accent,
  bodyClassName = 'px-[18px] py-[14px]',
  children,
}: DashboardCardSkeletonProps) {
  return (
    <PaperCard accent={accent} testId={testId}>
      <PaperCardHeader
        title={<Skeleton className="h-[12px] w-[130px]" />}
        right={<Skeleton className="h-[10px] w-[60px]" />}
      />
      <PaperCardBody className={bodyClassName}>{children}</PaperCardBody>
    </PaperCard>
  )
}

/**
 * Dashboard route-level skeleton.
 *
 * Mirrors `DashboardPage`'s ready-state Tailwind structure exactly: the same
 * `mx-auto max-w-[1080px]` wrapper, the same hero-band grid, the same
 * `grid-cols-1 lg:grid-cols-2` / `lg:grid-cols-3` card grids, and each card
 * as a real `PaperCard` shell — so there is no layout shift when
 * `DashboardRouteFallback` swaps this out for the populated page.
 */
export function DashboardSkeleton({ label }: LabeledSkeletonProps) {
  return (
    <div
      className="mx-auto flex w-full max-w-[1080px] flex-col pt-7"
      aria-busy="true"
      aria-label={label}
    >
      <header className="border-border-light mb-7 grid grid-cols-1 items-end gap-10 border-b pb-5 lg:grid-cols-[1fr_auto]">
        <div>
          <Skeleton className="mb-[6px] h-[31.2px] w-[240px]" />
          <Skeleton className="h-[22.5px] w-[70%] max-w-[420px]" />
        </div>
        <div className="flex flex-wrap items-end gap-7">
          {Array.from({ length: 4 }, (_, index) => (
            <div key={index} className="text-center">
              <Skeleton className="mx-auto mb-1 h-[24px] w-[52px]" />
              <Skeleton className="mx-auto h-[9px] w-[64px]" />
            </div>
          ))}
        </div>
      </header>

      <div className="mb-4 grid grid-cols-1 gap-4 lg:grid-cols-2">
        <DashboardCardSkeleton accent testId="dashboard-on-this-day-skeleton">
          <Skeleton className="h-[160px] w-full" />
          <Skeleton className="mt-2 h-[9px] w-1/2" />
        </DashboardCardSkeleton>
        <DashboardCardSkeleton testId="dashboard-this-week-skeleton">
          <Skeleton className="mb-2 h-[14px] w-3/4" />
          <Skeleton className="h-[13px] w-1/2" />
          <div className="border-border-light mt-[14px] border-t border-dashed pt-3">
            <Skeleton className="h-[42px] w-full" />
          </div>
        </DashboardCardSkeleton>
      </div>

      <PaperCard className="mb-4" testId="dashboard-year-heatmap-skeleton">
        <PaperCardHeader
          compact
          title={<Skeleton className="h-[12px] w-[130px]" />}
          right={<Skeleton className="h-[10px] w-[70px]" />}
        />
        <PaperCardBody className="px-[18px] pb-[14px] pt-[10px]">
          <Skeleton className="h-[100px] w-full" />
        </PaperCardBody>
      </PaperCard>

      <div className="mb-4 grid grid-cols-1 gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <DashboardCardSkeleton
            testId="dashboard-active-threads-skeleton"
            bodyClassName="px-[18px] pt-1 pb-[14px]"
          >
            <div className="flex flex-col gap-[10px]">
              {Array.from({ length: 3 }, (_, index) => (
                <Skeleton key={index} className="h-[56px] w-full" />
              ))}
            </div>
          </DashboardCardSkeleton>
        </div>
        <DashboardCardSkeleton
          testId="dashboard-archive-card-skeleton"
          bodyClassName="px-4 py-3"
        >
          <Skeleton className="mb-2.5 h-[28px] w-full" />
          <Skeleton className="mb-2.5 h-[10px] w-2/3" />
          <div className="flex flex-col">
            {Array.from({ length: 3 }, (_, index) => (
              <Skeleton key={index} className="h-[26px] w-full" />
            ))}
          </div>
          <div className="border-border-light mt-2.5 border-t pt-2">
            <Skeleton className="h-[34px] w-1/2" />
          </div>
          <div className="mt-2.5 flex gap-1">
            <Skeleton className="h-[26px] flex-1" />
            <Skeleton className="h-[26px] flex-1" />
          </div>
        </DashboardCardSkeleton>
      </div>

      <div className="border-border-light mt-6 border-t pb-2 pt-5 text-center">
        <Skeleton className="mx-auto h-[13px] w-[260px]" />
      </div>
    </div>
  )
}

/**
 * Explorer route-level skeleton.
 *
 * Stands in for the default Browse contact-sheet shape while the shell is
 * loading and no snapshot has resolved yet (so we don't yet know which
 * surface — Browse, Search, Starred — the user will land on). Mirrors:
 * - the real route's outer `page-shell explorer-page` wrapper;
 * - the contact-sheet's sticky toolbar (`h-[44px]` day-nav + view-toggle row,
 *   `-mx-7 px-7` full-bleed) in `paper-contact-sheet.tsx`;
 * - the filter-strip row's `flex flex-wrap items-center gap-x-3 gap-y-1.5`
 *   shape (`PaperFilterStrip`'s wrapper in `explorer/index.tsx`);
 * - one sticky day header's classes, including its `-mx-7 px-7` full-bleed
 *   wrapper (`PaperDayHeader`);
 * - several list rows' `grid-cols-[26px_1fr_auto]` classes (`PaperListRow`).
 *
 * The toolbar, day header, and rows are wrapped in a single `relative flex
 * w-full flex-col` div — the same root class `PaperContactSheet` renders —
 * so they're the `.page-shell` grid's *one* child instead of three siblings.
 * `.page-shell` is `display:grid; gap:var(--space-4)` (16px), so three direct
 * grid-item children would pick up two 16px inter-section gaps the real
 * contact sheet never has. Combined with reserving the toolbar's ~44px up
 * front, this keeps the day header and rows at the same y-offset once the
 * shell resolves into the real `PaperContactSheet`, so there's no visible
 * downward jump.
 */
export function SkeletonExplorer({ label }: LabeledSkeletonProps) {
  return (
    <section
      className="page-shell explorer-page"
      aria-busy="true"
      aria-label={label}
    >
      {/* Mirrors `PaperContactSheet`'s own root (`relative flex w-full
          flex-col`, no gap) so this is the `.page-shell` grid's only child.
          Without this wrapper the toolbar/day-header/rows below would be
          three direct grid items and pick up the grid's 16px `gap-4` between
          each of them — space the real contact sheet never has. */}
      <div className="relative flex w-full flex-col">
        <div className="-mx-7 flex flex-col gap-1.5 px-7 pb-1">
          <div className="flex h-[44px] items-center justify-between gap-4">
            <Skeleton className="h-7 w-[190px]" />
            <div className="flex items-center gap-2">
              <Skeleton className="h-[9px] w-8" />
              <Skeleton className="h-6 w-16" />
            </div>
          </div>
          <div className="flex flex-wrap items-center justify-between gap-3 pb-1 pt-0.5">
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
              <Skeleton className="h-6 w-24" />
              <Skeleton className="h-6 w-20" />
              <Skeleton className="h-6 w-16" />
            </div>
            <Skeleton className="h-6 w-28" />
          </div>
        </div>

        <div className="border-border-default -mx-7 flex items-baseline justify-between gap-4 border-b-[2px] px-7 py-[14px] pb-[10px]">
          <div className="flex min-w-0 flex-1 flex-wrap items-baseline gap-3">
            <Skeleton className="h-[22px] w-[150px]" />
            <Skeleton className="h-[12.5px] w-[110px]" />
          </div>
          <Skeleton className="h-[10px] w-[46px] shrink-0" />
        </div>

        <div className="flex flex-col">
          {Array.from({ length: 7 }, (_, index) => (
            <div
              key={index}
              className="border-border-light grid w-full grid-cols-[26px_1fr_auto] items-center gap-[10px] border-b px-1 py-[7px]"
            >
              <Skeleton className="h-6 w-6 rounded-[6px]" />
              <div className="flex min-w-0 items-baseline gap-2">
                <Skeleton className="h-[12.5px] w-2/3" />
                <Skeleton className="h-[10px] w-10 shrink-0" />
              </div>
              <Skeleton className="h-[10px] w-10" />
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}
