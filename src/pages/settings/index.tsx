import { LoadingState } from '../../components/primitives/loading-state'

export function SettingsPage() {
  return (
    <section className="page-shell">
      <section className="shell-panel">
        <div className="panel-header">
          <span className="panel-title">SETTINGS SURFACE</span>
          <span className="panel-action">
            Profiles, paths, language, providers
          </span>
        </div>
        <div className="panel-body stack-list">
          <LoadingState label="Settings modules are being split out of the legacy context." />
          <div className="list-item">
            <strong>Data root</strong>
            <span className="mono-support">
              ~/Library/Application Support/PathKeep
            </span>
          </div>
          <div className="list-item">
            <strong>Language</strong>
            <span className="mono-support">
              System / English / 简体中文 / 繁體中文
            </span>
          </div>
        </div>
      </section>
    </section>
  )
}
