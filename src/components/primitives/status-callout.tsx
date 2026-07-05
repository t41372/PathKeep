/**
 * This module provides a shared primitive for loading, empty, error, permission, or trust-first shell states.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `StatusCallout`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

import type { ReactNode } from 'react'

/**
 * Describes the props accepted by `StatusCallout`.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
interface StatusCalloutProps {
  tone: 'info' | 'warning' | 'danger' | 'blocked' | 'success'
  title: string
  body?: string
  eyebrow?: string
  actions?: ReactNode
  /**
   * Optional ARIA live-region role. Omit (default) for static callouts so
   * existing call sites are unchanged. Pass `"status"` (polite) or `"alert"`
   * (assertive) when the callout appears/updates in response to a user action
   * and assistive tech must announce it — e.g. an empty/denied state revealed
   * after a re-check.
   */
  role?: 'status' | 'alert'
}

/**
 * Explains how status callout works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function StatusCallout({
  tone,
  title,
  body,
  eyebrow,
  actions,
  role,
}: StatusCalloutProps) {
  return (
    <section className={`status-callout status-callout--${tone}`} role={role}>
      {eyebrow ? <p className="mono-kicker">{eyebrow}</p> : null}
      <h3>{title}</h3>
      {body ? <p>{body}</p> : null}
      {actions ? <div className="utility-block__actions">{actions}</div> : null}
    </section>
  )
}
