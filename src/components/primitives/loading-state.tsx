/**
 * This module provides a shared primitive for loading, empty, error, permission, or trust-first shell states.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `LoadingState`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

/**
 * Describes the props accepted by `LoadingState`.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
interface LoadingStateProps {
  label: string
  detail?: string | null
  progressLabel?: string | null
  progressValue?: number | null
  compact?: boolean
}

/**
 * Explains how clamped progress works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
function clampedProgress(value: number | null | undefined) {
  if (value === null || value === undefined || Number.isNaN(value)) return null
  return Math.max(0, Math.min(100, Math.round(value)))
}

/**
 * Explains how loading state works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function LoadingState({
  label,
  detail,
  progressLabel,
  progressValue,
  compact = false,
}: LoadingStateProps) {
  const normalizedProgress = clampedProgress(progressValue)

  return (
    <div
      className={`loading-state ${compact ? 'loading-state--compact' : ''}`}
      role="status"
      aria-live="polite"
    >
      <div className="loading-state__spinner" aria-hidden="true">
        <span className="loading-state__block" />
        <span className="loading-state__block" />
        <span className="loading-state__block" />
      </div>
      <div className="loading-state__content">
        <span className="loading-state__label">{label}</span>
        {detail ? (
          <span className="loading-state__detail">{detail}</span>
        ) : null}
        {progressLabel || normalizedProgress !== null ? (
          <div className="loading-state__progress">
            <div className="loading-state__progress-meta">
              {progressLabel ? (
                <span className="loading-state__progress-label">
                  {progressLabel}
                </span>
              ) : (
                <span />
              )}
              {normalizedProgress !== null ? (
                <span className="loading-state__progress-value">
                  {normalizedProgress}%
                </span>
              ) : null}
            </div>
            {normalizedProgress !== null ? (
              <div className="loading-state__progress-track" aria-hidden="true">
                <span
                  className="loading-state__progress-fill"
                  style={{ width: `${Math.max(normalizedProgress, 4)}%` }}
                />
              </div>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  )
}
