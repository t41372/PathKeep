import {
  dashboardStats,
  insightHighlights,
  queueItems,
  recentRuns,
} from '../../app/preview-data'

export function DashboardPage() {
  return (
    <section className="page-shell" data-testid="dashboard-page">
      <div className="stats-row">
        {dashboardStats.map((stat) => (
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

      <div className="content-grid">
        <section className="shell-panel">
          <div className="panel-header">
            <span className="panel-title">RECENT RUNS</span>
            <span className="panel-action">View all →</span>
          </div>
          <div className="panel-body">
            <table className="shell-table">
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
                {recentRuns.map((run) => (
                  <tr key={run.id}>
                    <td className="mono-cell">{run.id}</td>
                    <td>{run.type}</td>
                    <td>{run.source}</td>
                    <td className="mono-cell">{run.records}</td>
                    <td>{run.status}</td>
                    <td className="mono-cell">{run.time}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <aside className="stacked-column">
          <section className="shell-panel shell-panel--accent">
            <div className="panel-header">
              <span className="panel-title">ON THIS DAY · 2025-04-05</span>
              <span className="panel-action">1 year ago</span>
            </div>
            <div className="panel-body stack-list">
              {insightHighlights.map((item) => (
                <article key={item.title} className="list-item">
                  <strong>{item.title}</strong>
                  <span className="mono-support">{item.source}</span>
                </article>
              ))}
            </div>
          </section>

          <section className="shell-panel">
            <div className="panel-header">
              <span className="panel-title">JOB QUEUE</span>
              <span className="panel-action">Manual + optional AI work</span>
            </div>
            <div className="panel-body stack-list">
              {queueItems.map((job) => (
                <article key={job.id} className="list-item">
                  <div className="row-between">
                    <strong>{job.title}</strong>
                    <span className={`state-chip state-chip--${job.state}`}>
                      {job.state}
                    </span>
                  </div>
                  <span className="mono-support">{job.id}</span>
                  <p>{job.detail}</p>
                </article>
              ))}
            </div>
          </section>
        </aside>
      </div>
    </section>
  )
}
