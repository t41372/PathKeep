interface LoadingStateProps {
  label: string
}

export function LoadingState({ label }: LoadingStateProps) {
  return (
    <div className="status-panel" role="status">
      <span aria-hidden className="status-dot status-dot--accent" />
      <span>{label}</span>
    </div>
  )
}
