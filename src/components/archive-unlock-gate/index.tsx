/**
 * @file index.tsx
 * @description Blocking inline unlock gate rendered above all routes when the archive is
 * encrypted and the session has no key — replacing the old sidebar-nudge-to-Settings flow.
 * @module components/archive-unlock-gate
 *
 * ## Responsibilities
 * - Render a focused, paper-aesthetic overlay dialog when the archive needs a key.
 * - Offer manual password entry, a "Use saved password" shortcut (when keychain has one),
 *   and a "Remember on this device" checkbox (checked by default when keychain is available).
 * - After a successful unlock: call `reconcileArchiveEncryption` fire-and-forget, then
 *   refresh app data. When `retryBackupOnUnlock` is set, call `onRetryBackup`.
 * - Offer a calm "Can't unlock? Recover from a snapshot" escape hatch that swaps the dialog
 *   body for a `SnapshotRecoveryPanel` — so a corrupt-archive user whose correct password keeps
 *   failing is never dead-ended out of restoring a snapshot.
 * - Full a11y: role="dialog", aria-modal, focus trap. Escape does NOT dismiss (app
 *   is unusable while locked).
 *
 * ## Not responsible for
 * - Rendering route content or deciding which route is active.
 * - Storing the session database key beyond the current form state.
 * - Orchestrating backup scheduling or schedule health probes.
 *
 * ## Dependencies
 * - Uses `useShellData()` for snapshot, saveConfig, refreshAppData.
 * - Uses `backend` client for keyring and session-key commands.
 * - Uses `useI18n('security')` for all copy.
 *
 * ## Performance notes
 * - Focus trap is a lightweight keyboard listener. The gate is only mounted when locked,
 *   so it never adds overhead to the unlocked main path.
 */

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react'
import { useShellData } from '../../app/shell-data-context'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { useI18n } from '../../lib/i18n'
import type { AppSnapshot } from '../../lib/types'
import { SnapshotRecoveryPanel } from '../snapshot-restore'

interface ArchiveUnlockGateProps {
  /** The current shell snapshot — always truthy when this gate is mounted. */
  snapshot: AppSnapshot
  /**
   * When true, a successful unlock will trigger `onRetryBackup` after
   * refreshing app data. Set when the last backup failed with a lock error.
   */
  retryBackupOnUnlock?: boolean
  /**
   * Called after a successful unlock when `retryBackupOnUnlock` is true.
   * Callers should fire-and-forget their backup action here.
   */
  onRetryBackup?: () => void
}

/**
 * Renders the blocking archive-unlock overlay. Mounts only when the archive is
 * encrypted and the current session is locked; unmounts after a successful unlock
 * (because `archiveStatus.unlocked` becomes true and the parent gates on it).
 */
export function ArchiveUnlockGate({
  snapshot,
  retryBackupOnUnlock = false,
  onRetryBackup,
}: ArchiveUnlockGateProps) {
  const { refreshAppData, saveConfig, runFullArchiveRestore } = useShellData()
  const { t } = useI18n('security')

  const [sessionKey, setSessionKey] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // Which body the shared dialog shell shows: the password form, or the snapshot
  // recovery escape hatch for a user whose (even correct) password keeps failing.
  const [mode, setMode] = useState<'unlock' | 'recover'>('unlock')

  const keyringAvailable = snapshot.keyringStatus.available
  const keyringStored = snapshot.keyringStatus.storedSecret
  const keyringBackend = snapshot.keyringStatus.backend

  // "Remember on this device" defaults to checked when the keychain is available.
  const [rememberChecked, setRememberChecked] = useState(keyringAvailable)

  const passwordRef = useRef<HTMLInputElement | null>(null)
  const recoverHeadingRef = useRef<HTMLHeadingElement | null>(null)
  const dialogRef = useRef<HTMLDivElement | null>(null)
  // Skips the mode-swap focus effect on the very first render (the mount effect
  // below already places initial focus on the password field).
  const modeInitialized = useRef(false)

  // Autofocus the password input on mount, and restore focus to whatever was
  // focused before the gate appeared once it unmounts (best-effort a11y).
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null
    passwordRef.current?.focus()
    return () => {
      // Stryker disable next-line OptionalChaining: defensive — element may be gone.
      previouslyFocused?.focus?.()
    }
  }, [])

  // Keep focus inside the dialog when the body swaps between unlock and recover.
  // The control that triggered the swap (the escape hatch / "Back to unlock")
  // unmounts with its branch, which would otherwise drop focus to document.body
  // and let Tab escape the modal. Move focus to the new body's anchor instead:
  // the recover heading (announces the new context to a screen reader) or the
  // password field (mirrors the mount autofocus).
  useEffect(() => {
    if (!modeInitialized.current) {
      modeInitialized.current = true
      return
    }
    if (mode === 'recover') {
      recoverHeadingRef.current?.focus()
    } else {
      passwordRef.current?.focus()
    }
  }, [mode])

  // Defense-in-depth focus recapture. In recover mode the embedded
  // SnapshotRecoveryPanel renders over a still-focusable shell (the sidebar /
  // main / status bar behind this gate are NOT inert). The panel moves focus
  // deterministically on each of its internal transitions, but as a backstop we
  // also pull focus back if it ever escapes the dialog — so no present or future
  // unmount can leave focus on document.body (from where sequential Tab would
  // land on a control behind the locked gate). Armed only in recover mode, where
  // the panel's mount/unmount churn lives.
  useEffect(() => {
    if (mode !== 'recover') return
    const dialog = dialogRef.current
    /* v8 ignore next -- the dialog is always mounted while in recover mode. */
    if (!dialog) return
    const recapture = (event: FocusEvent) => {
      if (dialog.contains(event.target as Node)) return
      dialog
        .querySelector<HTMLElement>(
          'input,button,select,textarea,a[href],[tabindex]:not([tabindex="-1"])',
        )
        ?.focus()
    }
    document.addEventListener('focusin', recapture)
    return () => document.removeEventListener('focusin', recapture)
  }, [mode])

  // Focus trap: keep Tab / Shift+Tab cycling inside the dialog. Bound to the
  // dialog element via onKeyDown so `event.currentTarget` is always the dialog
  // (no ref-null guard), and the listener is torn down automatically with the
  // element (no manual add/removeEventListener). Escape is intentionally NOT
  // handled — the app is unusable while locked, so the gate cannot be dismissed.
  const handleTrapKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      if (event.key !== 'Tab') return
      const focusable = Array.from(
        event.currentTarget.querySelectorAll<HTMLElement>(
          'input,button,select,textarea,a[href],[tabindex]:not([tabindex="-1"])',
        ),
      ).filter((el) => !el.hasAttribute('disabled') && el.tabIndex >= 0)
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      if (event.shiftKey) {
        if (document.activeElement === first) {
          event.preventDefault()
          last.focus()
        }
      } else if (document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    },
    [],
  )

  const afterSuccessfulUnlock = useCallback(
    async (key: string) => {
      // Fire-and-forget reconcile to self-heal any drifted encryption state.
      // The transparency surface for a repair is the Audit "Automatic Repair"
      // run, NOT an in-gate notice: refreshAppData below unmounts the gate
      // before this resolves, so any inline notice would be unreachable.
      void backend.reconcileArchiveEncryption().catch(() => undefined)

      // Sync the keychain to the user's "remember" choice (only meaningful when
      // the keychain is available — the checkbox is hidden otherwise). Mirrors
      // the Settings toggle so the two surfaces never drift:
      //   checked   → store the key + persist rememberDatabaseKeyInKeyring=true
      //   unchecked → clear the key  + persist rememberDatabaseKeyInKeyring=false
      // Without the unchecked branch, a previously stored key would keep
      // auto-unlocking against an explicit opt-out (stale-key/flag desync).
      if (keyringAvailable) {
        try {
          if (rememberChecked) {
            await backend.keyringStoreDatabaseKey(key)
          } else {
            await backend.keyringClearDatabaseKey()
          }
          await saveConfig(
            {
              ...snapshot.config,
              rememberDatabaseKeyInKeyring: rememberChecked,
            },
            { quiet: true },
          )
        } catch {
          // Keychain sync is best-effort — unlock still succeeds.
        }
      }

      // Refresh shell snapshot (makes archiveStatus.unlocked = true → gate unmounts).
      await refreshAppData(false)

      // Retry pending backup AFTER the snapshot refresh so the archive is
      // confirmed unlocked before runBackup reads the session key.
      if (retryBackupOnUnlock) {
        onRetryBackup?.()
      }
    },
    [
      keyringAvailable,
      onRetryBackup,
      refreshAppData,
      rememberChecked,
      retryBackupOnUnlock,
      saveConfig,
      snapshot.config,
    ],
  )

  const handleUnlock = async () => {
    const trimmedKey = sessionKey.trim()
    if (!trimmedKey) {
      setError(t('currentDatabaseKeyRequired'))
      return
    }
    setBusy(true)
    setError(null)
    try {
      await backend.setSessionDatabaseKey(trimmedKey)
      // Confirm the archive is actually unlocked.
      const nextStatus = await backend.securityStatus()
      if (!nextStatus.unlocked) {
        await backend.clearSessionDatabaseKey().catch(() => undefined)
        throw new Error(t('archiveUnlockFailed'))
      }
      await afterSuccessfulUnlock(trimmedKey)
    } catch (nextError) {
      setError(describeError(nextError, 'unlock_archive'))
      setBusy(false)
    }
  }

  const handleUseSavedPassword = async () => {
    setBusy(true)
    setError(null)
    try {
      const key = await backend.keyringGetDatabaseKey()
      if (!key) {
        throw new Error(t('archiveUnlockFailed'))
      }
      await backend.setSessionDatabaseKey(key)
      const nextStatus = await backend.securityStatus()
      if (!nextStatus.unlocked) {
        await backend.clearSessionDatabaseKey().catch(() => undefined)
        throw new Error(t('archiveUnlockFailed'))
      }
      await afterSuccessfulUnlock(key)
    } catch (nextError) {
      setError(describeError(nextError, 'unlock_from_keyring'))
      setBusy(false)
    }
  }

  const rememberLabel = keyringAvailable
    ? t('rememberOnThisDeviceNamed', { backend: keyringBackend })
    : t('rememberOnThisDevice')

  const bodyText = retryBackupOnUnlock
    ? t('unlockGateBackupRetryBody')
    : t('unlockGateBody')

  return (
    <div
      ref={dialogRef}
      className="archive-unlock-gate"
      role="dialog"
      aria-modal="true"
      aria-labelledby="unlock-gate-title"
      aria-describedby="unlock-gate-desc"
      onKeyDown={handleTrapKeyDown}
      data-testid="archive-unlock-gate"
    >
      <div className="archive-unlock-gate__backdrop" aria-hidden="true" />
      <div className="archive-unlock-gate__panel">
        <div
          className="archive-unlock-gate__eyebrow mono-kicker"
          aria-hidden="true"
        >
          PathKeep
        </div>

        {mode === 'recover' ? (
          <>
            <h1
              ref={recoverHeadingRef}
              tabIndex={-1}
              id="unlock-gate-title"
              className="archive-unlock-gate__title"
            >
              {t('recoverTitle')}
            </h1>

            <p id="unlock-gate-desc" className="archive-unlock-gate__body">
              {t('recoverBody')}
            </p>

            <SnapshotRecoveryPanel
              runFullArchiveRestore={runFullArchiveRestore}
              initialKey={sessionKey}
              onRestored={() => setMode('unlock')}
            />

            <div className="archive-unlock-gate__actions">
              <button
                type="button"
                className="btn-ghost archive-unlock-gate__recover-link"
                onClick={() => setMode('unlock')}
                aria-label={t('backToUnlockAria')}
              >
                {t('backToUnlock')}
              </button>
            </div>
          </>
        ) : (
          <>
            <h1 id="unlock-gate-title" className="archive-unlock-gate__title">
              {t('unlockGateTitle')}
            </h1>

            <p id="unlock-gate-desc" className="archive-unlock-gate__body">
              {bodyText}
            </p>

            {/* "Use saved password" shortcut — only when keychain has one */}
            {keyringStored && !busy ? (
              <button
                type="button"
                className="archive-unlock-gate__keyring-btn btn-secondary"
                onClick={() => void handleUseSavedPassword()}
                aria-label={t('useKeyring')}
              >
                {t('useKeyring')}
              </button>
            ) : null}

            <div className="archive-unlock-gate__divider" aria-hidden="true" />

            <label className="field-stack archive-unlock-gate__field">
              <span className="mono-kicker">{t('currentDatabaseKey')}</span>
              <input
                ref={passwordRef}
                type="password"
                autoComplete="current-password"
                value={sessionKey}
                onChange={(e) => {
                  setSessionKey(e.target.value)
                  setError(null)
                }}
                onKeyDown={(e) => {
                  // The input is disabled while busy, so Enter cannot double-fire.
                  if (e.key === 'Enter') {
                    void handleUnlock()
                  }
                }}
                placeholder={t('currentDatabaseKeyPlaceholder')}
                aria-label={t('currentDatabaseKey')}
                aria-invalid={error !== null}
                aria-describedby={error ? 'unlock-gate-error' : undefined}
                disabled={busy}
              />
            </label>

            {error ? (
              <p id="unlock-gate-error" className="inline-error" role="alert">
                {error}
              </p>
            ) : null}

            {keyringAvailable ? (
              <label className="archive-unlock-gate__remember form-checkbox-row">
                <input
                  type="checkbox"
                  checked={rememberChecked}
                  onChange={(e) => setRememberChecked(e.target.checked)}
                  disabled={busy}
                  aria-label={rememberLabel}
                />
                <span>{rememberLabel}</span>
              </label>
            ) : null}

            <div className="archive-unlock-gate__actions">
              <button
                type="button"
                className="btn-primary archive-unlock-gate__submit"
                onClick={() => void handleUnlock()}
                disabled={busy}
                aria-busy={busy}
                // Stable accessible name: the visible label collapses to "…" while
                // busy, so without this the button would briefly announce as "…".
                aria-label={t('unlockArchive')}
              >
                {busy ? '…' : t('unlockArchive')}
              </button>
            </div>

            {/* Calm escape hatch — always reachable so a correct-password-that-keeps-failing
                user can still restore a snapshot instead of being dead-ended. */}
            <button
              type="button"
              className="btn-ghost archive-unlock-gate__recover-link"
              onClick={() => setMode('recover')}
              aria-label={t('cantUnlockRecoverAria')}
            >
              {t('cantUnlockRecover')}
            </button>
          </>
        )}
      </div>
    </div>
  )
}
