export function ImportPage() {
  return (
    <section className="page-shell">
      <section className="shell-panel">
        <div className="panel-header">
          <span className="panel-title">IMPORT WORKFLOW</span>
          <span className="panel-action">Preview → Manual → Execute</span>
        </div>
        <div className="panel-body pme-grid">
          <article className="pme-column">
            <span className="mono-kicker">PREVIEW</span>
            <h2>Inspect Takeout contents</h2>
            <p>
              Show recognized files, duplicates, quarantine paths, and the exact
              row counts before import.
            </p>
          </article>
          <article className="pme-column">
            <span className="mono-kicker">MANUAL</span>
            <h2>Explain manual fallbacks</h2>
            <p>
              When direct ingestion is not available, PathKeep should hand you
              commands and verification steps instead of guessing.
            </p>
          </article>
          <article className="pme-column">
            <span className="mono-kicker">EXECUTE</span>
            <h2>Commit only after review</h2>
            <p>
              Run the import after the destination archive, rollback path, and
              expected counts are visible.
            </p>
          </article>
        </div>
      </section>
    </section>
  )
}
