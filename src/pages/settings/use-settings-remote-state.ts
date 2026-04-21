/**
 * @file use-settings-remote-state.ts
 * @description Owns the Settings remote-backup PME draft and credential workflows.
 * @module pages/settings
 *
 * ## 職責
 * - 管理 remote backup config draft、credential inputs、preview/result/verify state。
 * - 集中 save / preview / execute / verify handlers。
 * - 保持 remote backup PME 的 single source of truth。
 *
 * ## 不負責
 * - 不渲染 remote-backup section UI。
 * - 不管理 unrelated Settings workflows。
 * - 不改變 remote backup backend contract。
 *
 * ## 依賴關係
 * - 依賴 shell snapshot/config actions、backend client 與 i18n。
 *
 * ## 性能備注
 * - 只有使用者操作 remote backup surface 時才觸發 preview/execute/verify，不做 background polling。
 */

import { useEffect, useState } from 'react'
import { backend } from '../../lib/backend-client'
import { useI18n } from '../../lib/i18n'
import type {
  AppConfig,
  AppSnapshot,
  RemoteBackupConfig,
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
} from '../../lib/types'

interface UseSettingsRemoteStateArgs {
  refreshAppData: () => Promise<void>
  saveConfig: (config: AppConfig) => Promise<AppSnapshot>
  snapshot: AppSnapshot | null
}

/**
 * Keeps the remote-backup PME workflow under one focused hook.
 */
export function useSettingsRemoteState({
  refreshAppData,
  saveConfig,
  snapshot,
}: UseSettingsRemoteStateArgs) {
  const { t } = useI18n()
  const [remoteTab, setRemoteTab] = useState<
    'preview' | 'manual' | 'execute' | 'verify'
  >('preview')
  const [remoteDraft, setRemoteDraft] = useState<RemoteBackupConfig | null>(
    snapshot?.config.remoteBackup ?? null,
  )
  const [accessKeyId, setAccessKeyId] = useState('')
  const [secretAccessKey, setSecretAccessKey] = useState('')
  const [remotePreview, setRemotePreview] =
    useState<RemoteBackupPreview | null>(null)
  const [remoteResult, setRemoteResult] = useState<RemoteBackupResult | null>(
    null,
  )
  const [remoteVerification, setRemoteVerification] =
    useState<RemoteBackupVerification | null>(null)
  const [remoteAction, setRemoteAction] = useState<string | null>(null)
  const currentRemoteDraft =
    remoteDraft ?? snapshot?.config.remoteBackup ?? null
  const remoteConfigured = Boolean(
    currentRemoteDraft?.bucket.trim() && currentRemoteDraft.region.trim(),
  )
  const latestRemoteBundlePath = remoteResult?.bundlePath ?? null

  useEffect(() => {
    if (!snapshot) {
      return
    }

    setRemoteDraft(snapshot.config.remoteBackup)
  }, [snapshot])

  async function persistRemoteConfig() {
    if (!snapshot || !remoteDraft) {
      return
    }

    const nextSnapshot = await saveConfig({
      ...snapshot.config,
      remoteBackup: {
        ...remoteDraft,
        credentialsSaved: snapshot.config.remoteBackup.credentialsSaved,
      },
    })
    setRemoteDraft(nextSnapshot.config.remoteBackup)
  }

  function handleRemoteDraftChange(patch: Partial<RemoteBackupConfig>) {
    setRemoteDraft((current) => (current ? { ...current, ...patch } : current))
  }

  async function handleSaveRemoteConfig() {
    if (!remoteDraft) {
      return
    }

    setRemoteAction(t('settings.savingRemoteSettings'))
    try {
      await persistRemoteConfig()
      setRemoteVerification(null)
    } finally {
      setRemoteAction(null)
    }
  }

  async function handleStoreCredentials() {
    if (!accessKeyId.trim() || !secretAccessKey.trim()) {
      return
    }

    setRemoteAction(t('settings.storingRemoteCredentials'))
    try {
      await backend.storeS3Credentials({
        accessKeyId: accessKeyId.trim(),
        secretAccessKey: secretAccessKey.trim(),
      })
      setAccessKeyId('')
      setSecretAccessKey('')
      await refreshAppData()
    } finally {
      setRemoteAction(null)
    }
  }

  async function handleClearCredentials() {
    setRemoteAction(t('settings.clearingRemoteCredentials'))
    try {
      await backend.clearS3Credentials()
      await refreshAppData()
    } finally {
      setRemoteAction(null)
    }
  }

  async function handlePreviewRemote() {
    setRemoteAction(t('settings.previewingRemoteBackup'))
    try {
      await persistRemoteConfig()
      const preview = await backend.previewRemoteBackup()
      setRemotePreview(preview)
      setRemoteTab('preview')
    } finally {
      setRemoteAction(null)
    }
  }

  async function handleExecuteRemote() {
    setRemoteAction(t('settings.executingRemoteBackup'))
    try {
      await persistRemoteConfig()
      const result = await backend.runRemoteBackup()
      setRemoteResult(result)
      setRemoteTab('execute')
      await refreshAppData()
    } finally {
      setRemoteAction(null)
    }
  }

  async function handleVerifyRemote() {
    if (!latestRemoteBundlePath) {
      return
    }

    setRemoteAction(t('settings.verifyingRemoteBackup'))
    try {
      const verification = await backend.verifyRemoteBackup(
        latestRemoteBundlePath,
      )
      setRemoteVerification(verification)
      setRemoteTab('verify')
    } finally {
      setRemoteAction(null)
    }
  }

  return {
    remote: {
      accessKeyId,
      action: remoteAction,
      configured: remoteConfigured,
      currentDraft: currentRemoteDraft,
      latestRemoteBundlePath,
      preview: remotePreview,
      result: remoteResult,
      secretAccessKey,
      tab: remoteTab,
      verification: remoteVerification,
      onAccessKeyIdChange: setAccessKeyId,
      onClearCredentials: handleClearCredentials,
      onDraftChange: handleRemoteDraftChange,
      onExecute: handleExecuteRemote,
      onPreview: handlePreviewRemote,
      onSaveConfig: handleSaveRemoteConfig,
      onSecretAccessKeyChange: setSecretAccessKey,
      onSetTab: setRemoteTab,
      onStoreCredentials: handleStoreCredentials,
      onVerify: handleVerifyRemote,
    },
  }
}
