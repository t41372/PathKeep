/**
 * This module provides a shared primitive for loading, empty, error, permission, or trust-first shell states.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `ErrorState`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

import type { ReactNode } from 'react'

/**
 * Describes the props accepted by `ErrorState`.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
interface ErrorStateProps {
  title: string
  description: string
  eyebrow?: string
  action?: ReactNode
}

/**
 * Explains how error state works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function ErrorState({
  title,
  description,
  eyebrow,
  action,
}: ErrorStateProps) {
  return (
    <section className="utility-block utility-block--danger" role="alert">
      {eyebrow ? <span className="mono-kicker">{eyebrow}</span> : null}
      <h2>{title}</h2>
      <p>{description}</p>
      {action ? <div className="utility-block__actions">{action}</div> : null}
    </section>
  )
}
