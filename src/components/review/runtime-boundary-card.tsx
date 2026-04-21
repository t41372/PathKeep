/**
 * @file runtime-boundary-card.tsx
 * @description Renders a shared review-row shell for runtime-boundary summaries across Jobs and Settings.
 * @module components/review
 *
 * ## Responsibilities
 * - Keep plugin/module runtime review cards on the same header, metric-row, note, and action grammar.
 * - Let Jobs and Settings share one runtime-boundary presentation shell without sharing route-level loading or mutation state.
 * - Preserve the app-wide review-surface contract while adding a domain-specific card for runtime diagnostics.
 *
 * ## Non-Responsibilities
 * - Does not fetch runtime snapshots or interpret queue freshness policy.
 * - Does not localize labels or decide which metrics a consumer should show.
 * - Does not own retry, toggle, or navigation side effects.
 *
 * ## Dependencies
 * - Depends on `ReviewSection` as the canonical app-wide review shell.
 * - Reuses existing review/detail CSS classes so consumers do not need a parallel card layout.
 *
 * ## Performance Notes
 * - This component is render-only and expects callers to pass already-derived display values.
 */

import type { ReactNode } from 'react'
import { ReviewSection } from './review-surface'

/**
 * Describes one labeled metric row inside a shared runtime-boundary card.
 *
 * Runtime diagnostics often need a small set of high-signal facts such as
 * queue counts, derived tables, or last-built timestamps. This typed row keeps
 * that structure explicit instead of letting each consumer hand-roll its own
 * `config-row` mapping logic.
 */
export interface ReviewRuntimeBoundaryMetric {
  label: ReactNode
  value: ReactNode
  valueClassName?: string
}

/**
 * Describes the props accepted by `ReviewRuntimeBoundaryCard`.
 *
 * The goal is to share the review shell around runtime/plugin/module facts
 * while leaving each route in control of the actual metrics, notes, and
 * actions it needs to expose.
 */
export interface ReviewRuntimeBoundaryCardProps {
  actions?: ReactNode
  active?: boolean
  className?: string
  description?: ReactNode
  headerMeta?: ReactNode
  metrics?: ReviewRuntimeBoundaryMetric[]
  notes?: ReactNode
  title: ReactNode
}

/**
 * Renders one shared runtime-boundary review card.
 *
 * Jobs uses this for lightweight runtime health summaries, while Settings uses
 * the same shell for richer deterministic/enrichment review cards. Sharing
 * this chrome keeps runtime-boundary surfaces aligned without forcing the
 * routes to share business logic.
 */
export function ReviewRuntimeBoundaryCard({
  actions,
  active = false,
  className,
  description,
  headerMeta,
  metrics = [],
  notes,
  title,
}: ReviewRuntimeBoundaryCardProps) {
  return (
    <ReviewSection
      active={active}
      className={className}
      headerMeta={headerMeta}
      title={title}
    >
      {description ? <p>{description}</p> : null}
      {metrics.map((metric, index) => (
        <div className="config-row" key={`${index}`}>
          <span className="config-label">{metric.label}</span>
          <span
            className={
              metric.valueClassName
                ? `config-value ${metric.valueClassName}`
                : 'config-value'
            }
          >
            {metric.value}
          </span>
        </div>
      ))}
      {notes}
      {actions ? <div className="settings-action-row">{actions}</div> : null}
    </ReviewSection>
  )
}
