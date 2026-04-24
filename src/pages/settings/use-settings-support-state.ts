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
 * - 不管理 analytics、updater、AI、derived runtime、或 remote backup。
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
  const appLockConfigDirty =
    currentAppLockSettings !== null &&
    JSON.stringify(currentAppLockSettings) !==
      JSON.stringify(snapshot?.config.appLock ?? null)
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
          setRetentionError(
            error instanceof Error ? error.message : t('common.notAvailable'),
          )
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
      setRetentionError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
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
      setRetentionError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
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

  function handleAppLockEnabledChange(enabled: boolean) {
    setAppLockDraft((current) => (current ? { ...current, enabled } : current))
  }

  function handleAppLockIdleTimeoutChange(idleTimeoutMinutes: number) {
    setAppLockDraft((current) =>
      current ? { ...current, idleTimeoutMinutes } : current,
    )
  }

  function handleAppLockBiometricChange(biometricEnabled: boolean) {
    setAppLockDraft((current) =>
      current ? { ...current, biometricEnabled } : current,
    )
  }

  function handleAppLockRecoveryHintChange(recoveryHint: string) {
    setAppLockRecoveryHint(recoveryHint)
    setAppLockDraft((current) =>
      current ? { ...current, recoveryHint } : current,
    )
  }

  async function handleSaveAppLockConfig() {
    if (!snapshot || !appLockDraft) {
      return
    }

    setAppLockAction(t('settings.appLockSaving'))
    try {
      const nextSnapshot = await saveConfig({
        ...snapshot.config,
        appLock: {
          ...appLockDraft,
          biometricEnabled:
            appLockDraft.biometricEnabled &&
            Boolean(appLockStatus?.biometricAvailable),
          passcodeEnabled: true,
          passcodeConfigured:
            appLockStatus?.passcodeConfigured ??
            appLockDraft.passcodeConfigured,
          recoveryHint: appLockRecoveryHint.trim() || null,
        },
      })
      setAppLockDraft(nextSnapshot.config.appLock)
      setAppLockRecoveryHint(nextSnapshot.config.appLock.recoveryHint ?? '')
    } finally {
      setAppLockAction(null)
    }
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
      configDirty: appLockConfigDirty,
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
      onSaveConfig: handleSaveAppLockConfig,
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
