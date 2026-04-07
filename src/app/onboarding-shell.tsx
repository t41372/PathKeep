import { Link, Outlet } from 'react-router-dom'

export function OnboardingShell() {
  return (
    <div className="app-frame onboarding-frame" data-testid="onboarding-shell">
      <div aria-hidden className="shell-dot-grid" />
      <header className="onboarding-header">
        <div className="onboarding-brand">
          <span className="mono-kicker">PATHKEEP</span>
          <h1>First archive decisions stay visible.</h1>
          <p>
            Before PathKeep touches a browser database, the app should show what
            it found, why it matters, and how to roll the change back.
          </p>
        </div>
        <div className="onboarding-actions">
          <Link className="ghost-button" to="/">
            Skip onboarding
          </Link>
        </div>
      </header>
      <main className="onboarding-scroll">
        <Outlet />
      </main>
    </div>
  )
}
