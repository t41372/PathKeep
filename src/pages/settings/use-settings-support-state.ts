/**
 * @file use-settings-support-state.ts
 * @description Owns Settings support, language, retention, App Lock, and profile-selection state without pulling unrelated intelligence workflows into the same hook.
 * @module pages/settings
 *
 * ## 職責
 * - 載入 Settings 依賴的 support snapshot（schedule/security）。
 * - 管理 general section 的 copy/open/language actions。
 * - 管理 retention、App Lock、與 browser profile selection 的 draft state 和 handlers。
 *
 * ## 不負責
 * - 不管理 updater、AI、derived runtime、或 remote backup。
 * - 不渲染任何 section UI。
 * - 不建立新的 shell-level polling source。
 *
 * ## 依賴關係
 * - 依賴 shell snapshot/config actions、`backend-client`，以及 Settings helpers。
 *
 * ## 性能備注
 * - 只在 `refreshKey` 或 snapshot 變化時重新同步 support/retention state，避免重複 background work。
 */

import { useEffect, useMemo, useState } from 'react'
import {
  copyReviewValue,
  type ReviewCopyFeedback,
} from '../../components/review'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { normalizeExplorerBackgroundPrefetchPages } from '../../lib/explorer-preferences'
import { useI18n } from '../../lib/i18n'
import type {
  AppConfig,
  AppLockConfig,
  AppLockStatus,
  AppSnapshot,
  RetentionPreview,
  RetentionPruneResult,
} from '../../lib/types'
import { buildRetentionSelection, type SupportState } from './helpers'

/**
 * Structural equality for two App Lock configs that have BOTH already been run
 * through `normalizeAppLock` (so `recoveryHint` is a canonical `string | null`).
 * Used to short-circuit a redundant auto-save (and a misleading "Saved" chip)
 * when an App Lock edit normalizes back to the persisted value — e.g. re-selecting
 * the same idle timeout, or toggling biometric on a machine without biometric
 * hardware.
 */
function appLockConfigEquals(a: AppLockConfig, b: AppLockConfig): boolean {
  return (
    a.enabled === b.enabled &&
    a.idleTimeoutMinutes === b.idleTimeoutMinutes &&
    a.biometricEnabled === b.biometricEnabled &&
    a.passcodeEnabled === b.passcodeEnabled &&
    a.passcodeConfigured === b.passcodeConfigured &&
    a.recoveryHint === b.recoveryHint
  )
}

interface UseSettingsSupportStateArgs {
  appLockStatus: AppLockStatus | null
  clearAppLockPasscode: () => Promise<AppLockStatus>
  enableRetentionPreview?: boolean
  lockAppSession: (reason?: string | null) => Promise<AppLockStatus>
  refreshAppData: () => Promise<void>
  refreshKey: number
  saveConfig: (config: AppConfig) => Promise<AppSnapshot>
  setAppLockPasscode: (request: {
    passcode: string
    recoveryHint?: string | null
  }) => Promise<AppLockStatus>
  setLanguagePreference: (language: AppConfig['preferredLanguage']) => void
  snapshot: AppSnapshot | null
}

/**
 * Keeps the non-AI, non-remote Settings support surfaces under one small hook.
 *
 * These sections all depend on the same shell snapshot and support-state load,
 * so grouping them here avoids spreading small but stateful workflows across
 * the route shell.
 */
export function useSettingsSupportState({
  appLockStatus,
  clearAppLockPasscode,
  enableRetentionPreview = true,
  lockAppSession,
  refreshAppData,
  refreshKey,
  saveConfig,
  setAppLockPasscode,
  setLanguagePreference,
  snapshot,
}: UseSettingsSupportStateArgs) {
  const { t } = useI18n()
  const [saving, setSaving] = useState(false)
  const [supportState, setSupportState] = useState<SupportState>({
    scheduleStatus: null,
    securityStatus: null,
  })
  const [supportStateLoaded, setSupportStateLoaded] = useState(false)
  const [supportCopyFeedback, setSupportCopyFeedback] =
    useState<ReviewCopyFeedback | null>(null)
  const [retentionPreview, setRetentionPreview] =
    useState<RetentionPreview | null>(null)
  const [retentionSelection, setRetentionSelection] = useState<
    Record<string, boolean>
  >({})
  const [retentionResult, setRetentionResult] =
    useState<RetentionPruneResult | null>(null)
  const [retentionAction, setRetentionAction] = useState<string | null>(null)
  const [retentionError, setRetentionError] = useState<string | null>(null)
  const [appLockDraft, setAppLockDraft] = useState<AppLockConfig | null>(
    snapshot?.config.appLock ?? null,
  )
  const [appLockPasscode, setAppLockPasscodeDraft] = useState('')
  const [appLockRecoveryHint, setAppLockRecoveryHint] = useState('')
  const [appLockAction, setAppLockAction] = useState<string | null>(null)
  const selectedIds = useMemo(
    () => new Set(snapshot?.config.selectedProfileIds ?? []),
    [snapshot?.config.selectedProfileIds],
  )
  const currentAppLockSettings =
    appLockDraft ?? snapshot?.config.appLock ?? null
  const selectedRetentionBuckets = retentionPreview
    ? retentionPreview.buckets.filter((bucket) => retentionSelection[bucket.id])
    : []
  const selectedRetentionBytes = selectedRetentionBuckets.reduce(
    (total, bucket) => total + bucket.bytes,
    0,
  )
  const retentionNeedsUnlock =
    enableRetentionPreview &&
    supportState.securityStatus?.encrypted === true &&
    supportState.securityStatus.unlocked === false
  const appLockCanEnable =
    Boolean(currentAppLockSettings?.passcodeConfigured) ||
    Boolean(appLockStatus?.passcodeConfigured)
  const biometricUsesTouchId =
    appLockStatus?.biometricState === 'touch-id-available' ||
    appLockStatus?.biometricState === 'touch-id-unavailable'

  useEffect(() => {
    let cancelled = false
    setSupportStateLoaded(false)

    const loadSupportState = async () => {
      try {
        const [scheduleStatus, securityStatus] = await Promise.all([
          backend.scheduleStatus(),
          backend.securityStatus(),
        ])

        if (!cancelled) {
          setSupportState({ scheduleStatus, securityStatus })
          setSupportStateLoaded(true)
        }
      } catch {
        if (!cancelled) {
          setSupportState({ scheduleStatus: null, securityStatus: null })
          setSupportStateLoaded(true)
        }
      }
    }

    void loadSupportState()
    return () => {
      cancelled = true
    }
  }, [snapshot?.config.preferredLanguage])

  useEffect(() => {
    if (!snapshot) {
      return
    }

    setAppLockDraft(snapshot.config.appLock)
    setAppLockRecoveryHint(snapshot.config.appLock.recoveryHint ?? '')
  }, [snapshot])

  useEffect(() => {
    let cancelled = false

    const loadRetentionPreview = async () => {
      if (!enableRetentionPreview) {
        setRetentionPreview(null)
        setRetentionSelection({})
        setRetentionResult(null)
        setRetentionAction(null)
        setRetentionError(null)
        return
      }

      try {
        const preview = await backend.previewRetentionPrune()
        if (!cancelled) {
          setRetentionPreview(preview)
          setRetentionSelection((current) =>
            buildRetentionSelection(preview, current),
          )
          setRetentionError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setRetentionPreview(null)
          setRetentionError(describeError(error, 'preview_retention_prune'))
        }
      }
    }

    void loadRetentionPreview()
    return () => {
      cancelled = true
    }
  }, [enableRetentionPreview, refreshKey, snapshot?.config.initialized, t])

  async function handleSupportPathCopy(key: string, value: string) {
    await copyReviewValue(value, {
      key,
      onFeedback: setSupportCopyFeedback,
    })
  }

  function handleSupportPathOpen(path: string) {
    void backend.openPathInFileManager(path)
  }

  async function handleLanguageChange(nextLanguage: string) {
    if (
      !snapshot ||
      (nextLanguage !== 'system' &&
        nextLanguage !== 'en' &&
        nextLanguage !== 'zh-CN' &&
        nextLanguage !== 'zh-TW')
    ) {
      return
    }

    setSaving(true)
    try {
      setLanguagePreference(nextLanguage)
      await saveConfig({
        ...snapshot.config,
        preferredLanguage: nextLanguage,
      })
    } finally {
      setSaving(false)
    }
  }

  async function handleExplorerBackgroundPrefetchPagesChange(
    nextPages: number,
  ) {
    if (!snapshot) {
      return
    }

    const normalizedPages = normalizeExplorerBackgroundPrefetchPages(nextPages)
    if (normalizedPages === snapshot.config.explorerBackgroundPrefetchPages) {
      return
    }

    setSaving(true)
    try {
      await saveConfig({
        ...snapshot.config,
        explorerBackgroundPrefetchPages: normalizedPages,
      })
    } finally {
      setSaving(false)
    }
  }

  async function refreshRetentionPreview() {
    if (!enableRetentionPreview) {
      return
    }

    try {
      const preview = await backend.previewRetentionPrune()
      setRetentionPreview(preview)
      setRetentionSelection((current) =>
        buildRetentionSelection(preview, current),
      )
      setRetentionError(null)
    } catch (error) {
      setRetentionPreview(null)
      setRetentionError(describeError(error, 'preview_retention_prune'))
    }
  }

  function handleRetentionBucketSelection(bucketId: string, checked: boolean) {
    setRetentionSelection((current) => ({
      ...current,
      [bucketId]: checked,
    }))
  }

  async function handleRetentionPrune() {
    if (!enableRetentionPreview) {
      return
    }

    if (selectedRetentionBuckets.length === 0) {
      setRetentionError(t('settings.retentionNothingSelected'))
      return
    }

    setRetentionAction(t('settings.retentionExecute'))
    setRetentionError(null)
    try {
      const result = await backend.runRetentionPrune({
        bucketIds: selectedRetentionBuckets.map((bucket) => bucket.id),
      })
      setRetentionResult(result)
      await refreshAppData()
      await refreshRetentionPreview()
    } catch (error) {
      setRetentionError(describeError(error, 'run_retention_prune'))
    } finally {
      setRetentionAction(null)
    }
  }

  async function toggleProfile(profileId: string) {
    if (saving || !snapshot) {
      return
    }

    setSaving(true)
    try {
      const next = selectedIds.has(profileId)
        ? snapshot.config.selectedProfileIds.filter((id) => id !== profileId)
        : [...snapshot.config.selectedProfileIds, profileId]
      await saveConfig({ ...snapshot.config, selectedProfileIds: next })
    } finally {
      setSaving(false)
    }
  }

  // Normalize an App Lock draft the same way the backend persists it: biometric is
  // gated on real availability, passcode flags are forced, and the recovery hint is
  // trimmed (empty → null). Crucially this normalizes the hint that is ALREADY on
  // `next` — NOT the live editing buffer — so toggling a non-hint field never
  // flushes an in-progress recovery-hint edit to disk before its own blur. The
  // hint's own commit-on-blur handler is the one place that copies the buffer onto
  // `next`, so the buffer only persists when the user actually leaves the field.
  function normalizeAppLock(next: AppLockConfig): AppLockConfig {
    return {
      ...next,
      biometricEnabled:
        next.biometricEnabled && Boolean(appLockStatus?.biometricAvailable),
      passcodeEnabled: true,
      passcodeConfigured:
        appLockStatus?.passcodeConfigured ?? next.passcodeConfigured,
      recoveryHint: (next.recoveryHint ?? '').trim() || null,
    }
  }

  // Persist the App Lock config immediately (the page is all-auto-save). Returns
  // true only when the write actually landed, so the section flashes the quiet
  // "Saved" chip on success and stays silent on a no-op or failure. No-ops (return
  // false, no write, no chip) when the normalized config already matches the
  // persisted draft — so re-selecting the same idle timeout, or a biometric toggle
  // that normalizes back to the saved value, never fires a redundant write or a
  // misleading "Saved", mirroring the AI path's guard.
  async function persistAppLock(next: AppLockConfig): Promise<boolean> {
    if (!snapshot) {
      return false
    }

    // `currentAppLockSettings` is `appLockDraft ?? snapshot.config.appLock`, so with
    // a snapshot present (checked above) it is always the committed config — every
    // caller also guards on it before reaching here. Compare the normalized next
    // config against it to short-circuit a redundant write (and a misleading
    // "Saved"). The non-null assertion just expresses what the snapshot check
    // guarantees without an unreachable runtime branch.
    const normalized = normalizeAppLock(next)
    const persisted = normalizeAppLock(currentAppLockSettings!)
    if (appLockConfigEquals(normalized, persisted)) {
      return false
    }

    const nextSnapshot = await saveConfig({
      ...snapshot.config,
      appLock: normalized,
    })
    // Update the committed-config mirror so a back-to-back auto-save (or the no-op
    // guard above) computes from the freshest persisted value. Intentionally does
    // NOT touch `appLockRecoveryHint`: toggling a non-hint field must leave the
    // in-progress hint editing buffer alone — only the hint's own commit-on-blur
    // reconciles the buffer (below), and an external snapshot change reconciles it
    // via the sync effect.
    setAppLockDraft(nextSnapshot.config.appLock)
    return true
  }

  async function handleAppLockEnabledChange(
    enabled: boolean,
  ): Promise<boolean> {
    if (!currentAppLockSettings) {
      return false
    }
    return persistAppLock({ ...currentAppLockSettings, enabled })
  }

  async function handleAppLockIdleTimeoutChange(
    idleTimeoutMinutes: number,
  ): Promise<boolean> {
    if (!currentAppLockSettings) {
      return false
    }
    return persistAppLock({ ...currentAppLockSettings, idleTimeoutMinutes })
  }

  async function handleAppLockBiometricChange(
    biometricEnabled: boolean,
  ): Promise<boolean> {
    if (!currentAppLockSettings) {
      return false
    }
    return persistAppLock({ ...currentAppLockSettings, biometricEnabled })
  }

  // Recovery hint is free text, so it edits ONLY the editing buffer on every
  // keystroke (off the persistence path) and writes on blur. The committed-config
  // mirror (`currentAppLockSettings.recoveryHint`) keeps the last-persisted value —
  // unchanged by non-hint toggles — so commit-on-blur can tell a real edit from a
  // no-op blur, and toggling another field never flushes the in-progress buffer.
  function handleAppLockRecoveryHintChange(recoveryHint: string) {
    setAppLockRecoveryHint(recoveryHint)
  }

  async function handleAppLockRecoveryHintCommit(): Promise<boolean> {
    if (!currentAppLockSettings) {
      return false
    }
    // No-op when the trimmed hint already matches what's persisted (the draft
    // mirrors the backend after every write), so a blur without an edit never
    // fires a redundant write or a misleading "Saved".
    const nextHint = appLockRecoveryHint.trim() || null
    if (nextHint === (currentAppLockSettings.recoveryHint ?? null)) {
      return false
    }
    const saved = await persistAppLock({
      ...currentAppLockSettings,
      recoveryHint: appLockRecoveryHint,
    })
    // This is the ONE place the editing buffer is reconciled to the persisted
    // (trimmed) hint, so a typed-then-blurred value with surrounding whitespace
    // settles to exactly what was stored. `persistAppLock` deliberately leaves the
    // buffer untouched so unrelated field toggles never wipe an in-progress edit.
    setAppLockRecoveryHint(nextHint ?? '')
    return saved
  }

  async function handleSetAppLockPasscode() {
    setAppLockAction(t('settings.appLockSavingPasscode'))
    try {
      await setAppLockPasscode({
        passcode: appLockPasscode,
        recoveryHint: appLockRecoveryHint.trim() || null,
      })
      setAppLockPasscodeDraft('')
    } finally {
      setAppLockAction(null)
    }
  }

  async function handleClearAppLockPasscode() {
    setAppLockAction(t('settings.appLockClearingPasscode'))
    try {
      await clearAppLockPasscode()
      setAppLockPasscodeDraft('')
      setAppLockRecoveryHint('')
    } finally {
      setAppLockAction(null)
    }
  }

  async function handleLockNow() {
    setAppLockAction(t('settings.appLockLockingNow'))
    try {
      await lockAppSession('manual')
    } finally {
      setAppLockAction(null)
    }
  }

  return {
    supportState,
    supportStateLoaded,
    general: {
      explorerBackgroundPrefetchPages:
        snapshot?.config.explorerBackgroundPrefetchPages ??
        normalizeExplorerBackgroundPrefetchPages(undefined),
      saving,
      supportCopyFeedback,
      onCopyPath: handleSupportPathCopy,
      onExplorerBackgroundPrefetchPagesChange:
        handleExplorerBackgroundPrefetchPagesChange,
      onLanguageChange: handleLanguageChange,
      onOpenPath: handleSupportPathOpen,
    },
    retention: {
      action: retentionAction,
      error: retentionError,
      needsUnlock: retentionNeedsUnlock,
      preview: retentionPreview,
      result: retentionResult,
      selectedBytes: selectedRetentionBytes,
      selection: retentionSelection,
      onBucketSelectionChange: handleRetentionBucketSelection,
      onPrune: handleRetentionPrune,
      onRefresh: refreshRetentionPreview,
    },
    appLock: {
      action: appLockAction,
      canEnable: appLockCanEnable,
      copyFeedback: supportCopyFeedback,
      currentSettings: currentAppLockSettings,
      passcode: appLockPasscode,
      recoveryHint: appLockRecoveryHint,
      status: appLockStatus,
      usesTouchId: biometricUsesTouchId,
      onBiometricChange: handleAppLockBiometricChange,
      onClearPasscode: handleClearAppLockPasscode,
      onCopyPath: handleSupportPathCopy,
      onEnabledChange: handleAppLockEnabledChange,
      onIdleTimeoutChange: handleAppLockIdleTimeoutChange,
      onLockNow: handleLockNow,
      onOpenPath: handleSupportPathOpen,
      onPasscodeChange: setAppLockPasscodeDraft,
      onRecoveryHintChange: handleAppLockRecoveryHintChange,
      onRecoveryHintCommit: handleAppLockRecoveryHintCommit,
      onSetPasscode: handleSetAppLockPasscode,
    },
    profiles: {
      profiles: snapshot?.browserProfiles ?? [],
      saving,
      selectedIds,
      onToggleProfile: toggleProfile,
    },
  }
}
