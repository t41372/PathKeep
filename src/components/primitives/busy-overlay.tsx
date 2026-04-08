interface BusyOverlayProps {
  label: string
  detail?: string | null
  steps?: string[]
  activeStep?: number
}

export function BusyOverlay({
  label,
  detail,
  steps,
  activeStep,
}: BusyOverlayProps) {
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
