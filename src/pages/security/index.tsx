import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { LoadingState } from '../../components/primitives/loading-state'

export function SecurityPage() {
  const { loading, snapshot } = useShellData()

  if (loading && !snapshot) {
    return (
      <section className="page-shell">
        <LoadingState label="Loading security posture" />
      </section>
    )
  }

  if (!snapshot) {
    return (
      <section className="page-shell">
        <EmptyState
          description="PathKeep needs the local app snapshot before it can describe the current encryption and keyring posture."
          eyebrow="SECURITY"
          title="Security posture is unavailable"
        />
      </section>
    )
  }

  return (
    <section className="page-shell">
      <div className="content-grid">
        <section className="shell-panel shell-panel--accent">
          <div className="panel-header">
            <span className="panel-title">ARCHIVE MODE</span>
            <span className="panel-action">
              {snapshot.archiveStatus.encrypted ? 'Encrypted' : 'Plaintext'}
            </span>
          </div>
          <div className="panel-body stack-list">
            <article className="list-item">
              <strong>
                {snapshot.archiveStatus.encrypted
                  ? 'Master password required'
                  : 'No database password configured'}
              </strong>
              <span className="mono-support">
                {snapshot.archiveStatus.encrypted
                  ? 'The archive only unlocks when a valid database key is present in the session or keyring.'
                  : 'Choose encrypted mode in onboarding or rekey later if the archive should be protected at rest.'}
              </span>
            </article>
            <article className="list-item">
              <strong>Keyring status</strong>
              <span className="mono-support">
                {snapshot.keyringStatus.storedSecret
                  ? 'A stored unlock secret is available in the native keyring.'
                  : 'No unlock secret is stored in the native keyring.'}
              </span>
            </article>
            <article className="list-item">
              <strong>Recovery boundary</strong>
              <span className="mono-support">
                Forgetting the password means losing access to the encrypted
                archive. PathKeep keeps that warning visible before the first
                run.
              </span>
            </article>
          </div>
        </section>

        <aside className="stacked-column">
          <section className="shell-panel">
            <div className="panel-header">
              <span className="panel-title">DAY-ONE CHECKLIST</span>
              <span className="panel-action">Manual review stays explicit</span>
            </div>
            <div className="panel-body stack-list">
              <article className="list-item">
                <strong>Archive path</strong>
                <span className="mono-support">
                  {snapshot.directories.archiveDatabasePath}
                </span>
              </article>
              <article className="list-item">
                <strong>Session status</strong>
                <span className="mono-support">
                  {snapshot.archiveStatus.unlocked
                    ? 'Archive is currently unlocked for local read operations.'
                    : 'Archive is currently locked. Explorer and Audit remain read-blocked until unlock.'}
                </span>
              </article>
            </div>
          </section>

          <section className="shell-panel">
            <div className="panel-header">
              <span className="panel-title">NEXT STEP</span>
              <span className="panel-action">Trust copy first</span>
            </div>
            <div className="panel-body">
              <p className="dashboard-next-action">
                Use onboarding to review the first-run security choice, then
                come back here once the dedicated rekey flow lands.
              </p>
              <div className="utility-block__actions">
                <Link className="ghost-button" to="/onboarding">
                  Review onboarding
                </Link>
                <Link className="primary-button" to="/audit">
                  Inspect audit trail
                </Link>
              </div>
            </div>
          </section>
        </aside>
      </div>
    </section>
  )
}
