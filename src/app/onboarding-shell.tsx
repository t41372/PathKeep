import { Outlet } from 'react-router-dom'

export function OnboardingShell() {
  return (
    <div className="app-frame onboarding-frame" data-testid="onboarding-shell">
      <div aria-hidden className="shell-dot-grid" />
      <span aria-hidden className="corner-mark corner-mark--tl">
        +
      </span>
      <span aria-hidden className="corner-mark corner-mark--tr">
        +
      </span>
      <span aria-hidden className="corner-mark corner-mark--bl">
        +
      </span>
      <span aria-hidden className="corner-mark corner-mark--br">
        +
      </span>
      <div
        className="onboarding-content"
        style={{ position: 'relative', zIndex: 2 }}
      >
        <Outlet />
      </div>
    </div>
  )
}
