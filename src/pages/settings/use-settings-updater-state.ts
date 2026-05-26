/**
 * @file use-settings-updater-state.ts
 * @description Owns Settings/Maintenance updater review state without product usage reporting.
 * @module pages/settings
 *
 * ## 職責
 * - 管理 updater check / download / relaunch review state。
 * - 把 updater lifecycle 狀態集中在單一 route-owned hook。
 * - 保持 Maintenance 的手動更新流程不依賴 Settings route shell。
 *
 * ## 不負責
 * - 不渲染 updater section。
 * - 不管理 support, AI, derived runtime, 或 remote backup。
 * - 不收集或傳送產品使用資料。
 *
 * ## 依賴關係
 * - 依賴 shell snapshot/build info 與 update helper。
 *
 * ## 性能備注
 * - 只有使用者操作 updater 時才觸發 mutation，不做背景輪詢。
 */

import { useState } from 'react'
import { backend } from '../../lib/backend-client'
import { describeError } from '../../lib/errors'
import { useI18n } from '../../lib/i18n'
import {
  RELEASES_PAGE_URL,
  checkForAppUpdate,
  downloadAndInstallAppUpdate,
  initialUpdateInstallState,
  relaunchAfterUpdate,
  type PendingAppUpdate,
} from '../../lib/update'
import type {
  AppBuildInfo,
  AppSnapshot,
  UpdateAvailability,
  UpdateInstallState,
} from '../../lib/types'

interface UseSettingsUpdaterStateArgs {
  buildInfo: AppBuildInfo | null
  snapshot: AppSnapshot | null
}

/**
 * Keeps updater state in a focused hook.
 */
export function useSettingsUpdaterState({
  buildInfo,
  snapshot,
}: UseSettingsUpdaterStateArgs) {
  const { t } = useI18n()
  const [updateAvailability, setUpdateAvailability] =
    useState<UpdateAvailability | null>(null)
  const [pendingUpdate, setPendingUpdate] = useState<PendingAppUpdate | null>(
    null,
  )
  const [updateInstallState, setUpdateInstallState] =
    useState<UpdateInstallState>(initialUpdateInstallState)

  async function handleCheckForUpdates() {
    if (!snapshot) {
      return
    }

    setUpdateInstallState({
      phase: 'checking',
      downloadedBytes: null,
      contentLength: null,
      message: t('settings.updateChecking'),
    })
    try {
      const result = await checkForAppUpdate(buildInfo?.version)
      setUpdateAvailability(result.availability)
      setPendingUpdate(result.pendingUpdate)
      if (!result.availability.supported) {
        setUpdateInstallState({
          phase: 'unsupported',
          downloadedBytes: null,
          contentLength: null,
          message:
            result.availability.error ?? t('settings.updateUnsupportedBody'),
        })
      } else if (result.availability.error) {
        setUpdateInstallState({
          phase: 'error',
          downloadedBytes: null,
          contentLength: null,
          message: result.availability.error,
        })
      } else if (result.availability.available) {
        setUpdateInstallState({
          phase: 'available',
          downloadedBytes: null,
          contentLength: null,
          message: t('settings.updateAvailableBody', {
            version: result.availability.version ?? t('common.notAvailable'),
          }),
        })
      } else {
        setUpdateInstallState({
          phase: 'uptodate',
          downloadedBytes: null,
          contentLength: null,
          message: t('settings.updateUpToDateBody'),
        })
      }
    } catch (error) {
      setUpdateAvailability(null)
      setPendingUpdate(null)
      setUpdateInstallState({
        phase: 'error',
        downloadedBytes: null,
        contentLength: null,
        message: describeError(error, 'check_for_app_update'),
      })
    }
  }

  async function handleDownloadAndInstallUpdate() {
    if (!pendingUpdate) {
      return
    }

    await downloadAndInstallAppUpdate(pendingUpdate, setUpdateInstallState)
  }

  async function handleRelaunchForUpdate() {
    await relaunchAfterUpdate()
  }

  async function handleOpenReleasePage() {
    await backend.openExternalUrl(
      updateAvailability?.downloadUrl ?? RELEASES_PAGE_URL,
    )
  }

  return {
    updater: {
      buildInfo,
      pendingUpdate,
      updateAvailability,
      updateInstallState,
      onCheckForUpdates: handleCheckForUpdates,
      onDownloadAndInstallUpdate: handleDownloadAndInstallUpdate,
      onOpenReleasePage: handleOpenReleasePage,
      onRelaunchForUpdate: handleRelaunchForUpdate,
    },
  }
}
