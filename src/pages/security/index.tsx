import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { BusyOverlay } from '../../components/primitives/busy-overlay'
import { EmptyState } from '../../components/primitives/empty-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { backend } from '../../lib/backend'
import { formatRelativeTime } from '../../lib/format'
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

function securityModeLabel(status: SecurityStatus) {
  if (status.mode === 'uninitialized') return 'UNINITIALIZED'
  if (status.mode === 'locked') return 'ENCRYPTED / LOCKED'
  if (status.mode === 'encrypted') return 'ENCRYPTED'
  return 'PLAINTEXT'
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
  const [loadState, setLoadState] = useState<SecurityLoadState>({
    status: null,
    error: null,
  })
  const [sessionKey, setSessionKey] = useState('')
  const [rekeyMode, setRekeyMode] = useState<ArchiveMode>('Encrypted')
  const [rekeyKey, setRekeyKey] = useState('')
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
                : 'PathKeep could not read the current security posture.',
          })
      }
    }
    void loadSecurity()
    return () => {
      cancelled = true
    }
  }, [refreshKey])

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
          : 'PathKeep could not complete the security action.',
      )
    } finally {
      setBusy(null)
    }
  }

  async function handleUnlock() {
    const trimmedKey = sessionKey.trim()
    if (!trimmedKey) {
      setActionError('Enter the current archive key before unlocking.')
      return
    }

    await withBusy('Unlocking archive', async () => {
      await backend.setSessionDatabaseKey(trimmedKey)
      await reloadAfterAction('Archive unlocked for Explorer and Audit.')
      setSessionKey('')
    })
  }

  async function handleUnlockFromKeyring() {
    await withBusy('Reading keyring', async () => {
      const key = await backend.keyringGetDatabaseKey()
      if (!key) {
        throw new Error('No database key is stored in the native keyring.')
      }
      await backend.setSessionDatabaseKey(key)
      await reloadAfterAction('Archive unlocked using the native keyring.')
    })
  }

  async function handleStoreKeyringKey() {
    const trimmedKey = sessionKey.trim()
    if (!trimmedKey) {
      setActionError('Enter a database key before storing it in the keyring.')
      return
    }

    await withBusy('Saving keyring secret', async () => {
      await backend.keyringStoreDatabaseKey(trimmedKey)
      await reloadAfterAction('Database key saved to the native keyring.')
    })
  }

  async function handleClearKeyring() {
    await withBusy('Clearing keyring secret', async () => {
      await backend.keyringClearDatabaseKey()
      await reloadAfterAction('Stored database key removed from the keyring.')
    })
  }

  async function handleLockArchive() {
    await withBusy('Locking archive', async () => {
      await backend.clearSessionDatabaseKey()
      await reloadAfterAction(
        'Archive session cleared. Explorer and Audit are locked again.',
      )
    })
  }

  async function handlePreviewRekey() {
    if (!status?.initialized) return

    const request: RekeyRequest = {
      newMode: rekeyMode,
      newKey: rekeyMode === 'Encrypted' ? rekeyKey : null,
    }

    await withBusy('Previewing re-key', async () => {
      const nextPreview = await backend.previewRekeyArchive(request)
      setPreview(nextPreview)
      setNotice(
        'Preview generated. Review the snapshot path and rewrite steps before execute.',
      )
    })
  }

  async function handleExecuteRekey() {
    if (!status?.initialized) return

    const request: RekeyRequest = {
      newMode: rekeyMode,
      newKey: rekeyMode === 'Encrypted' ? rekeyKey : null,
    }

    if (request.newMode === 'Encrypted' && !request.newKey?.trim()) {
      setActionError(
        'Encrypted re-key needs a new database key before execute.',
      )
      return
    }

    await withBusy('Executing re-key', async () => {
      await backend.rekeyArchive(request)
      if (request.newMode === 'Encrypted' && saveRekeyKey && request.newKey) {
        await backend.keyringStoreDatabaseKey(request.newKey)
      }
      if (request.newMode === 'Plaintext') {
        await backend.keyringClearDatabaseKey()
      }
      await reloadAfterAction(
        'Archive re-key completed. Review the safety snapshot and verify Explorer access.',
      )
      setPreview(null)
      setRekeyKey('')
    })
  }

  if (loading && !snapshot)
    return (
      <section className="page-shell">
        <LoadingState label="Loading security posture" />
      </section>
    )
  if (!snapshot || !status) {
    return (
      <section className="page-shell">
        <EmptyState
          description={
            pageError ??
            'PathKeep needs the local app snapshot before it can describe the current encryption and keyring posture.'
          }
          eyebrow="SECURITY"
          title="Security posture is unavailable"
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
              Initialize archive first
            </Link>
          }
          description="Security review becomes meaningful after onboarding creates the archive and the first backup writes the baseline manifest chain."
          eyebrow="SECURITY"
          title="The archive has not been initialized yet"
        />
      </section>
    )
  }

  return (
    <section className="page-shell security-page" data-testid="security-page">
      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">ENCRYPTION STATUS</span>
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
                Archive is <strong>{securityModeLabel(status)}</strong>
              </div>
              <div className="security-detail dim mono">
                {status.encrypted
                  ? 'SQLCipher at rest · unlock required before read access'
                  : 'Standard SQLite archive · disk encryption depends on the host system'}
              </div>
            </div>
          </div>

          <div className="detail-divider" />

          <div className="security-fields">
            <div className="config-row">
              <span className="config-label">Keyring</span>
              <span className="config-value">
                {status.keyringStatus.backend}
                {status.keyringStatus.storedSecret
                  ? ' (database key stored)'
                  : ' (no stored database key)'}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">Session Status</span>
              <span className="config-value">
                {status.unlocked
                  ? 'Archive is currently unlocked'
                  : 'Archive is locked — Explorer and Audit remain read-blocked'}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">Last Backup</span>
              <span className="config-value mono">
                {status.lastSuccessfulBackupAt
                  ? formatRelativeTime(status.lastSuccessfulBackupAt)
                  : 'N/A'}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">Stronghold</span>
              <span className="config-value mono dim">
                {status.strongholdPath}
              </span>
            </div>
            <div className="config-row">
              <span className="config-label">Archive Path</span>
              <span className="config-value mono dim">
                {status.databasePath}
              </span>
            </div>
          </div>

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
                <strong>Password loss = data loss.</strong> PathKeep does not
                have a recovery backdoor. Keep the current or future database
                key in a secure place before re-keying.
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">UNLOCK + KEYRING</span>
          <span className="panel-action">
            {status.unlocked ? 'Session active' : 'Needs unlock'}
          </span>
        </div>
        <div className="panel-body">
          <div className="security-form-grid">
            <label className="field-stack">
              <span className="mono-kicker">CURRENT DATABASE KEY</span>
              <input
                aria-label="Current database key"
                type="password"
                value={sessionKey}
                onChange={(event) => setSessionKey(event.target.value)}
                placeholder="Enter current archive key"
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
                  {busy === 'Unlocking archive' ? busy : 'Unlock archive'}
                </button>
                <button
                  className="btn-secondary"
                  type="button"
                  onClick={() => void handleUnlockFromKeyring()}
                >
                  {busy === 'Reading keyring' ? busy : 'Use keyring'}
                </button>
              </>
            ) : status.encrypted ? (
              <button
                className="btn-secondary"
                type="button"
                onClick={() => void handleLockArchive()}
              >
                {busy === 'Locking archive' ? busy : 'Lock archive'}
              </button>
            ) : null}
            <button
              className="btn-secondary"
              type="button"
              onClick={() => void handleStoreKeyringKey()}
            >
              {busy === 'Saving keyring secret' ? busy : 'Store in keyring'}
            </button>
            <button
              className="btn-secondary"
              type="button"
              onClick={() => void handleClearKeyring()}
            >
              {busy === 'Clearing keyring secret' ? busy : 'Clear keyring'}
            </button>
          </div>
          <p className="mono-support">
            Storing the key in the native keyring is optional convenience
            unlock. PathKeep still keeps the archive local-first and does not
            upload secrets anywhere.
          </p>
        </div>
      </div>

      <div className="panel">
        <div className="panel-header">
          <span className="panel-title">RE-KEY PREVIEW</span>
          <span className="panel-action">Preview before execute</span>
        </div>
        <div className="panel-body">
          <div className="security-form-grid">
            <label className="field-stack">
              <span className="mono-kicker">TARGET MODE</span>
              <select
                aria-label="Target archive mode"
                value={rekeyMode}
                onChange={(event) => {
                  setPreview(null)
                  setRekeyMode(event.target.value as ArchiveMode)
                }}
              >
                <option value="Encrypted">Encrypted</option>
                <option value="Plaintext">Plaintext</option>
              </select>
            </label>
            {rekeyMode === 'Encrypted' && (
              <label className="field-stack">
                <span className="mono-kicker">NEW DATABASE KEY</span>
                <input
                  aria-label="New database key"
                  type="password"
                  value={rekeyKey}
                  onChange={(event) => setRekeyKey(event.target.value)}
                  placeholder="Enter the replacement archive key"
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
              <span>
                Store the new database key in the native keyring after execute
              </span>
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
              {busy === 'Previewing re-key' ? busy : 'Preview re-key'}
            </button>
            <button
              className={
                rekeyMode === 'Plaintext' ? 'btn-danger' : 'btn-primary'
              }
              type="button"
              disabled={preview === null}
              onClick={() => {
                void handleExecuteRekey()
              }}
            >
              {busy === 'Executing re-key' ? busy : 'Execute re-key'}
            </button>
          </div>

          {preview && (
            <div
              className="manual-steps"
              style={{ marginTop: 'var(--space-4)' }}
            >
              <div className="manual-step">
                <span className="step-num-inline mono">MODE</span>
                <span>
                  {preview.currentMode} → {preview.nextMode}
                </span>
              </div>
              <div className="manual-step">
                <span className="step-num-inline mono">SNAPSHOT</span>
                <span className="mono">{preview.snapshotPath}</span>
              </div>
              <div className="manual-step">
                <span className="step-num-inline mono">TEMP</span>
                <span className="mono">{preview.tempDatabasePath}</span>
              </div>
              {preview.steps.map((step, index) => (
                <div key={step} className="manual-step">
                  <span className="step-num-inline mono">
                    {String(index + 1).padStart(2, '0')}
                  </span>
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
