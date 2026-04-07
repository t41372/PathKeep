import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { formatBytes, formatRelativeTime } from '../../lib/format'

function isBackupReadyProfile(profileId: string) {
  return profileId.startsWith('chrome:') || profileId.startsWith('arc:')
}

function browserIconClass(profileId: string) {
  if (profileId.startsWith('chrome:')) return 'chrome'
  if (profileId.startsWith('arc:')) return 'arc'
  if (profileId.startsWith('firefox:')) return 'firefox'
  if (profileId.startsWith('safari:')) return 'safari'
  return ''
}

function browserIconLetter(profileId: string) {
  if (profileId.startsWith('chrome:')) return 'C'
  if (profileId.startsWith('arc:')) return 'A'
  if (profileId.startsWith('firefox:')) return 'F'
  if (profileId.startsWith('safari:')) return 'S'
  return '?'
}

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

  const selectedProfiles = snapshot.browserProfiles.filter((profile) =>
    snapshot.config.selectedProfileIds.includes(profile.profileId),
  )
  const backupReadyProfiles = selectedProfiles.filter((profile) =>
    isBackupReadyProfile(profile.profileId),
  )
  const previewOnlyProfiles = selectedProfiles.filter(
    (profile) => !isBackupReadyProfile(profile.profileId),
  )
  const totalStorage =
    dashboard.storage.archiveDatabaseBytes +
    dashboard.storage.manifestBytes +
    dashboard.storage.snapshotBytes +
    dashboard.storage.exportBytes
  const storageSegments = [
    {
      label: 'Archive Database',
      tone: 'storage-fill',
      value: dashboard.storage.archiveDatabaseBytes,
    },
    {
      label: 'Manifests',
      tone: 'storage-fill secondary',
      value: dashboard.storage.manifestBytes,
    },
    {
      label: 'Snapshots',
      tone: 'storage-fill tertiary',
      value: dashboard.storage.snapshotBytes,
    },
    {
      label: 'Exports',
      tone: 'storage-fill dim',
      value: dashboard.storage.exportBytes,
    },
  ]
  const stats = [
    {
      label: 'TOTAL RECORDS',
      value: dashboard.totalVisits.toLocaleString(),
      detail: `${dashboard.totalUrls.toLocaleString()} unique URLs`,
      tone: 'accent' as const,
    },
    {
      label: 'LAST BACKUP',
      value: dashboard.lastSuccessfulBackupAt
        ? formatRelativeTime(dashboard.lastSuccessfulBackupAt)
        : 'Pending',
      detail:
        dashboard.recentRuns[0]?.manifestHash ??
        'No manifest written to the chain yet',
      tone: dashboard.lastSuccessfulBackupAt
        ? ('success' as const)
        : ('neutral' as const),
    },
    {
      label: 'PROFILES IN SCOPE',
      value: `${selectedProfiles.length}`,
      detail: `${backupReadyProfiles.length} backup-ready · ${previewOnlyProfiles.length} preview-only`,
      tone: 'neutral' as const,
    },
    {
      label: 'ARCHIVE MODE',
      value: snapshot.config.archiveMode.toUpperCase(),
      detail: snapshot.archiveStatus.unlocked
        ? 'Archive session is unlocked'
        : 'Archive requires an unlock before Explorer / Audit can read',
      tone: snapshot.archiveStatus.unlocked
        ? ('success' as const)
        : ('neutral' as const),
    },
  ]

  if (!snapshot.config.initialized || dashboard.recentRuns.length === 0) {
    return (
      <section className="page-shell" data-testid="dashboard-page">
        <div className="stats-row">
          {stats.map((stat) => (
            <article
              key={stat.label}
              className="stat-card"
              data-tone={stat.tone}
            >
              <div className="stat-label">{stat.label}</div>
              <div className="stat-value">{stat.value}</div>
              <div className="stat-delta neutral">{stat.detail}</div>
            </article>
          ))}
        </div>
        <EmptyState
          action={
            <Link className="btn-primary" to="/onboarding">
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
      </section>
    )
  }

  return (
    <section className="page-shell" data-testid="dashboard-page">
      <div className="stats-row">
        {stats.map((stat) => (
          <article key={stat.label} className="stat-card" data-tone={stat.tone}>
            <div className="stat-label">{stat.label}</div>
            <div className="stat-value">{stat.value}</div>
            <div
              className={`stat-delta ${stat.tone === 'success' ? 'positive' : 'neutral'}`}
            >
              {stat.detail}
            </div>
          </article>
        ))}
      </div>

      <div className="dashboard-grid">
        <div className="dashboard-left">
          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">RECENT RUNS</span>
              <Link className="panel-action" to="/audit">
                Full ledger →
              </Link>
            </div>
            <div className="panel-body" style={{ padding: 0 }}>
              <table className="data-table">
                <thead>
                  <tr>
                    <th>RUN</th>
                    <th>TYPE</th>
                    <th>SOURCE</th>
                    <th>RECORDS</th>
                    <th>STATUS</th>
                    <th>TIME</th>
                  </tr>
                </thead>
                <tbody>
                  {dashboard.recentRuns.map((run) => (
                    <tr key={run.id}>
                      <td>
                        <Link
                          className="table-link mono accent"
                          to={`/audit?run=${run.id}`}
                        >
                          #{run.id}
                        </Link>
                      </td>
                      <td>
                        <span className="tag tag-sm tag-backup">BACKUP</span>
                      </td>
                      <td>{run.profilesProcessed} profiles</td>
                      <td className="accent">+{run.newVisits}</td>
                      <td>
                        <span
                          className={`status-badge ${
                            run.status === 'success'
                              ? 'status-completed'
                              : 'status-pending'
                          }`}
                        >
                          {run.status}
                        </span>
                      </td>
                      <td className="dim">
                        {formatRelativeTime(run.finishedAt ?? run.startedAt)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">ARCHIVE BOUNDARY</span>
              <span className="panel-action">
                {selectedProfiles.length} selected profiles
              </span>
            </div>
            <div className="panel-body">
              {selectedProfiles.map((profile) => (
                <div key={profile.profileId} className="otd-item">
                  <div
                    className={`browser-icon ${browserIconClass(profile.profileId)}`}
                  >
                    {browserIconLetter(profile.profileId)}
                  </div>
                  <div style={{ minWidth: 0 }}>
                    <div className="otd-title">
                      {profile.browserName} / {profile.profileName}
                    </div>
                    <div className="otd-url dim mono">
                      {profile.profilePath}
                    </div>
                  </div>
                  <span
                    className={`tag tag-sm ${
                      isBackupReadyProfile(profile.profileId)
                        ? 'tag-backup'
                        : 'tag-search'
                    }`}
                  >
                    {isBackupReadyProfile(profile.profileId)
                      ? 'backup-ready'
                      : 'preview-only'}
                  </span>
                </div>
              ))}
              <div className="otd-summary">
                <div className="summary-label">VISIBLE SCOPE</div>
                <p>
                  Explorer, Export, and Audit read only the canonical archive.
                  Preview-only browser detections are visible here, but they do
                  not count as ingested history until their pipeline ships.
                </p>
              </div>
            </div>
          </div>
        </div>

        <div className="dashboard-right">
          <div className="panel panel-accent">
            <div className="panel-header">
              <span className="panel-title">NEXT ACTION</span>
              <span className="panel-badge">M1</span>
            </div>
            <div className="panel-body">
              <div className="summary-label">VERIFY</div>
              <p className="dashboard-next-action">
                {dashboard.nextAction ??
                  'Manual backup is current. Review the latest audit run, then confirm schedule and security before treating the archive as hands-off.'}
              </p>
              <div className="wizard-actions">
                <Link className="btn-secondary" to="/audit">
                  Review audit
                </Link>
                <Link className="btn-secondary" to="/schedule">
                  Review schedule
                </Link>
              </div>
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">STORAGE</span>
              <span className="panel-action">{formatBytes(totalStorage)}</span>
            </div>
            <div className="panel-body">
              {storageSegments.map((segment) => (
                <div key={segment.label} className="storage-item">
                  <div className="storage-label">
                    <span>{segment.label}</span>
                    <span className="dim">{formatBytes(segment.value)}</span>
                  </div>
                  <div className="storage-bar">
                    <div
                      className={segment.tone}
                      style={{
                        width:
                          totalStorage === 0
                            ? '0%'
                            : `${(segment.value / totalStorage) * 100}%`,
                      }}
                    />
                  </div>
                </div>
              ))}
              <div className="storage-total">
                <span>Total</span>
                <span>{formatBytes(totalStorage)}</span>
              </div>
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">SECURITY POSTURE</span>
              <Link className="panel-action" to="/security">
                Open security →
              </Link>
            </div>
            <div className="panel-body">
              <div className="config-row">
                <span className="config-label">Mode</span>
                <span className="config-value">
                  {snapshot.config.archiveMode}
                </span>
              </div>
              <div className="config-row">
                <span className="config-label">Session</span>
                <span className="config-value">
                  {snapshot.archiveStatus.unlocked ? 'Unlocked' : 'Locked'}
                </span>
              </div>
              <div className="config-row">
                <span className="config-label">Keyring</span>
                <span className="config-value">
                  {snapshot.keyringStatus.storedSecret
                    ? `${snapshot.keyringStatus.backend} stored`
                    : `${snapshot.keyringStatus.backend} empty`}
                </span>
              </div>
              <div className="config-row">
                <span className="config-label">Database</span>
                <span className="config-value mono dim">
                  {snapshot.directories.archiveDatabasePath}
                </span>
              </div>
            </div>
          </div>

          <div className="panel">
            <div className="panel-header">
              <span className="panel-title">EXPLORER + EXPORT</span>
              <Link className="panel-action" to="/explorer">
                Open explorer →
              </Link>
            </div>
            <div className="panel-body">
              <div className="otd-item">
                <span className="otd-favicon">◎</span>
                <div>
                  <div className="otd-title">URL-deep-link queries</div>
                  <div className="otd-url dim mono">
                    Re-openable filters for keyword, domain, profile, browser,
                    and date range.
                  </div>
                </div>
              </div>
              <div className="otd-item">
                <span className="otd-favicon">⊞</span>
                <div>
                  <div className="otd-title">Audit-first exports</div>
                  <div className="otd-url dim mono">
                    Export reads the current visible archive scope only. Review
                    the run ledger for the manifest and artifact trail.
                  </div>
                </div>
              </div>
              <div className="wizard-actions">
                <Link className="btn-secondary" to="/explorer">
                  Search history
                </Link>
                <Link className="btn-secondary" to="/audit">
                  Manifest chain
                </Link>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}
