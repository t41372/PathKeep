import { useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { BrandMark } from '../../components/brand-mark'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend'
import { useI18n } from '../../lib/i18n'

function lockReasonLabel(
  reason: string | null | undefined,
  t: (key: string, params?: Record<string, string | number>) => string,
) {
  switch (reason) {
    case 'idle-timeout':
      return t('shell.lockReasonIdleTimeout')
    case 'startup':
      return t('shell.lockReasonStartup')
    default:
      return t('shell.lockReasonManual')
  }
}

export function LockPage() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const { appLockStatus, buildInfo, error, unlockAppSession } = useShellData()
  const { t } = useI18n()
  const [passcode, setPasscode] = useState('')
  const [unlocking, setUnlocking] = useState(false)

  if (!appLockStatus) {
    return (
      <section className="lock-page-shell">
        <LoadingState label={t('common.loading')} />
      </section>
    )
  }

  const nextPath = searchParams.get('next')?.trim() || '/'
  const reason = lockReasonLabel(appLockStatus.lockReason, t)
  const canTryBiometric =
    appLockStatus.biometricEnabled || appLockStatus.biometricAvailable
  const touchIdState =
    appLockStatus.biometricState === 'touch-id-available' ||
    appLockStatus.biometricState === 'touch-id-unavailable'

  async function handleUnlock(useBiometric = false) {
    setUnlocking(true)
    try {
      await unlockAppSession({
        passcode: useBiometric ? null : passcode,
        useBiometric,
      })
      setPasscode('')
      void navigate(nextPath, { replace: true })
    } finally {
      setUnlocking(false)
    }
  }

  return (
    <section className="lock-page-shell" data-testid="lock-page">
      <div className="lock-page">
        <div className="lock-page__hero">
          <div className="logo-lockup">
            <div aria-hidden className="logo-mark">
              <BrandMark alt="" />
            </div>
            <div className="logo-text">
              <span className="logo-name">PATHKEEP</span>
              <span className="logo-version">
                {buildInfo?.version ?? t('common.notAvailable')}
              </span>
            </div>
          </div>
          <span className="mono-kicker">{t('shell.lockEyebrow')}</span>
          <h1 className="lock-page__title">{t('shell.lockTitle')}</h1>
          <p className="lock-page__body">{t('shell.lockDescription')}</p>
        </div>

        <div className="lock-page__panel">
          <div className="lock-page__summary">
            <div className="config-row">
              <span className="config-label">{t('shell.lockReason')}</span>
              <span className="config-value mono">{reason}</span>
            </div>
            <div className="config-row">
              <span className="config-label">{t('shell.lockConfigPath')}</span>
              <span className="config-value mono">
                {appLockStatus.configPath}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">{t('shell.lastUnlockedAt')}</span>
              <span className="config-value mono">
                {appLockStatus.lastUnlockedAt ?? t('common.notAvailable')}
              </span>
            </div>
          </div>

          {error ? (
            <StatusCallout
              tone="danger"
              title={t('shell.unlockAppFailed')}
              body={error}
            />
          ) : null}

          {appLockStatus.warnings.map((warning) => (
            <StatusCallout
              key={warning}
              tone="warning"
              title={t('common.warning')}
              body={warning}
            />
          ))}

          <form
            className="lock-page__form"
            onSubmit={(event) => {
              event.preventDefault()
              void handleUnlock(false)
            }}
          >
            <label className="fieldBlock">
              <span className="config-label">
                {t('shell.lockPasscodeLabel')}
              </span>
              <input
                aria-label={t('shell.lockPasscodeLabel')}
                autoComplete="current-password"
                className="settings-input"
                disabled={unlocking}
                placeholder={t('shell.lockPasscodePlaceholder')}
                type="password"
                value={passcode}
                onChange={(event) => setPasscode(event.target.value)}
              />
            </label>

            <div className="settings-action-row">
              <button
                className="btn-primary"
                type="submit"
                disabled={unlocking || !passcode.trim()}
              >
                {unlocking ? t('shell.unlockingApp') : t('shell.unlockApp')}
              </button>
              {canTryBiometric ? (
                <button
                  className="btn-secondary"
                  type="button"
                  disabled={unlocking || !appLockStatus.biometricAvailable}
                  onClick={() => {
                    void handleUnlock(true)
                  }}
                >
                  {touchIdState
                    ? t('shell.unlockWithTouchId')
                    : t('shell.unlockWithBiometric')}
                </button>
              ) : null}
            </div>
          </form>

          {canTryBiometric && !appLockStatus.biometricAvailable ? (
            <p className="dashboard-next-action">
              {touchIdState
                ? t('shell.unlockTouchIdUnavailable')
                : t('shell.unlockBiometricUnavailable')}
            </p>
          ) : null}

          <StatusCallout
            tone="info"
            title={t('shell.lockRecoveryTitle')}
            body={
              appLockStatus.recoveryHint
                ? t('shell.lockRecoveryHintBody', {
                    hint: appLockStatus.recoveryHint,
                  })
                : t('shell.lockRecoveryBody')
            }
            actions={
              <div className="settings-action-row">
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => {
                    void backend.openPathInFileManager(appLockStatus.configPath)
                  }}
                >
                  {t('shell.lockRecoveryAction')}
                </button>
              </div>
            }
          />

          {appLockStatus.degradationNotes.length ? (
            <div className="lock-page__notes">
              {appLockStatus.degradationNotes.map((note) => (
                <p key={note} className="dashboard-next-action">
                  {note}
                </p>
              ))}
            </div>
          ) : null}
        </div>
      </div>
    </section>
  )
}
