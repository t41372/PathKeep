import { useState } from 'react'
import { useShellData } from '../../app/shell-data-context'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'

export function SettingsPage() {
  const { loading, saveConfig, snapshot } = useShellData()
  const [saving, setSaving] = useState(false)

  if (loading && !snapshot)
    return (
      <section className="page-shell">
        <LoadingState label="Loading settings" />
      </section>
    )
  if (!snapshot)
    return (
      <section className="page-shell">
        <LoadingState label="Settings modules loading" />
      </section>
    )

  const profiles = snapshot.browserProfiles
  const selectedIds = new Set(snapshot.config.selectedProfileIds)

  function browserIcon(profileId: string): string {
    const kind = profileId.split(':')[0]
    if (kind === 'chrome') return 'C'
    if (kind === 'arc') return 'A'
    if (kind === 'firefox') return 'F'
    if (kind === 'safari') return 'S'
    return kind[0]?.toUpperCase() ?? '?'
  }

  function browserIconClass(profileId: string): string {
    const kind = profileId.split(':')[0]
    return `browser-icon ${kind}`
  }

  async function toggleProfile(profileId: string) {
    if (saving || !snapshot) return
    setSaving(true)
    try {
      const next = selectedIds.has(profileId)
        ? snapshot.config.selectedProfileIds.filter((id) => id !== profileId)
        : [...snapshot.config.selectedProfileIds, profileId]
      await saveConfig({ ...snapshot.config, selectedProfileIds: next })
    } finally {
      setSaving(false)
    }
  }

  return (
    <section className="page-shell settings-page" data-testid="settings-page">
      {/* Browser Profiles */}
      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">BROWSER PROFILES</span>
          <span className="panel-action">Rescan</span>
        </div>
        <div className="panel-body">
          <div className="profile-list">
            {profiles.map((profile) => {
              const checked = selectedIds.has(profile.profileId)
              return (
                <div
                  key={profile.profileId}
                  className={`profile-item ${checked ? 'checked' : ''}`}
                  onClick={() => {
                    void toggleProfile(profile.profileId)
                  }}
                  style={{ cursor: 'pointer' }}
                >
                  <div className="profile-check">
                    <div className={`checkbox ${checked ? 'active' : ''}`}>
                      {checked ? '✓' : ''}
                    </div>
                  </div>
                  <div className="profile-icon">
                    <div className={browserIconClass(profile.profileId)}>
                      {browserIcon(profile.profileId)}
                    </div>
                  </div>
                  <div className="profile-info">
                    <div className="profile-name">
                      {profile.browserName} / {profile.profileName}
                    </div>
                    <div className="profile-path dim mono">
                      {profile.profilePath}
                    </div>
                  </div>
                  <div className="profile-stats mono dim">
                    {profile.historyExists
                      ? `History found · ${profile.browserVersion ?? 'unknown version'}`
                      : 'No history file detected'}
                  </div>
                </div>
              )
            })}
          </div>
        </div>
      </div>

      {/* AI Provider */}
      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">AI PROVIDER</span>
          <span className="panel-badge">OPTIONAL</span>
        </div>
        <div className="panel-body">
          <div className="provider-cards">
            <div
              className={`provider-card ${snapshot.config.ai.enabled ? 'active-provider' : ''}`}
            >
              <div className="provider-header">
                <span className="provider-name">Ollama (Local)</span>
                <span
                  className={`status-badge ${snapshot.config.ai.enabled ? 'status-completed' : ''}`}
                  style={
                    !snapshot.config.ai.enabled
                      ? {
                          color: 'var(--text-faint)',
                          borderColor: 'var(--border)',
                        }
                      : undefined
                  }
                >
                  {snapshot.config.ai.enabled ? 'CONNECTED' : 'DISABLED'}
                </span>
              </div>
              <div className="provider-config">
                <div className="config-row">
                  <span className="config-label">Base URL</span>
                  <span className="config-value mono">
                    http://localhost:11434
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">Embedding Model</span>
                  <span className="config-value mono">nomic-embed-text</span>
                </div>
                <div className="config-row">
                  <span className="config-label">LLM Model</span>
                  <span className="config-value mono">llama3.2:8b</span>
                </div>
              </div>
            </div>
            <div className="provider-card">
              <div className="provider-header">
                <span className="provider-name">OpenAI</span>
                <span
                  className="status-badge"
                  style={{
                    color: 'var(--text-faint)',
                    borderColor: 'var(--border)',
                  }}
                >
                  DISABLED
                </span>
              </div>
              <div className="provider-config dim">
                <div className="config-row">
                  <span className="config-label">API Key</span>
                  <span className="config-value mono">Not configured</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* General */}
      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">GENERAL</span>
        </div>
        <div className="panel-body">
          <div className="config-row">
            <span className="config-label">Language</span>
            <span className="config-value">
              {snapshot.config.preferredLanguage === 'system'
                ? 'System'
                : snapshot.config.preferredLanguage}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">Data Directory</span>
            <span className="config-value mono">
              {snapshot.directories.appRoot}
            </span>
            <button
              className="btn-tiny"
              type="button"
              onClick={() => {
                void backend.openPathInFileManager(snapshot.directories.appRoot)
              }}
            >
              Open in Finder
            </button>
          </div>
          <div className="config-row">
            <span className="config-label">MCP Server</span>
            <span className="config-value">
              {snapshot.config.ai.mcpEnabled ? 'Enabled' : 'Disabled'}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">Version</span>
            <span className="config-value mono">0.1.0-alpha</span>
          </div>
        </div>
      </div>
    </section>
  )
}
