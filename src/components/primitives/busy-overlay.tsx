/**
 * This module provides a shared primitive for loading, empty, error, permission, or trust-first shell states.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `BusyOverlay`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

/**
 * Describes the props accepted by `BusyOverlay`.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
interface BusyOverlayProps {
  label: string
  detail?: string | null
  progressLabel?: string | null
  progressValue?: number | null
  steps?: string[]
  activeStep?: number
  logLines?: string[]
}

/**
 * Explains how busy overlay works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function BusyOverlay({
  label,
  detail,
  progressLabel,
  progressValue,
  steps,
  activeStep,
  logLines,
}: BusyOverlayProps) {
  const normalizedProgress =
    progressValue === null ||
    progressValue === undefined ||
    Number.isNaN(progressValue)
      ? null
      : Math.max(0, Math.min(100, Math.round(progressValue)))

  return (
    <div className="busy-overlay" role="status" aria-live="polite">
      <div className="busy-overlay__card">
        <div className="busy-overlay__spinner" aria-hidden="true">
          <span className="busy-overlay__bar" />
          <span className="busy-overlay__bar" />
          <span className="busy-overlay__bar" />
          <span className="busy-overlay__bar" />
          <span className="busy-overlay__bar" />
        </div>
        <div className="busy-overlay__content">
          <span className="busy-overlay__label">{label}</span>
          {detail ? (
            <span className="busy-overlay__detail">{detail}</span>
          ) : null}
          {progressLabel || normalizedProgress !== null ? (
            <div className="busy-overlay__progress">
              <div className="busy-overlay__progress-meta">
                {progressLabel ? (
                  <span className="busy-overlay__progress-label">
                    {progressLabel}
                  </span>
                ) : (
                  <span />
                )}
                {normalizedProgress !== null ? (
                  <span className="busy-overlay__progress-value">
                    {normalizedProgress}%
                  </span>
                ) : null}
              </div>
              {normalizedProgress !== null ? (
                <div
                  className="busy-overlay__progress-track"
                  aria-hidden="true"
                >
                  <span
                    className="busy-overlay__progress-fill"
                    style={{ width: `${Math.max(normalizedProgress, 4)}%` }}
                  />
                </div>
              ) : null}
            </div>
          ) : null}
          {steps?.length ? (
            <div className="busy-overlay__steps">
              {steps.map((step, index) => {
                const status =
                  activeStep === undefined
                    ? 'pending'
                    : index < activeStep
                      ? 'done'
                      : index === activeStep
                        ? 'active'
                        : 'pending'

                return (
                  <div
                    key={step}
                    className={`busy-overlay__step busy-overlay__step--${status}`}
                  >
                    <span
                      className="busy-overlay__step-marker"
                      aria-hidden="true"
                    />
                    <span>{step}</span>
                  </div>
                )
              })}
            </div>
          ) : null}
          {logLines?.length ? (
            <div className="busy-overlay__steps">
              {logLines.slice(-4).map((line) => (
                <div
                  key={line}
                  className="busy-overlay__step busy-overlay__step--active"
                >
                  <span
                    className="busy-overlay__step-marker"
                    aria-hidden="true"
                  />
                  <span className="mono-support">{line}</span>
                </div>
              ))}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  )
}
