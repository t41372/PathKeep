import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { formatBytes, formatRelativeTime } from '../../lib/format'

export function DashboardPage() {
  const { dashboard, error, loading, snapshot } = useShellData()

  if (loading && !dashboard) {
    return (
      <section className="page-shell" data-testid="dashboard-page">
        <LoadingState label="Loading archive overview" />
      </section>
    )
  }

  if (error && !dashboard) {
    return (
      <section className="page-shell" data-testid="dashboard-page">
        <ErrorState
          title="Dashboard could not read the archive"
          description={error}
        />
      </section>
    )
  }

  if (!snapshot || !dashboard) {
    return (
      <section className="page-shell" data-testid="dashboard-page">
        <ErrorState
          title="Dashboard data is unavailable"
          description="PathKeep could not load the current archive snapshot."
        />
      </section>
    )
  }

  const stats = [
    {
      label: 'ARCHIVE HEALTH',
      value: snapshot.archiveStatus.warning ? 'Attention' : 'Ready',
      detail:
        snapshot.archiveStatus.warning ??
        'Manifest chain and archive shell are readable.',
      tone: snapshot.archiveStatus.warning ? 'warning' : 'success',
    },
    {
      label: 'LAST BACKUP',
      value: formatRelativeTime(dashboard.lastSuccessfulBackupAt),
      detail:
        dashboard.recentRuns[0]?.manifestHash ??
        'No successful backup has written a manifest yet.',
      tone: dashboard.lastSuccessfulBackupAt ? 'accent' : 'neutral',
    },
    {
      label: 'PROFILES SELECTED',
      value: `${snapshot.config.selectedProfileIds.length}`,
      detail:
        snapshot.config.selectedProfileIds.join(' · ') ||
        'Choose at least one Chromium profile in onboarding.',
      tone:
        snapshot.config.selectedProfileIds.length > 0 ? 'neutral' : 'warning',
    },
    {
      label: 'ARCHIVE STORAGE',
      value: formatBytes(dashboard.storage.archiveDatabaseBytes),
      detail: `${formatBytes(dashboard.storage.snapshotBytes)} snapshots · ${formatBytes(
        dashboard.storage.manifestBytes,
      )} manifests`,
      tone: 'accent',
    },
  ] as const

  return (
    <section className="page-shell" data-testid="dashboard-page">
      {snapshot.archiveStatus.warning ? (
        <section className="shell-panel shell-panel--warning">
          <div className="panel-header">
            <span className="panel-title">TRUST STATUS</span>
            <span className="panel-action">Review before the next run</span>
          </div>
          <div className="panel-body">
            <h2>Archive attention needed</h2>
            <p>{snapshot.archiveStatus.warning}</p>
          </div>
        </section>
      ) : null}

      <div className="stats-row">
        {stats.map((stat) => (
          <article
            key={stat.label}
            className="stat-block"
            data-tone={stat.tone}
          >
            <span className="stat-label">{stat.label}</span>
            <strong className="stat-value">{stat.value}</strong>
            <span className="stat-detail">{stat.detail}</span>
          </article>
        ))}
      </div>

      {!snapshot.config.initialized || dashboard.recentRuns.length === 0 ? (
        <EmptyState
          action={
            <Link className="primary-button" to="/onboarding">
              Open onboarding flow
            </Link>
          }
          description={
            dashboard.nextAction ??
            'Dashboard cards stay empty until the archive is initialized and the first manual backup finishes.'
          }
          eyebrow="DAY-ONE"
          title="The first archive run still needs review"
        />
      ) : (
        <div className="content-grid">
          <section className="shell-panel">
            <div className="panel-header">
              <span className="panel-title">RECENT RUNS</span>
              <Link className="panel-action panel-link" to="/audit">
                Open audit ledger
              </Link>
            </div>
            <div className="panel-body">
              <table className="shell-table">
                <thead>
                  <tr>
                    <th>RUN</th>
                    <th>STATUS</th>
                    <th>PROFILES</th>
                    <th>VISITS</th>
                    <th>TIME</th>
                    <th>ACTION</th>
                  </tr>
                </thead>
                <tbody>
                  {dashboard.recentRuns.map((run) => (
                    <tr key={run.id}>
                      <td className="mono-cell">#{run.id}</td>
                      <td>{run.status}</td>
                      <td className="mono-cell">{run.profilesProcessed}</td>
                      <td className="mono-cell">{run.newVisits}</td>
                      <td className="mono-cell">
                        {formatRelativeTime(run.finishedAt ?? run.startedAt)}
                      </td>
                      <td>
                        <Link
                          className="table-link"
                          to={`/audit?run=${run.id}`}
                        >
                          Detail
                        </Link>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <aside className="stacked-column">
            <section className="shell-panel shell-panel--accent">
              <div className="panel-header">
                <span className="panel-title">ARCHIVE COVERAGE</span>
                <span className="panel-action">Manual archive baseline</span>
              </div>
              <div className="panel-body stack-list">
                <article className="list-item">
                  <strong>
                    {dashboard.totalVisits.toLocaleString()} visible visits
                  </strong>
                  <span className="mono-support">
                    {dashboard.totalUrls.toLocaleString()} URLs tracked across{' '}
                    {dashboard.totalProfiles} selected Chromium profiles
                  </span>
                </article>
                <article className="list-item">
                  <strong>
                    {snapshot.archiveStatus.encrypted
                      ? 'Encrypted'
                      : 'Plaintext'}{' '}
                    vault
                  </strong>
                  <span className="mono-support">
                    {snapshot.keyringStatus.storedSecret
                      ? 'Keyring has a stored unlock secret.'
                      : 'No unlock secret is stored in the keyring.'}
                  </span>
                </article>
                <article className="list-item">
                  <strong>
                    {snapshot.browserProfiles.length} browser sources detected
                  </strong>
                  <span className="mono-support">
                    Chromium profiles are ready now; Firefox and Safari stay
                    preview-only in M1.
                  </span>
                </article>
              </div>
            </section>

            <section className="shell-panel">
              <div className="panel-header">
                <span className="panel-title">STORAGE SUMMARY</span>
                <span className="panel-action">
                  {formatBytes(dashboard.storage.exportBytes)} exports
                </span>
              </div>
              <div className="panel-body stack-list">
                <article className="list-item">
                  <strong>Archive database</strong>
                  <span className="mono-support">
                    {formatBytes(dashboard.storage.archiveDatabaseBytes)}
                  </span>
                </article>
                <article className="list-item">
                  <strong>Snapshot checkpoints</strong>
                  <span className="mono-support">
                    {formatBytes(dashboard.storage.snapshotBytes)}
                  </span>
                </article>
                <article className="list-item">
                  <strong>Manifest ledger</strong>
                  <span className="mono-support">
                    {formatBytes(dashboard.storage.manifestBytes)}
                  </span>
                </article>
              </div>
            </section>

            <section className="shell-panel">
              <div className="panel-header">
                <span className="panel-title">NEXT ACTION</span>
                <span className="panel-action">PME boundary stays visible</span>
              </div>
              <div className="panel-body">
                <p className="dashboard-next-action">
                  {dashboard.nextAction ??
                    'Move into Explorer or Audit to inspect concrete records, warnings, and artifacts.'}
                </p>
                <div className="utility-block__actions">
                  <Link className="ghost-button" to="/explorer">
                    Open explorer
                  </Link>
                  <Link className="primary-button" to="/audit">
                    Review run artifacts
                  </Link>
                </div>
              </div>
            </section>
          </aside>
        </div>
      )}
    </section>
  )
}
