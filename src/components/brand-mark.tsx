/**
 * This module renders the PathKeep brand mark used by the desktop shell and onboarding flows.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `BrandMark`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

import pathkeepMarkUrl from '../assets/pathkeep-mark.svg'

/**
 * Describes the props accepted by `BrandMark`.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
interface BrandMarkProps {
  alt?: string
  className?: string
}

/**
 * Explains how brand mark works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function BrandMark({
  alt = 'PathKeep',
  className = '',
}: BrandMarkProps) {
  return (
    <img
      alt={alt}
      className={className ? `brand-mark ${className}` : 'brand-mark'}
      src={pathkeepMarkUrl}
    />
  )
}
