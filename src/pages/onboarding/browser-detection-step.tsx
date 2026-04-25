/**
 * @file browser-detection-step.tsx
 * @description Renders the onboarding browser detection and profile selection step.
 * @module pages/onboarding
 *
 * ## 職責
 * - 顯示 detected browser profiles、retention honesty、與可讀/需注意狀態。
 * - 把 profile toggle 和 back/continue actions 交回 route owner。
 *
 * ## 不負責
 * - 不重新掃描瀏覽器。
 * - 不直接保存 config。
 * - 不決定 onboarding 下一步是否合法。
 */

import { browserRetentionMeta } from '../../lib/browser-retention'
import { BrowserIcon } from '../../lib/browser-icons'
import { useI18n } from '../../lib/i18n'
import type { BrowserProfile } from '../../lib/types'

export interface BrowserDetectionStepProps {
  browserProfiles: BrowserProfile[]
  busyAction: string | null
  localError: string | null
  selectedCount: number
  selectedProfileIds: string[]
  onBack: () => void
  onContinue: () => void
  onToggleProfile: (profileId: string) => void
}

export function BrowserDetectionStep({
  browserProfiles,
  busyAction,
  localError,
  selectedCount,
  selectedProfileIds,
  onBack,
  onContinue,
  onToggleProfile,
}: BrowserDetectionStepProps) {
  const { t, ns } = useI18n('onboarding')
  const commonT = ns('common')
  const readableProfiles = browserProfiles.filter(
    (profile) => profile.historyExists,
  )
  const attentionProfiles = browserProfiles.filter(
    (profile) => !profile.historyExists,
  )

  return (
    <div className="ob-panel-container">
      <div className="ob-header">
        <div className="crosshair-mark">+</div>
        <h2 className="ob-title">{t('browserDetectionTitle')}</h2>
        <p className="ob-desc">{t('browserDetectionDesc')}</p>
      </div>

      <div className="ob-scan-status">
        <div className="status-dot status-ok" />
        <span className="mono">
          {t('scanStatus')
            .replace('{count}', String(browserProfiles.length))
            .replace('{selected}', String(selectedCount))}
        </span>
      </div>

      <div className="panel" style={{ marginTop: 'var(--space-4)' }}>
        <div className="panel-header">
          <span className="panel-title">{t('detectedProfiles')}</span>
          <span className="panel-action">
            {t('found').replace('{count}', String(browserProfiles.length))}
          </span>
        </div>
        <div className="panel-body" style={{ padding: 0 }}>
          <div className="profile-list">
            {[...readableProfiles, ...attentionProfiles].map((profile) => {
              const selected = selectedProfileIds.includes(profile.profileId)
              const retention = browserRetentionMeta(profile, commonT)

              return (
                <label
                  className="profile-item"
                  key={profile.profileId}
                  style={{ cursor: 'pointer' }}
                >
                  <input
                    aria-label={`${profile.browserName} / ${profile.profileName}`}
                    checked={selected}
                    className="sr-only"
                    type="checkbox"
                    onChange={() => onToggleProfile(profile.profileId)}
                  />
                  <div className={`checkbox ${selected ? 'active' : ''}`}>
                    {selected ? '✓' : ''}
                  </div>
                  <div className="browser-icon">
                    <BrowserIcon browserName={profile.browserName} />
                  </div>
                  <div className="profile-info">
                    <div className="profile-name">
                      {profile.browserName} / {profile.profileName}
                    </div>
                    <div className="profile-path dim mono">
                      {profile.profilePath}
                    </div>
                  </div>
                  <div className="profile-detection">
                    <span
                      className={`status-badge ${
                        profile.historyExists
                          ? 'status-completed'
                          : 'status-pending'
                      }`}
                    >
                      {profile.historyExists
                        ? t('historyFound')
                        : t('actionRequired')}
                    </span>
                    <span
                      className="mono dim"
                      style={{
                        display: 'block',
                        fontSize: '10px',
                        marginTop: '2px',
                      }}
                    >
                      {profile.historyExists
                        ? t('browserEngineLabel', {
                            version:
                              profile.browserVersion ?? t('versionUnknown'),
                            engine: profile.browserFamily,
                          })
                        : profile.browserFamily === 'safari'
                          ? t('safariAccessHint')
                          : t('cannotReadHint').replace(
                              '{fileName}',
                              profile.historyFileName,
                            )}
                    </span>
                    {profile.historyExists ? (
                      <>
                        <span
                          className="mono dim"
                          style={{
                            display: 'block',
                            fontSize: '10px',
                            marginTop: '2px',
                          }}
                        >
                          {retention.label}
                        </span>
                        <span
                          className="mono dim"
                          style={{
                            display: 'block',
                            fontSize: '10px',
                            marginTop: '2px',
                          }}
                        >
                          {commonT('browserRetentionArchiveBoundary')}
                        </span>
                      </>
                    ) : null}
                  </div>
                </label>
              )
            })}
          </div>
        </div>
      </div>

      {browserProfiles.some(
        (profile) => profile.browserFamily !== 'chromium',
      ) ? (
        <div className="ob-info-box">
          <span className="info-icon">ℹ</span>
          <span className="info-text">{t('firefoxSafariInfo')}</span>
        </div>
      ) : null}

      {localError ? (
        <p className="inline-error" role="alert">
          {localError}
        </p>
      ) : null}

      <div className="ob-actions">
        <button className="btn-secondary" type="button" onClick={onBack}>
          {t('backButton')}
        </button>
        <button
          className="btn-primary"
          type="button"
          disabled={busyAction !== null}
          onClick={onContinue}
        >
          {t('continueButton')}
        </button>
      </div>
    </div>
  )
}
