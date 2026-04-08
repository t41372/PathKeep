interface LoadingStateProps {
  label: string
}

export function LoadingState({ label }: LoadingStateProps) {
  return (
    <div className="loading-state" role="status">
      <div className="loading-state__spinner" aria-hidden="true">
        <span className="loading-state__block" />
        <span className="loading-state__block" />
        <span className="loading-state__block" />
      </div>
      <span className="loading-state__label">{label}</span>
    </div>
  )
}
