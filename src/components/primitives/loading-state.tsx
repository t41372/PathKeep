interface LoadingStateProps {
  label: string
  detail?: string | null
  progressLabel?: string | null
  progressValue?: number | null
  compact?: boolean
}

function clampedProgress(value: number | null | undefined) {
  if (value === null || value === undefined || Number.isNaN(value)) return null
  return Math.max(0, Math.min(100, Math.round(value)))
}

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
