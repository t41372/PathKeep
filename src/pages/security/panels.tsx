/**
 * @file panels.tsx
 * @description Owns the render-heavy Security route panels so the route shell can focus on loading, routing, and mutations.
 * @module pages/security
 *
 * ## Responsibilities
 * - Render the archive posture, unlock/keyring, and rekey review panels.
 * - Keep path actions, warnings, and button chrome consistent across Security flows.
 *
 * ## Not responsible for
 * - Fetching security posture or executing unlock/rekey actions
 * - Deciding whether the Security route is loading, unavailable, or uninitialized
 *
 * ## Dependencies
 * - Depends on review primitives, formatting helpers, trust-review helpers, and Security helper types.
 *
 * ## Performance notes
 * - Render-only components; keep them effect-free so unlock and rekey transitions do not add extra work to the route shell.
 */

import type { RefObject } from 'react'
import { Link } from 'react-router-dom'
import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '../../components/cards'
import {
  copyReviewValue,
  ReviewPathActionRow,
  type ReviewCopyFeedback,
} from '../../components/review'
import { StatusCallout } from '../../components/primitives/status-callout'
import { formatRelativeTime } from '../../lib/format'
import type { ResolvedLanguage } from '../../lib/i18n'
import { archiveModeKey, securityModeKey } from '../../lib/trust-review'
import type { ArchiveMode, RekeyPreview, SecurityStatus } from '../../lib/types'
import type { SecurityTranslate } from './helpers'

interface SecurityStatusPanelProps {
  copyFeedback: ReviewCopyFeedback | null
  language: ResolvedLanguage
  localizedWarnings: string[]
  onOpenPath: (path: string) => void
  setCopyFeedback: (value: ReviewCopyFeedback | null) => void
  status: SecurityStatus
  t: SecurityTranslate
}

/**
 * Renders the archive posture panel so Security route trust copy and path actions stay in one owner.
 */
export function SecurityStatusPanel({
  copyFeedback,
  language,
  localizedWarnings,
  onOpenPath,
  setCopyFeedback,
  status,
  t,
}: SecurityStatusPanelProps) {
  const copyPathValue = (key: string, value: string) => {
    void copyReviewValue(value, {
      key,
      onFeedback: setCopyFeedback,
    })
  }

  return (
    <PaperCard testId="security-status-panel" id="unlock-archive">
      <PaperCardHeader title={t('security.encryptionStatus')} />
      <PaperCardBody>
        <div className="security-status">
          <div
            className={`security-icon ${status.encrypted ? 'encrypted' : ''}`}
          >
            ⊘
          </div>
          <div className="security-info">
            <div className="security-state">
              {t('security.archiveIs', {
                mode: t(securityModeKey(status.mode)),
              })}
            </div>
            <div className="security-detail dim mono">
              {status.encrypted
                ? t('security.encryptedDetail')
                : t('security.plaintextDetail')}
            </div>
          </div>
        </div>

        <div className="detail-divider" />

        <div className="security-fields">
          <div className="config-row">
            <span className="config-label">{t('security.keyring')}</span>
            <span className="config-value">
              {status.keyringStatus.backend}
              {status.keyringStatus.storedSecret
                ? ` (${t('settings.enabled')})`
                : ` (${t('settings.disabled')})`}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">{t('security.sessionStatus')}</span>
            <span className="config-value">
              {status.unlocked
                ? t('security.sessionUnlocked')
                : t('security.sessionLocked')}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">{t('security.lastBackup')}</span>
            <span className="config-value mono">
              {status.lastSuccessfulBackupAt
                ? formatRelativeTime(status.lastSuccessfulBackupAt, language)
                : t('common.notAvailable')}
            </span>
          </div>
          <div className="config-row">
            <span className="config-label">{t('security.lastRekey')}</span>
            <span className="config-value mono">
              {status.lastRekeyAt
                ? formatRelativeTime(status.lastRekeyAt, language)
                : t('common.notAvailable')}
            </span>
          </div>
          <ReviewPathActionRow
            copyFeedback={copyFeedback}
            copyKey="security:stronghold"
            copyLabel={t('common.copyAction')}
            errorMessage={t('audit.copyFailed')}
            label={t('security.stronghold')}
            onCopy={(key, value) => copyPathValue(key, value)}
            onOpenPath={onOpenPath}
            openPathLabel={t('common.openPath')}
            successMessage={t('common.copiedNotice')}
            value={status.strongholdPath}
          />
          <ReviewPathActionRow
            copyFeedback={copyFeedback}
            copyKey="security:archive"
            copyLabel={t('common.copyAction')}
            errorMessage={t('audit.copyFailed')}
            label={t('security.archivePath')}
            onCopy={(key, value) => copyPathValue(key, value)}
            onOpenPath={onOpenPath}
            openPathLabel={t('common.openPath')}
            successMessage={t('common.copiedNotice')}
            value={status.databasePath}
          />
          {status.lastRekeySnapshotPath ? (
            <ReviewPathActionRow
              copyFeedback={copyFeedback}
              copyKey="security:last-rekey-snapshot"
              copyLabel={t('common.copyAction')}
              errorMessage={t('audit.copyFailed')}
              label={t('security.lastRekeySnapshot')}
              onCopy={(key, value) => copyPathValue(key, value)}
              onOpenPath={onOpenPath}
              openPathLabel={t('common.openAction')}
              successMessage={t('common.copiedNotice')}
              value={status.lastRekeySnapshotPath}
            />
          ) : null}
        </div>

        {status.lastRekeyRunId ? (
          <div
            className="wizard-actions"
            style={{ marginTop: 'var(--space-3)' }}
          >
            <Link
              className="btn-secondary"
              to={`/audit?run=${status.lastRekeyRunId}`}
            >
              {t('security.openLastRekeyAudit')}
            </Link>
          </div>
        ) : null}

        {localizedWarnings.map((warning) => (
          <div key={warning} className="mt-3">
            <StatusCallout tone="warning" title={warning} />
          </div>
        ))}
        {status.encrypted ? (
          <div className="mt-3">
            <StatusCallout
              tone="warning"
              title={t('security.passwordLossTitle')}
              body={t('security.passwordLossBody')}
            />
          </div>
        ) : null}
      </PaperCardBody>
    </PaperCard>
  )
}

interface SecurityUnlockPanelProps {
  busy: string | null
  handleLockArchive: () => Promise<void>
  handleUnlock: () => Promise<void>
  handleUnlockFromKeyring: () => Promise<void>
  sessionKey: string
  setSessionKey: (value: string) => void
  status: SecurityStatus
  t: SecurityTranslate
  unlockInputRef: RefObject<HTMLInputElement | null>
}

/**
 * Renders the unlock/keyring controls so the route shell only owns the action handlers and current form state.
 */
export function SecurityUnlockPanel({
  busy,
  handleLockArchive,
  handleUnlock,
  handleUnlockFromKeyring,
  sessionKey,
  setSessionKey,
  status,
  t,
  unlockInputRef,
}: SecurityUnlockPanelProps) {
  return (
    <PaperCard testId="security-unlock-panel">
      <PaperCardHeader
        title={t('security.unlockKeyringTitle')}
        right={
          <PaperCardBadge>
            {status.unlocked
              ? t('security.sessionActive')
              : t('security.needsUnlock')}
          </PaperCardBadge>
        }
      />
      <PaperCardBody>
        <div className="security-form-grid">
          <label className="field-stack">
            <span className="mono-kicker">
              {t('security.currentDatabaseKey')}
            </span>
            <input
              aria-label={t('security.currentDatabaseKey')}
              autoComplete="current-password"
              ref={unlockInputRef}
              type="password"
              value={sessionKey}
              onChange={(event) => setSessionKey(event.target.value)}
              placeholder={t('security.currentDatabaseKeyPlaceholder')}
            />
          </label>
        </div>

        <div className="wizard-actions">
          {!status.unlocked && status.encrypted ? (
            <>
              <button
                className="btn-primary"
                type="button"
                onClick={() => void handleUnlock()}
              >
                {busy === t('security.unlockArchive')
                  ? busy
                  : t('security.unlockArchive')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                onClick={() => void handleUnlockFromKeyring()}
              >
                {busy === t('security.useKeyring')
                  ? busy
                  : t('security.useKeyring')}
              </button>
            </>
          ) : status.encrypted ? (
            <button
              className="btn-secondary"
              type="button"
              onClick={() => void handleLockArchive()}
            >
              {busy === t('security.lockArchive')
                ? busy
                : t('security.lockArchive')}
            </button>
          ) : null}
        </div>
      </PaperCardBody>
    </PaperCard>
  )
}

interface SecurityRekeyPanelProps {
  actionError: string | null
  busy: string | null
  handleExecuteRekey: () => Promise<void>
  handlePreviewRekey: () => Promise<void>
  notice: string | null
  preview: RekeyPreview | null
  rekeyConfirmText: string
  rekeyKey: string
  rekeyMode: ArchiveMode
  saveRekeyKey: boolean
  setRekeyConfirmText: (value: string) => void
  setRekeyKey: (value: string) => void
  setRekeyMode: (value: ArchiveMode) => void
  setPreview: (value: RekeyPreview | null) => void
  setSaveRekeyKey: (value: boolean) => void
  t: SecurityTranslate
  localizedWarning: (warning: string) => string
}

/**
 * Renders the rekey preview/execute surface so the route shell only keeps the mutation state machine.
 */
export function SecurityRekeyPanel({
  actionError,
  busy,
  handleExecuteRekey,
  handlePreviewRekey,
  notice,
  preview,
  rekeyConfirmText,
  rekeyKey,
  rekeyMode,
  saveRekeyKey,
  setRekeyConfirmText,
  setRekeyKey,
  setRekeyMode,
  setPreview,
  setSaveRekeyKey,
  t,
  localizedWarning,
}: SecurityRekeyPanelProps) {
  return (
    <PaperCard testId="security-rekey-panel">
      <PaperCardHeader
        title={t('security.rekeyTitle')}
        right={
          <PaperCardBadge>{t('security.previewBeforeExecute')}</PaperCardBadge>
        }
      />
      <PaperCardBody>
        <div className="security-form-grid">
          <label className="field-stack">
            <span className="mono-kicker">{t('security.targetMode')}</span>
            <select
              aria-label={t('security.targetMode')}
              value={rekeyMode}
              onChange={(event) => {
                setPreview(null)
                setRekeyConfirmText('')
                setRekeyMode(event.target.value as ArchiveMode)
              }}
            >
              <option value="Encrypted">
                {t(archiveModeKey('Encrypted'))}
              </option>
              <option value="Plaintext">
                {t(archiveModeKey('Plaintext'))}
              </option>
            </select>
          </label>
          {rekeyMode === 'Encrypted' ? (
            <label className="field-stack">
              <span className="mono-kicker">
                {t('security.newDatabaseKey')}
              </span>
              <input
                aria-label={t('security.newDatabaseKey')}
                autoComplete="new-password"
                type="password"
                value={rekeyKey}
                onChange={(event) => setRekeyKey(event.target.value)}
                placeholder={t('security.newDatabaseKeyPlaceholder')}
              />
            </label>
          ) : null}
        </div>

        {rekeyMode === 'Encrypted' ? (
          <label
            className="form-checkbox-row"
            style={{ marginTop: 'var(--space-3)' }}
          >
            <input
              type="checkbox"
              checked={saveRekeyKey}
              onChange={(event) => setSaveRekeyKey(event.target.checked)}
            />
            <span>{t('security.storeNewKey')}</span>
          </label>
        ) : null}

        {rekeyMode === 'Plaintext' && preview !== null ? (
          <label
            className="field-stack"
            style={{ marginTop: 'var(--space-3)' }}
          >
            <span className="mono-kicker">
              {t('security.rekeyConfirmLabel')}
            </span>
            <input
              aria-label={t('security.rekeyConfirmLabel')}
              autoComplete="off"
              type="text"
              value={rekeyConfirmText}
              onChange={(event) => setRekeyConfirmText(event.target.value)}
              placeholder={t('security.rekeyConfirmPlaceholder')}
            />
          </label>
        ) : null}

        <div className="wizard-actions">
          <button
            className="btn-secondary"
            type="button"
            onClick={() => void handlePreviewRekey()}
          >
            {busy === t('security.previewRekey')
              ? busy
              : t('security.previewRekey')}
          </button>
          <button
            className={rekeyMode === 'Plaintext' ? 'btn-danger' : 'btn-primary'}
            type="button"
            disabled={
              preview === null ||
              (rekeyMode === 'Plaintext' && rekeyConfirmText !== 'confirm')
            }
            onClick={() => void handleExecuteRekey()}
          >
            {busy === t('security.executeRekey')
              ? busy
              : t('security.executeRekey')}
          </button>
        </div>

        {preview ? (
          <div className="manual-steps" style={{ marginTop: 'var(--space-4)' }}>
            <div className="manual-step">
              <span className="step-num-inline mono">{t('security.mode')}</span>
              <span>
                {t(archiveModeKey(preview.currentMode))} →{' '}
                {t(archiveModeKey(preview.nextMode))}
              </span>
            </div>
            <div className="manual-step">
              <span className="step-num-inline mono">
                {t('security.snapshot')}
              </span>
              <span className="mono">{preview.snapshotPath}</span>
            </div>
            <div className="manual-step">
              <span className="step-num-inline mono">
                {t('security.temporaryDatabase')}
              </span>
              <span className="mono">{preview.tempDatabasePath}</span>
            </div>
            {preview.steps.map((step, index) => (
              <div key={step} className="manual-step">
                <span className="step-num-inline mono">{index + 1}</span>
                <span>{step}</span>
              </div>
            ))}
            {preview.warnings.map((warning) => (
              <div key={warning} className="mt-3">
                <StatusCallout
                  tone="warning"
                  title={localizedWarning(warning)}
                />
              </div>
            ))}
          </div>
        ) : null}

        {notice ? <p className="mono-support">{notice}</p> : null}
        {actionError ? (
          <p className="inline-error" role="alert">
            {actionError}
          </p>
        ) : null}
      </PaperCardBody>
    </PaperCard>
  )
}
