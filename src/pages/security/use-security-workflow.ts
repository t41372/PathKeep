/**
 * @file use-security-workflow.ts
 * @description Security route workflow hook for posture loading, unlock/keyring actions, and rekey mutations.
 * @module pages/security
 *
 * ## Responsibilities
 * - Own the Security route read-model load and refresh-after-action sequence.
 * - Keep unlock, keyring, lock, preview, and execute-rekey mutations in one testable workflow owner.
 * - Publish the form, busy, notice, and action-error state consumed by the render-only Security panels.
 *
 * ## Not responsible for
 * - Rendering Security panels, route fallbacks, hash focus, or path-copy feedback.
 * - Changing backend command payloads or Security DTO shapes.
 * - Owning app-shell refresh logic beyond calling the injected `refreshAppData` hook after successful actions.
 *
 * ## Dependencies
 * - Depends on the backend client Security, keyring, and rekey commands.
 * - Depends on the shell refresh callback supplied by `useShellData()`.
 * - Consumed by `src/pages/security/index.tsx`.
 *
 * ## Performance notes
 * - All expensive rekey work remains behind backend commands; this hook only coordinates async calls and paint-first busy state.
 * - `waitForNextPaint()` runs before long actions so the route can show busy feedback before native work starts.
 */

import { useEffect, useState } from 'react'
import { backend } from '../../lib/backend-client'
import { waitForNextPaint } from '../../lib/wait-for-next-paint'
import type { ArchiveMode, RekeyPreview, RekeyRequest } from '../../lib/types'
import type { SecurityLoadState, SecurityTranslate } from './helpers'

interface SecurityWorkflowOptions {
  refreshAppData: () => Promise<void>
  refreshKey: number
  t: SecurityTranslate
}

/**
 * Coordinates Security route actions without making the route component own every mutation branch.
 *
 * The route passes shell refresh and translator dependencies in, then receives the same state and handlers
 * its panels already consume; this preserves the public UI contract while making the workflow reusable in tests.
 */
export function useSecurityWorkflow({
  refreshAppData,
  refreshKey,
  t,
}: SecurityWorkflowOptions) {
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
        if (!cancelled) {
          setLoadState({
            status: null,
            error:
              nextError instanceof Error
                ? nextError.message
                : t('security.unavailableBody'),
          })
        }
      }
    }

    void loadSecurity()
    return () => {
      cancelled = true
    }
  }, [refreshKey, t])

  const status = loadState.status
  const pageError = loadState.error

  const reloadAfterAction = async (nextNotice?: string) => {
    await refreshAppData()
    const nextStatus = await backend.securityStatus()
    setLoadState({
      status: nextStatus,
      error: null,
    })
    setRekeyMode(nextStatus.encrypted ? 'Plaintext' : 'Encrypted')
    setNotice(nextNotice ?? null)
  }

  const confirmArchiveUnlocked = async () => {
    const nextStatus = await backend.securityStatus()
    if (nextStatus.unlocked) {
      return nextStatus
    }

    await backend.clearSessionDatabaseKey().catch(() => undefined)
    throw new Error(t('security.archiveUnlockFailed'))
  }

  const withBusy = async <T>(label: string, fn: () => Promise<T>) => {
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

  const handleUnlock = async () => {
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

  const handleUnlockFromKeyring = async () => {
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

  const handleStoreKeyringKey = async () => {
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

  const handleClearKeyring = async () => {
    await withBusy(t('security.clearKeyring'), async () => {
      await backend.keyringClearDatabaseKey()
      await reloadAfterAction(t('security.clearKeyring'))
    })
  }

  const handleLockArchive = async () => {
    await withBusy(t('security.lockArchive'), async () => {
      await backend.clearSessionDatabaseKey()
      await reloadAfterAction(t('security.sessionLocked'))
    })
  }

  const handlePreviewRekey = async () => {
    if (!status?.initialized) {
      return
    }

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

  const handleExecuteRekey = async () => {
    if (!status?.initialized) {
      return
    }

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

  return {
    actionError,
    busy,
    handleClearKeyring,
    handleExecuteRekey,
    handleLockArchive,
    handlePreviewRekey,
    handleStoreKeyringKey,
    handleUnlock,
    handleUnlockFromKeyring,
    loadState,
    notice,
    pageError,
    preview,
    rekeyConfirmText,
    rekeyKey,
    rekeyMode,
    saveRekeyKey,
    sessionKey,
    setPreview,
    setRekeyConfirmText,
    setRekeyKey,
    setRekeyMode,
    setSaveRekeyKey,
    setSessionKey,
    status,
  }
}
