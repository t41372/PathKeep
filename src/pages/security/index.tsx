/**
 * This module renders the Security route, where archive mode, keyring review, rekey preview, and lock-state recovery stay explicit.
 *
 * Why this file exists:
 * - Route files are where PathKeep turns design-system primitives, desktop read models, and shell scope into user-facing workflow.
 * - They should make deep links, trust copy, loading states, and repair actions obvious without forcing readers to reconstruct the whole page mentally.
 *
 * Main declarations:
 * - `SecurityPage`
 *
 * Source-of-truth notes:
 * - Stay aligned with `docs/design/screens-and-nav.md` for route purpose, navigation, and shared profile-scope rules.
 * - Stay aligned with `docs/design/ux-principles.md` for PME, trust warning grammar, and the no-hidden-state loading contract.
 */

import { useEffect, useRef, useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { useShellData } from '../../app/shell-data-context'
import { type ReviewCopyFeedback } from '../../components/review'
import { BusyOverlay } from '../../components/primitives/busy-overlay'
import { EmptyState } from '../../components/primitives/empty-state'
import { LoadingState } from '../../components/primitives/loading-state'
import { StatusCallout } from '../../components/primitives/status-callout'
import { backend } from '../../lib/backend-client'
import { useI18n } from '../../lib/i18n'
import { waitForNextPaint } from '../../lib/wait-for-next-paint'
import type { ArchiveMode, RekeyPreview, RekeyRequest } from '../../lib/types'
import { type SecurityLoadState, localizeSecurityWarning } from './helpers'
import {
  SecurityRekeyPanel,
  SecurityStatusPanel,
  SecurityUnlockPanel,
} from './panels'

/**
 * Renders the security route.
 *
 * This route should keep its deep links, loading states, trust copy, and repair affordances aligned with the Security expectations in the design docs.
 */
export function SecurityPage() {
  const { refreshAppData, refreshKey } = useShellData()
  const { language, t } = useI18n()
  const location = useLocation()
  const unlockInputRef = useRef<HTMLInputElement | null>(null)
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
  const [copyFeedback, setCopyFeedback] = useState<ReviewCopyFeedback | null>(
    null,
  )

  useEffect(() => {
    let cancelled = false
    /**
     * Loads security.
     *
     * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
     */
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
  const localizedWarnings = status
    ? status.warnings.map((warning) => localizeSecurityWarning(warning, t))
    : []

  useEffect(() => {
    if (
      location.hash !== '#unlock-archive' ||
      !status?.initialized ||
      !status.encrypted ||
      status.unlocked
    ) {
      return
    }

    const unlockInput = unlockInputRef.current
    if (!unlockInput) {
      return
    }

    unlockInput.focus()
    unlockInput.select()
    if (typeof unlockInput.scrollIntoView === 'function') {
      unlockInput.scrollIntoView({ block: 'center' })
    }
  }, [location.hash, status?.encrypted, status?.initialized, status?.unlocked])

  /**
   * Explains how reload after action works.
   *
   * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
   */
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

  /**
   * Confirms whether the archive session key actually unlocked the archive.
   *
   * `setSessionDatabaseKey` only stores the candidate key in the desktop
   * session. We still need one cheap read-model check here so the Security
   * flow can fail fast instead of leaving the user under a spinner while later
   * shell refreshes discover that the key was wrong.
   */
  async function confirmArchiveUnlocked() {
    const nextStatus = await backend.securityStatus()
    if (nextStatus.unlocked) {
      return nextStatus
    }

    await backend.clearSessionDatabaseKey().catch(() => undefined)
    throw new Error(t('security.archiveUnlockFailed'))
  }

  /**
   * Explains how with busy works.
   *
   * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
   */
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

  /**
   * Handles unlock.
   *
   * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleUnlock() {
    const trimmedKey = sessionKey.trim()
    if (!trimmedKey) {
      setActionError(t('security.currentDatabaseKeyRequired'))
      return
    }

    await withBusy(t('security.unlockArchive'), async () => {
      await backend.setSessionDatabaseKey(trimmedKey)
      await confirmArchiveUnlocked()
      await reloadAfterAction(t('security.sessionUnlocked'))
      setSessionKey('')
    })
  }

  /**
   * Handles unlock from keyring.
   *
   * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleUnlockFromKeyring() {
    await withBusy(t('security.useKeyring'), async () => {
      const key = await backend.keyringGetDatabaseKey()
      if (!key) {
        throw new Error(t('platform.keyringTitle'))
      }
      await backend.setSessionDatabaseKey(key)
      await confirmArchiveUnlocked()
      await reloadAfterAction(t('security.sessionUnlocked'))
    })
  }

  /**
   * Handles store keyring key.
   *
   * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
   */
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

  /**
   * Handles clear keyring.
   *
   * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleClearKeyring() {
    await withBusy(t('security.clearKeyring'), async () => {
      await backend.keyringClearDatabaseKey()
      await reloadAfterAction(t('security.clearKeyring'))
    })
  }

  /**
   * Handles lock archive.
   *
   * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
   */
  async function handleLockArchive() {
    await withBusy(t('security.lockArchive'), async () => {
      await backend.clearSessionDatabaseKey()
      await reloadAfterAction(t('security.sessionLocked'))
    })
  }

  /**
   * Handles preview rekey.
   *
   * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
   */
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

  /**
   * Handles execute rekey.
   *
   * Keeping this as a named declaration makes the Security surface easier to review and test than burying the behavior inside another anonymous callback.
   */
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

  if (!status && pageError === null)
    return (
      <section className="page-shell">
        <LoadingState label={t('security.loadingPosture')} />
      </section>
    )
  if (!status) {
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

      <SecurityStatusPanel
        copyFeedback={copyFeedback}
        language={language}
        localizedWarnings={localizedWarnings}
        onOpenPath={(path) => {
          void backend.openPathInFileManager(path)
        }}
        setCopyFeedback={setCopyFeedback}
        status={status}
        t={t}
      />

      <SecurityUnlockPanel
        busy={busy}
        handleClearKeyring={handleClearKeyring}
        handleLockArchive={handleLockArchive}
        handleStoreKeyringKey={handleStoreKeyringKey}
        handleUnlock={handleUnlock}
        handleUnlockFromKeyring={handleUnlockFromKeyring}
        sessionKey={sessionKey}
        setSessionKey={setSessionKey}
        status={status}
        t={t}
        unlockInputRef={unlockInputRef}
      />

      <SecurityRekeyPanel
        actionError={actionError}
        busy={busy}
        handleExecuteRekey={handleExecuteRekey}
        handlePreviewRekey={handlePreviewRekey}
        localizedWarning={(warning) => localizeSecurityWarning(warning, t)}
        notice={notice}
        preview={preview}
        rekeyConfirmText={rekeyConfirmText}
        rekeyKey={rekeyKey}
        rekeyMode={rekeyMode}
        saveRekeyKey={saveRekeyKey}
        setPreview={setPreview}
        setRekeyConfirmText={setRekeyConfirmText}
        setRekeyKey={setRekeyKey}
        setRekeyMode={setRekeyMode}
        setSaveRekeyKey={setSaveRekeyKey}
        t={t}
      />
      {busy ? <BusyOverlay label={busy} /> : null}
    </section>
  )
}
