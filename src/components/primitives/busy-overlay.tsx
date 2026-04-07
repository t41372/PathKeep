interface BusyOverlayProps {
  label: string
}

export function BusyOverlay({ label }: BusyOverlayProps) {
  return (
    <div className="busy-overlay" role="status" aria-live="polite">
      <div className="busy-overlay__card">
        <span aria-hidden className="busy-overlay__spinner" />
        <span className="busy-overlay__label">{label}</span>
      </div>
    </div>
  )
}
