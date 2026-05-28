/**
 * @file security-step.tsx
 * @description Renders the onboarding archive-security selection step.
 * @module pages/onboarding
 */

import { useEffect, useState } from 'react'
import { backend } from '../../lib/backend'
import { StatusCallout } from '../../components/primitives/status-callout'
import { useI18n } from '../../lib/i18n'
import type { AppConfig } from '../../lib/types'
import type { SecurityDraftState } from './shared'

export interface SecurityStepProps {
  archiveMode: AppConfig['archiveMode']
  busyAction: string | null
  localError: string | null
  onBack: () => void
  onContinue: () => void
  onSecurityCardClick: (
    mode: AppConfig['archiveMode'],
    target: EventTarget | null,
  ) => void
  onSelectArchiveMode: (mode: AppConfig['archiveMode']) => void
  onUpdateSecurityDraft: (next: Partial<SecurityDraftState>) => void
  securityDraft: SecurityDraftState
}

export function SecurityStep({
  archiveMode,
  busyAction,
  localError,
  onBack,
  onContinue,
  onSecurityCardClick,
  onSelectArchiveMode,
  onUpdateSecurityDraft,
  securityDraft,
}: SecurityStepProps) {
  const { t } = useI18n('onboarding')

  // Probe the native keyring once when the encrypted card mounts. We don't
  // want users to discover headless-Linux / locked-keychain failures only
  // after they click "Create archive" — by then `initializeArchive` already
  // ran and the retry path is confusing. When the backend reports the
  // keyring is unavailable we disable the "remember in keychain" checkbox
  // and surface a short explanation instead of a generic setup-failed alert.
  // `null` while the probe is in flight so the checkbox doesn't flicker.
  const [keyringAvailable, setKeyringAvailable] = useState<boolean | null>(null)
  useEffect(() => {
    let cancelled = false
    backend
      .keyringStatus()
      .then((status) => {
        if (cancelled) return
        setKeyringAvailable(status.available)
        // If the keyring is unavailable, clear the draft so users don't ship
        // an unsatisfiable "remember it" flag into the create-archive call.
        if (!status.available && securityDraft.rememberKey) {
          onUpdateSecurityDraft({ rememberKey: false })
        }
      })
      .catch(() => {
        if (cancelled) return
        // Treat any RPC failure as "no keyring" — better to disable a
        // hopeful option than to silently fail later. The user can still
        // enter a password; they just type it on each launch.
        setKeyringAvailable(false)
        if (securityDraft.rememberKey) {
          onUpdateSecurityDraft({ rememberKey: false })
        }
      })
    return () => {
      cancelled = true
    }
    // We intentionally probe only once per mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div className="ob-panel-container">
      <div className="ob-header">
        <div className="crosshair-mark">+</div>
        <h2 className="ob-title">{t('securityTitle')}</h2>
        <p className="ob-desc">{t('securityDesc')}</p>
      </div>

      <div
        aria-label={t('encryptionModeLabel')}
        className="security-options"
        role="radiogroup"
      >
        <div
          className={`security-option ${archiveMode === 'Encrypted' ? 'selected' : ''}`}
          onClick={(event) => onSecurityCardClick('Encrypted', event.target)}
        >
          <button
            aria-checked={archiveMode === 'Encrypted'}
            aria-label={t('encryptedSelectLabel')}
            className="security-option-trigger"
            disabled={busyAction !== null}
            role="radio"
            type="button"
            onClick={() => onSelectArchiveMode('Encrypted')}
          >
            <div className="option-header">
              <div
                className={`option-radio ${archiveMode === 'Encrypted' ? 'selected' : ''}`}
              />
              <div className="option-title-row">
                <span className="option-title">🔒 {t('encryptedOption')}</span>
                <span className="tag tag-sm tag-backup">
                  {t('recommended')}
                </span>
              </div>
            </div>
          </button>
          <div className="option-body">
            <p className="option-desc">{t('encryptedDesc')}</p>
            {archiveMode === 'Encrypted' ? (
              <div className="security-form">
                <div className="form-field">
                  <label className="field-label">
                    {t('masterPasswordLabel')}
                  </label>
                  <input
                    autoComplete="new-password"
                    className="form-input"
                    placeholder={t('masterPasswordPlaceholder')}
                    type="password"
                    value={securityDraft.masterPassword}
                    onChange={(event) =>
                      onUpdateSecurityDraft({
                        masterPassword: event.target.value,
                      })
                    }
                  />
                </div>
                <div className="form-field">
                  <label className="field-label">
                    {t('confirmPasswordLabel')}
                  </label>
                  <input
                    autoComplete="new-password"
                    className="form-input"
                    placeholder={t('confirmPasswordPlaceholder')}
                    type="password"
                    value={securityDraft.confirmPassword}
                    onChange={(event) =>
                      onUpdateSecurityDraft({
                        confirmPassword: event.target.value,
                      })
                    }
                  />
                </div>
                <label className="form-checkbox-row">
                  <input
                    checked={securityDraft.rememberKey}
                    disabled={keyringAvailable === false}
                    type="checkbox"
                    onChange={(event) =>
                      onUpdateSecurityDraft({
                        rememberKey: event.target.checked,
                      })
                    }
                  />
                  <span>{t('storeInKeyring')}</span>
                </label>
                {keyringAvailable === false ? (
                  <p
                    className="text-ink-faint mt-1 text-xs"
                    data-testid="onboarding-keyring-unavailable"
                  >
                    {t('storeInKeyringUnavailable')}
                  </p>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>

        <div
          className={`security-option ${archiveMode === 'Plaintext' ? 'selected' : ''}`}
          onClick={(event) => onSecurityCardClick('Plaintext', event.target)}
        >
          <button
            aria-checked={archiveMode === 'Plaintext'}
            aria-label={t('plaintextSelectLabel')}
            className="security-option-trigger"
            disabled={busyAction !== null}
            role="radio"
            type="button"
            onClick={() => onSelectArchiveMode('Plaintext')}
          >
            <div className="option-header">
              <div
                className={`option-radio ${archiveMode === 'Plaintext' ? 'selected' : ''}`}
              />
              <div className="option-title-row">
                <span className="option-title">📄 {t('plaintextOption')}</span>
              </div>
            </div>
          </button>
          <div className="option-body">
            <p className="option-desc">{t('plaintextDesc')}</p>
            {archiveMode === 'Plaintext' ? (
              <div className="plaintext-tradeoffs">
                <div className="tradeoff-row tradeoff-pro">
                  ✓ {t('tradeoffNoPassword')}
                </div>
                <div className="tradeoff-row tradeoff-pro">
                  ✓ {t('tradeoffEasyInspect')}
                </div>
                <div className="tradeoff-row tradeoff-con">
                  ✗ {t('tradeoffVisible')}
                </div>
                <div className="tradeoff-row tradeoff-con">
                  ✗ {t('tradeoffNoUpgrade')}
                </div>
              </div>
            ) : null}
          </div>
        </div>
      </div>

      <div className="mt-4">
        <StatusCallout
          tone="warning"
          title={t('passwordWarningTitle')}
          body={t('passwordWarningBody')}
        />
      </div>

      {localError ? (
        <p className="inline-error" role="alert">
          {localError}
        </p>
      ) : null}

      <div className="ob-actions">
        <button className="btn-secondary" type="button" onClick={onBack}>
          {t('backButton')}
        </button>
        <button className="btn-primary" type="button" onClick={onContinue}>
          {t('continueButton')}
        </button>
      </div>
    </div>
  )
}
