import { Link } from 'react-router-dom'
import { onboardingSteps } from '../../app/preview-data'
import { PermissionGate } from '../../components/primitives/permission-gate'

export function OnboardingPage() {
  return (
    <section
      className="page-shell onboarding-page"
      data-testid="onboarding-page"
    >
      <div className="onboarding-grid">
        <section className="shell-panel shell-panel--accent">
          <div className="panel-header">
            <span className="panel-title">ONBOARDING / SETUP</span>
            <span className="panel-action">Trust before automation</span>
          </div>
          <div className="panel-body">
            <h2>Onboarding / Setup</h2>
            <p>
              Work through source discovery, archive storage, schedule preview,
              and final review. Every mutating step keeps Preview, Manual, and
              Execute visible.
            </p>
            <div className="pme-grid">
              {onboardingSteps.map((step) => (
                <article key={step.title} className="pme-column">
                  <span className="mono-kicker">{step.title}</span>
                  <p>{step.detail}</p>
                </article>
              ))}
            </div>
          </div>
        </section>

        <PermissionGate
          detail="PathKeep should not install a scheduler or request archive access until the artifact and destination path have both been reviewed."
          eyebrow="NEXT DECISION"
          title="Native schedule preview stays manual-first"
        >
          <button className="ghost-button" type="button">
            Preview native schedule
          </button>
          <Link className="primary-button" to="/">
            Open dashboard preview
          </Link>
        </PermissionGate>
      </div>
    </section>
  )
}
