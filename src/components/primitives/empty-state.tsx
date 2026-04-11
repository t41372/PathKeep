/**
 * This module provides a shared primitive for loading, empty, error, permission, or trust-first shell states.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `EmptyState`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

import type { ReactNode } from 'react'

/**
 * Describes the props accepted by `EmptyState`.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
interface EmptyStateProps {
  eyebrow: string
  title: string
  description: string
  action?: ReactNode
}

/**
 * Explains how empty state works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function EmptyState({
  eyebrow,
  title,
  description,
  action,
}: EmptyStateProps) {
  return (
    <section className="utility-block" data-testid="empty-state">
      <span className="mono-kicker">{eyebrow}</span>
      <h2>{title}</h2>
      <p>{description}</p>
      {action ? <div className="utility-block__actions">{action}</div> : null}
    </section>
  )
}
