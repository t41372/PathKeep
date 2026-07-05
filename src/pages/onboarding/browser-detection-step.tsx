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

import { useState } from 'react'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import { StatusCallout } from '../../components/primitives/status-callout'
import { browserRetentionMeta } from '../../lib/browser-retention'
import { BrowserIcon } from '../../lib/browser-icons'
import { describeError } from '../../lib/errors'
import { useI18n } from '../../lib/i18n'
import {
  browserDiscoveryState,
  hasBrowserProfileAccessIssue,
  isBrowserProfileReadable,
} from '../../lib/platform-guidance'
import type { BrowserProfile } from '../../lib/types'

export interface BrowserDetectionStepProps {
  browserProfiles: BrowserProfile[]
  /**
   * Optional discovery-outcome marker from the app snapshot. Drives the empty
   * state's grammar (permission denied vs. real error vs. genuinely empty) so a
   * missing OS permission is never mistaken for "no browsers".
   */
  browserDiscoveryIssue?: string | null
  busyAction: string | null
  localError: string | null
  selectedAccessIssueCount: number
  selectedCount: number
  selectedProfileIds: string[]
  onBack: () => void
  onContinue: () => void
  onOpenFullDiskAccessSettings: () => void
  /** Re-fetches the app snapshot so newly granted access / installed browsers appear without restarting setup. */
  onRecheck: () => void | Promise<void>
  onToggleProfile: (profileId: string) => void
}

export function BrowserDetectionStep({
  browserProfiles,
  browserDiscoveryIssue,
  busyAction,
  localError,
  selectedAccessIssueCount,
  selectedCount,
  selectedProfileIds,
  onBack,
  onContinue,
  onOpenFullDiskAccessSettings,
  onRecheck,
  onToggleProfile,
}: BrowserDetectionStepProps) {
  const { t, ns } = useI18n('onboarding')
  const commonT = ns('common')
  const [rechecking, setRechecking] = useState(false)
  const [recheckError, setRecheckError] = useState<string | null>(null)
  const readableProfiles = browserProfiles.filter(isBrowserProfileReadable)
  const attentionProfiles = browserProfiles.filter(
    (profile) => !isBrowserProfileReadable(profile),
  )
  const isEmpty = browserProfiles.length === 0
  const discoveryState = browserDiscoveryState(
    browserDiscoveryIssue,
    browserProfiles.length,
  )
  // Re-check keeps its own pending flag so the button shows progress and cannot
  // be fired twice, without freezing the step (the snapshot re-fetch runs off
  // the main thread and never blocks render). A rejected refresh (the snapshot
  // fetch re-throws) must NOT become an unhandled rejection or a silent no-op:
  // onboarding's global error gate is suppressed while a snapshot still exists,
  // so we surface the failure in-step and always re-enable the button.
  const handleRecheck = async () => {
    setRecheckError(null)
    setRechecking(true)
    try {
      await onRecheck()
    } catch (error) {
      setRecheckError(
        t('errorRecheckFailed', {
          detail: describeError(error, 'refresh_app_data'),
        }),
      )
    } finally {
      setRechecking(false)
    }
  }
  const recheckButton = (
    <button
      className="btn-secondary"
      disabled={rechecking}
      type="button"
      onClick={() => {
        void handleRecheck()
      }}
    >
      {rechecking ? t('recheckingBrowsers') : t('recheckBrowsers')}
    </button>
  )
  const engineLabel = (engine: string) => {
    if (engine === 'chromium') return t('browserEngineChromium')
    if (engine === 'safari') return t('browserEngineSafari')
    if (engine === 'firefox') return t('browserEngineFirefox')
    return engine || t('browserEngineUnknown')
  }

  return (
    <div className="ob-panel-container">
      <div className="ob-header">
        <div className="crosshair-mark">+</div>
        <h2 className="ob-title">{t('browserDetectionTitle')}</h2>
        <p className="ob-desc">{t('browserDetectionDesc')}</p>
      </div>

      {isEmpty ? (
        <div className="mt-4">
          {discoveryState === 'full-disk-access' ? (
            <StatusCallout
              tone="warning"
              role="status"
              title={t('fullDiskAccessEmptyTitle')}
              body={t('fullDiskAccessEmptyBody')}
              actions={
                <>
                  <button
                    className="btn-primary"
                    type="button"
                    onClick={onOpenFullDiskAccessSettings}
                  >
                    {t('openFullDiskAccessSettings')}
                  </button>
                  {recheckButton}
                </>
              }
            />
          ) : discoveryState === 'discovery-error' ? (
            <StatusCallout
              tone="danger"
              role="alert"
              title={t('discoveryErrorTitle')}
              body={t('discoveryErrorBody')}
              actions={recheckButton}
            />
          ) : (
            <StatusCallout
              tone="info"
              role="status"
              title={t('noBrowsersTitle')}
              body={t('noBrowsersBody')}
              actions={recheckButton}
            />
          )}
          {recheckError ? (
            <p className="inline-error mt-3" role="alert">
              {recheckError}
            </p>
          ) : null}
        </div>
      ) : (
        <>
          <div className="ob-scan-status">
            <div className="status-dot status-ok" />
            <span className="mono">
              {t('scanStatus')
                .replace('{count}', String(browserProfiles.length))
                .replace('{selected}', String(selectedCount))}
            </span>
          </div>

          <div className="mt-4">
            <PaperCard testId="onboarding-browser-detection-profiles">
              <PaperCardHeader
                title={t('detectedProfiles')}
                right={
                  <PaperCardBadge>
                    {t('found').replace(
                      '{count}',
                      String(browserProfiles.length),
                    )}
                  </PaperCardBadge>
                }
              />
              <PaperCardBody className="p-0">
                <div className="profile-list">
                  {[...readableProfiles, ...attentionProfiles].map(
                    (profile) => {
                      const selected = selectedProfileIds.includes(
                        profile.profileId,
                      )
                      const retention = browserRetentionMeta(profile, commonT)
                      const ready = isBrowserProfileReadable(profile)
                      const accessIssue = hasBrowserProfileAccessIssue(profile)
                      const historyFileLabel =
                        profile.historyFileName ||
                        profile.historyPath?.split(/[\\/]/).pop() ||
                        profile.profileName
                      const statusLabel = ready
                        ? t('historyFound')
                        : accessIssue
                          ? t('permissionRequired')
                          : t('actionRequired')
                      const statusClass = ready
                        ? 'status-completed'
                        : accessIssue
                          ? 'status-warning'
                          : 'status-pending'
                      const detail = ready
                        ? t('browserEngineLabel', {
                            version:
                              profile.browserVersion ?? t('versionUnknown'),
                            engine: engineLabel(profile.browserFamily),
                          })
                        : accessIssue
                          ? profile.browserFamily === 'safari'
                            ? t('safariAccessHint')
                            : t('browserProfileAccessHint')
                          : profile.browserFamily === 'safari'
                            ? t('safariAccessHint')
                            : t('cannotReadHint').replace(
                                '{fileName}',
                                historyFileLabel,
                              )

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
                          <div
                            className={`checkbox ${selected ? 'active' : ''}`}
                          >
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
                              {historyFileLabel}
                            </div>
                          </div>
                          <div className="profile-detection">
                            <span className={`status-badge ${statusClass}`}>
                              {statusLabel}
                            </span>
                            <span
                              className="mono dim"
                              style={{
                                display: 'block',
                                fontSize: '10px',
                                marginTop: '2px',
                              }}
                            >
                              {detail}
                            </span>
                            {ready ? (
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
                    },
                  )}
                </div>
              </PaperCardBody>
            </PaperCard>
          </div>

          {browserProfiles.some(
            (profile) => profile.browserFamily !== 'chromium',
          ) ? (
            <div className="mt-4">
              <StatusCallout tone="info" title={t('firefoxSafariInfo')} />
            </div>
          ) : null}

          {selectedAccessIssueCount > 0 ? (
            <div className="mt-4">
              <StatusCallout
                tone="warning"
                title={t('selectedProfilesNeedAccess')}
                actions={
                  <button
                    className="btn-secondary"
                    type="button"
                    onClick={onOpenFullDiskAccessSettings}
                  >
                    {t('openFullDiskAccessSettings')}
                  </button>
                }
              />
            </div>
          ) : null}
        </>
      )}

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
