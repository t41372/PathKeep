import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { BusyOverlay } from '../../components/primitives/busy-overlay'
import { EmptyState } from '../../components/primitives/empty-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { formatRelativeTime } from '../../lib/format'
import { useI18n } from '../../lib/i18n'
import { archiveModeKey, securityModeKey } from '../../lib/trust-review'
import type {
  ArchiveMode,
  RekeyPreview,
  RekeyRequest,
  SecurityStatus,
} from '../../lib/types'

interface SecurityLoadState {
  status: SecurityStatus | null
  error: string | null
}

function waitForNextPaint() {
  return new Promise<void>((resolve) => {
    if (typeof window === 'undefined') {
      resolve()
      return
    }
    window.requestAnimationFrame(() => resolve())
  })
}

export function SecurityPage() {
  const { loading, refreshAppData, refreshKey, snapshot } = useShellData()
  const { language, t } = useI18n()
  const [loadState, setLoadState] = useState<SecurityLoadState>({
    status: null,
    error: null,
  })
  const [sessionKey, setSessionKey] = useState('')
  const [rekeyMode, setRekeyMode] = useState<ArchiveMode>('Encrypted')
  const [rekeyKey, setRekeyKey] = useState('')
  const [rekeyConfirmText, setRekeyConfirmText] = useState('')
  const [saveRekeyKey, setSaveRekeyKey] = useState(false)
  const [preview, setPreview] = useState<RekeyPreview | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    const loadSecurity = async () => {
      try {
        const nextStatus = await backend.securityStatus()
        if (!cancelled) {
          setLoadState({
            status: nextStatus,
            error: null,
          })
          setRekeyMode(nextStatus.encrypted ? 'Plaintext' : 'Encrypted')
        }
      } catch (nextError) {
        if (!cancelled)
          setLoadState({
            status: null,
            error:
              nextError instanceof Error
                ? nextError.message
                : t('security.unavailableBody'),
          })
      }
    }
    void loadSecurity()
    return () => {
      cancelled = true
    }
  }, [refreshKey, t])

  const status = loadState.status
  const pageError = loadState.error

  async function reloadAfterAction(nextNotice?: string) {
    await refreshAppData()
    const nextStatus = await backend.securityStatus()
    setLoadState({
      status: nextStatus,
      error: null,
    })
    setRekeyMode(nextStatus.encrypted ? 'Plaintext' : 'Encrypted')
    setNotice(nextNotice ?? null)
  }

  async function withBusy<T>(label: string, fn: () => Promise<T>) {
    setBusy(label)
    setActionError(null)
    setNotice(null)

    try {
      await waitForNextPaint()
      return await fn()
    } catch (nextError) {
      setActionError(
        nextError instanceof Error
          ? nextError.message
          : t('common.unavailable'),
      )
    } finally {
      setBusy(null)
    }
  }

  async function handleUnlock() {
    const trimmedKey = sessionKey.trim()
    if (!trimmedKey) {
      setActionError(t('security.currentDatabaseKeyRequired'))
      return
    }

    await withBusy(t('security.unlockArchive'), async () => {
      await backend.setSessionDatabaseKey(trimmedKey)
      await reloadAfterAction(t('security.sessionUnlocked'))
      setSessionKey('')
    })
  }

  async function handleUnlockFromKeyring() {
    await withBusy(t('security.useKeyring'), async () => {
      const key = await backend.keyringGetDatabaseKey()
      if (!key) {
        throw new Error(t('platform.keyringTitle'))
      }
      await backend.setSessionDatabaseKey(key)
      await reloadAfterAction(t('security.sessionUnlocked'))
    })
  }

  async function handleStoreKeyringKey() {
    const trimmedKey = sessionKey.trim()
    if (!trimmedKey) {
      setActionError(t('security.currentDatabaseKeyRequired'))
      return
    }

    await withBusy(t('security.storeInKeyring'), async () => {
      await backend.keyringStoreDatabaseKey(trimmedKey)
      await reloadAfterAction(t('security.storeInKeyring'))
    })
  }

  async function handleClearKeyring() {
    await withBusy(t('security.clearKeyring'), async () => {
      await backend.keyringClearDatabaseKey()
      await reloadAfterAction(t('security.clearKeyring'))
    })
  }

  async function handleLockArchive() {
    await withBusy(t('security.lockArchive'), async () => {
      await backend.clearSessionDatabaseKey()
      await reloadAfterAction(t('security.sessionLocked'))
    })
  }

  async function handlePreviewRekey() {
    if (!status?.initialized) return

    const request: RekeyRequest = {
      newMode: rekeyMode,
      newKey: rekeyMode === 'Encrypted' ? rekeyKey : null,
    }

    await withBusy(t('security.previewRekey'), async () => {
      const nextPreview = await backend.previewRekeyArchive(request)
      setPreview(nextPreview)
      setNotice(t('security.previewBeforeExecute'))
    })
  }

  async function handleExecuteRekey() {
    if (!status?.initialized) return

    const request: RekeyRequest = {
      newMode: rekeyMode,
      newKey: rekeyMode === 'Encrypted' ? rekeyKey : null,
    }

    if (request.newMode === 'Encrypted' && !request.newKey?.trim()) {
      setActionError(t('security.newDatabaseKeyRequired'))
      return
    }

    await withBusy(t('security.executeRekey'), async () => {
      await backend.rekeyArchive(request)
      if (request.newMode === 'Encrypted' && saveRekeyKey && request.newKey) {
        await backend.keyringStoreDatabaseKey(request.newKey)
      }
      if (request.newMode === 'Plaintext') {
        await backend.keyringClearDatabaseKey()
      }
      await reloadAfterAction(t('security.executeRekey'))
      setPreview(null)
      setRekeyKey('')
      setRekeyConfirmText('')
    })
  }

  if (loading && !snapshot)
    return (
      <section className="page-shell">
        <LoadingState label={t('security.loadingPosture')} />
      </section>
    )
  if (!snapshot || !status) {
    return (
      <section className="page-shell">
        <EmptyState
          description={pageError ?? t('security.unavailableBody')}
          eyebrow={t('navigation.securityLabel')}
          title={t('security.unavailableTitle')}
        />
      </section>
    )
  }

  if (!status.initialized) {
    return (
      <section className="page-shell">
        <EmptyState
          action={
            <Link className="btn-primary" to="/onboarding">
              {t('security.initFirstAction')}
            </Link>
          }
          description={t('security.notInitializedBody')}
          eyebrow={t('navigation.securityLabel')}
          title={t('security.notInitializedTitle')}
        />
      </section>
    )
  }

  return (
    <section className="page-shell security-page" data-testid="security-page">
      {!status.keyringStatus.available ? (
        <StatusCallout
          tone="blocked"
          title={t('platform.keyringTitle')}
          body={t('platform.keyringBody')}
          actions={
            <Link className="btn-secondary" to="/settings">
              {t('navigation.settingsLabel')}
            </Link>
          }
        />
      ) : null}

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('security.encryptionStatus')}</span>
        </div>
        <div className="panel-body">
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
              <span className="config-label">
                {t('security.sessionStatus')}
              </span>
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
            <div className="config-row">
              <span className="config-label">{t('security.stronghold')}</span>
              <span className="config-value mono dim">
                {status.strongholdPath}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">{t('security.archivePath')}</span>
              <span className="config-value mono dim">
                {status.databasePath}
              </span>
            </div>
            {status.lastRekeySnapshotPath ? (
              <div className="config-row">
                <span className="config-label">
                  {t('security.lastRekeySnapshot')}
                </span>
                <span className="config-value mono dim">
                  {status.lastRekeySnapshotPath}
                </span>
                <button
                  className="btn-tiny"
                  type="button"
                  onClick={() => {
                    void backend.openPathInFileManager(
                      status.lastRekeySnapshotPath ?? '',
                    )
                  }}
                >
                  {t('common.openAction')}
                </button>
              </div>
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

          {status.warnings.map((warning) => (
            <div key={warning} className="warning-box">
              <div className="warning-icon">⚠</div>
              <div className="warning-text">{warning}</div>
            </div>
          ))}
          {status.encrypted && (
            <div className="warning-box">
              <div className="warning-icon">⚠</div>
              <div className="warning-text">
                <strong>{t('security.passwordLossTitle')}</strong>{' '}
                {t('security.passwordLossBody')}
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">
            {t('security.unlockKeyringTitle')}
          </span>
          <span className="panel-action">
            {status.unlocked
              ? t('security.sessionActive')
              : t('security.needsUnlock')}
          </span>
        </div>
        <div className="panel-body">
          <div className="security-form-grid">
            <label className="field-stack">
              <span className="mono-kicker">
                {t('security.currentDatabaseKey')}
              </span>
              <input
                aria-label={t('security.currentDatabaseKey')}
                autoComplete="current-password"
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
            <button
              className="btn-secondary"
              type="button"
              onClick={() => void handleStoreKeyringKey()}
            >
              {busy === t('security.storeInKeyring')
                ? busy
                : t('security.storeInKeyring')}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => void handleClearKeyring()}
            >
              {busy === t('security.clearKeyring')
                ? busy
                : t('security.clearKeyring')}
            </button>
          </div>
          <p className="mono-support">{t('security.keyringConvenience')}</p>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">{t('security.rekeyTitle')}</span>
          <span className="panel-action">
            {t('security.previewBeforeExecute')}
          </span>
        </div>
        <div className="panel-body">
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
            {rekeyMode === 'Encrypted' && (
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
            )}
          </div>

          {rekeyMode === 'Encrypted' && (
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
          )}

          {rekeyMode === 'Plaintext' && preview !== null && (
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
          )}

          <div className="wizard-actions">
            <button
              className="btn-secondary"
              type="button"
              onClick={() => {
                void handlePreviewRekey()
              }}
            >
              {busy === t('security.previewRekey')
                ? busy
                : t('security.previewRekey')}
            </button>
            <button
              className={
                rekeyMode === 'Plaintext' ? 'btn-danger' : 'btn-primary'
              }
              type="button"
              disabled={
                preview === null ||
                (rekeyMode === 'Plaintext' && rekeyConfirmText !== 'confirm')
              }
              onClick={() => {
                void handleExecuteRekey()
              }}
            >
              {busy === t('security.executeRekey')
                ? busy
                : t('security.executeRekey')}
            </button>
          </div>

          {preview && (
            <div
              className="manual-steps"
              style={{ marginTop: 'var(--space-4)' }}
            >
              <div className="manual-step">
                <span className="step-num-inline mono">
                  {t('security.mode')}
                </span>
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
                <div key={warning} className="warning-box">
                  <div className="warning-icon">⚠</div>
                  <div className="warning-text">{warning}</div>
                </div>
              ))}
            </div>
          )}

          {notice ? <p className="mono-support">{notice}</p> : null}
          {actionError ? (
            <p className="inline-error" role="alert">
              {actionError}
            </p>
          ) : null}
        </div>
      </div>
      {busy ? <BusyOverlay label={busy} /> : null}
    </section>
  )
}
