interface BusyOverlayProps {
  label: string
}

export function BusyOverlay({ label }: BusyOverlayProps) {
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
        <span className="busy-overlay__label">{label}</span>
      </div>
    </div>
  )
}
