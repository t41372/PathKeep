interface BusyOverlayProps {
  label: string
  detail?: string | null
  progressLabel?: string | null
  progressValue?: number | null
  steps?: string[]
  activeStep?: number
}

export function BusyOverlay({
  label,
  detail,
  progressLabel,
  progressValue,
  steps,
  activeStep,
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
        </div>
      </div>
    </div>
  )
}
