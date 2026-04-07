import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend'
import { languageLabel, supportedLanguages, useI18n } from '../../lib/i18n'
import {
  hasSafariAccessIssue,
  keyringNeedsReview,
  normalizePlatform,
  platformLabelKey,
  platformSummaryKey,
} from '../../lib/platform-guidance'
import type { ScheduleStatus, SecurityStatus } from '../../lib/types'
import { LoadingState } from '../../components/primitives/loading-state'

interface SupportState {
  scheduleStatus: ScheduleStatus | null
  securityStatus: SecurityStatus | null
}

export function SettingsPage() {
  const { saveConfig, snapshot } = useShellData()
  const { setLanguagePreference, t } = useI18n()
  const { language } = useI18n()
  const [saving, setSaving] = useState(false)
  const [supportState, setSupportState] = useState<SupportState>({
    scheduleStatus: null,
    securityStatus: null,
  })

  useEffect(() => {
    let cancelled = false
    const loadSupportState = async () => {
      try {
        const [scheduleStatus, securityStatus] = await Promise.all([
          backend.scheduleStatus(),
          backend.securityStatus(),
        ])

        if (!cancelled) {
          setSupportState({ scheduleStatus, securityStatus })
        }
      } catch {
        if (!cancelled) {
          setSupportState({ scheduleStatus: null, securityStatus: null })
        }
      }
    }

    void loadSupportState()
    return () => {
      cancelled = true
    }
  }, [snapshot?.config.preferredLanguage])

  if (!snapshot) {
    return (
      <section className="page-shell">
        <LoadingState label={t('settings.loadingModules')} />
      </section>
    )
  }

  const profiles = snapshot.browserProfiles
  const selectedIds = new Set(snapshot.config.selectedProfileIds)
  const safariNeedsAccess = hasSafariAccessIssue(profiles)
  const platform = normalizePlatform(supportState.scheduleStatus?.platform)
  const scheduleNeedsHelp =
    supportState.scheduleStatus?.installState === 'manual-review' ||
    supportState.scheduleStatus?.installState === 'mismatch' ||
    supportState.scheduleStatus?.installState === 'permission-warning' ||
    supportState.scheduleStatus?.installState === 'legacy-install-detected'
  const keyringWarning = keyringNeedsReview(supportState.securityStatus)

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

  async function handleLanguageChange(nextLanguage: string) {
    if (!snapshot) {
      return
    }

    if (
      nextLanguage !== 'system' &&
      nextLanguage !== 'en' &&
      nextLanguage !== 'zh-CN' &&
      nextLanguage !== 'zh-TW'
    ) {
      return
    }

    setSaving(true)
    try {
      setLanguagePreference(nextLanguage)
      await saveConfig({
        ...snapshot.config,
        preferredLanguage: nextLanguage,
      })
    } finally {
      setSaving(false)
    }
  }

  return (
    <section className="page-shell settings-page" data-testid="settings-page">
      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.browserProfiles')}</span>
          <span className="panel-action">{t('common.rescanAction')}</span>
        </div>
        <div className="panel-body">
          <div className="profile-list">
            {profiles.map((profile) => {
              const checked = selectedIds.has(profile.profileId)
              return (
                <button
                  key={profile.profileId}
                  className={`profile-item ${checked ? 'checked' : ''}`}
                  type="button"
                  onClick={() => {
                    void toggleProfile(profile.profileId)
                  }}
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
                      ? `${t('settings.historyFound')} · ${profile.browserVersion ?? t('common.notAvailable')}`
                      : t('settings.noHistoryDetected')}
                  </div>
                </button>
              )
            })}
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.aiProvider')}</span>
          <span className="panel-badge">{t('settings.optional')}</span>
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
                  {snapshot.config.ai.enabled
                    ? t('settings.enabled')
                    : t('settings.disabled')}
                </span>
              </div>
              <div className="provider-config">
                <div className="config-row">
                  <span className="config-label">
                    {t('settings.baseUrlLabel')}
                  </span>
                  <span className="config-value mono">
                    http://localhost:11434
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('settings.embeddingModelLabel')}
                  </span>
                  <span className="config-value mono">nomic-embed-text</span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {t('settings.llmModelLabel')}
                  </span>
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
                  {t('settings.disabled')}
                </span>
              </div>
              <div className="provider-config dim">
                <div className="config-row">
                  <span className="config-label">
                    {t('settings.apiKeyLabel')}
                  </span>
                  <span className="config-value mono">
                    {t('common.notAvailable')}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('settings.general')}</span>
        </div>
        <div className="panel-body">
          <div className="config-row">
            <span className="config-label">
              {t('settings.interfaceLanguage')}
            </span>
            <select
              aria-label={t('settings.interfaceLanguage')}
              className="settings-select"
              disabled={saving}
              value={snapshot.config.preferredLanguage}
              onChange={(event) => {
                void handleLanguageChange(event.target.value)
              }}
            >
              <option value="system">{t('common.followSystem')}</option>
              {supportedLanguages.map((entry) => (
                <option key={entry} value={entry}>
                  {languageLabel(entry, language)}
                </option>
              ))}
            </select>
          </div>
          <div className="config-row">
            <span className="config-label">
              {t('settings.currentLanguage')}
            </span>
            <span className="config-value">
              {languageLabel(language, language)}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">{t('settings.dataDirectory')}</span>
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
              {t('settings.openDirectory')}
            </button>
          </div>
          <div className="config-row">
            <span className="config-label">{t('settings.mcpServer')}</span>
            <span className="config-value">
              {snapshot.config.ai.mcpEnabled
                ? t('settings.enabled')
                : t('settings.disabled')}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">{t('settings.version')}</span>
            <span className="config-value mono">
              {snapshot.config.initialized ? '0.1.0-alpha' : '0.1.0-preview'}
            </span>
          </div>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">
            {t('settings.platformTroubleshooting')}
          </span>
        </div>
        <div className="panel-body settings-support-grid">
          <StatusCallout
            tone={scheduleNeedsHelp ? 'warning' : 'info'}
            title={t(platformLabelKey(platform))}
            body={t(platformSummaryKey(platform))}
            actions={
              <Link className="btn-secondary" to="/schedule">
                {t('settings.reviewSchedule')}
              </Link>
            }
          />
          {safariNeedsAccess ? (
            <StatusCallout
              tone="blocked"
              title={t('platform.safariAccessTitle')}
              body={t('platform.safariAccessBody')}
              actions={
                <Link className="btn-secondary" to="/import">
                  {t('settings.reviewImports')}
                </Link>
              }
            />
          ) : null}
          {keyringWarning ? (
            <StatusCallout
              tone="warning"
              title={t('platform.keyringTitle')}
              body={t('platform.keyringBody')}
              actions={
                <Link className="btn-secondary" to="/security">
                  {t('settings.reviewSecurity')}
                </Link>
              }
            />
          ) : null}
          {scheduleNeedsHelp ? (
            <StatusCallout
              tone="blocked"
              title={t('platform.schedulerMismatchTitle')}
              body={t('platform.schedulerMismatchBody')}
              actions={
                <Link className="btn-secondary" to="/schedule">
                  {t('settings.reviewSchedule')}
                </Link>
              }
            />
          ) : null}
        </div>
      </div>
    </section>
  )
}
