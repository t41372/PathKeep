/**
 * @file use-settings-analytics-updater-state.ts
 * @description Owns Settings analytics consent and updater review state so those control-tower flows do not live inline inside the route shell.
 * @module pages/settings
 *
 * ## 職責
 * - 管理 analytics consent draft/save flow。
 * - 管理 updater check / download / relaunch review state。
 * - 保持 analytics event 與 updater lifecycle 追蹤集中在單一 owner。
 *
 * ## 不負責
 * - 不渲染 analytics 或 updater sections。
 * - 不管理 support, AI, derived runtime, 或 remote backup。
 * - 不決定 updater backend contract。
 *
 * ## 依賴關係
 * - 依賴 shell snapshot/config、analytics helper、與 update helper。
 *
 * ## 性能備注
 * - 只有使用者操作 updater 或 analytics consent 時才觸發 mutation，不做背景輪詢。
 */

import { useEffect, useState } from 'react'
import {
  CONFIGURED_ANALYTICS_ENDPOINT,
  trackAnalyticsEvent,
} from '../../lib/analytics'
import { backend } from '../../lib/backend-client'
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
  AppConfig,
  AppSnapshot,
  UpdateAvailability,
  UpdateInstallState,
} from '../../lib/types'

interface UseSettingsAnalyticsUpdaterStateArgs {
  buildInfo: AppBuildInfo | null
  saveConfig: (config: AppConfig) => Promise<AppSnapshot>
  snapshot: AppSnapshot | null
}

/**
 * Keeps analytics consent and updater state in a focused hook.
 */
export function useSettingsAnalyticsUpdaterState({
  buildInfo,
  saveConfig,
  snapshot,
}: UseSettingsAnalyticsUpdaterStateArgs) {
  const { t } = useI18n()
  const [analyticsDraft, setAnalyticsDraft] = useState(
    snapshot?.config.analytics ?? null,
  )
  const [analyticsAction, setAnalyticsAction] = useState<string | null>(null)
  const [updateAvailability, setUpdateAvailability] =
    useState<UpdateAvailability | null>(null)
  const [pendingUpdate, setPendingUpdate] = useState<PendingAppUpdate | null>(
    null,
  )
  const [updateInstallState, setUpdateInstallState] =
    useState<UpdateInstallState>(initialUpdateInstallState)
  const currentAnalyticsSettings =
    analyticsDraft ?? snapshot?.config.analytics ?? null
  const analyticsConfigDirty =
    currentAnalyticsSettings !== null &&
    JSON.stringify(currentAnalyticsSettings) !==
      JSON.stringify(snapshot?.config.analytics ?? null)

  useEffect(() => {
    if (!snapshot) {
      return
    }

    setAnalyticsDraft(snapshot.config.analytics)
  }, [snapshot])

  function handleAnalyticsEnabledChange(enabled: boolean) {
    setAnalyticsDraft((current) => ({
      enabled,
      consentGrantedAt: current?.consentGrantedAt ?? null,
    }))
  }

  async function handleSaveAnalyticsConsent() {
    if (!snapshot || !currentAnalyticsSettings) {
      return
    }

    setAnalyticsAction(t('settings.analyticsSaving'))
    try {
      const nextEnabled = currentAnalyticsSettings.enabled
      const nextSnapshot = await saveConfig({
        ...snapshot.config,
        analytics: {
          enabled: nextEnabled,
          consentGrantedAt: nextEnabled ? new Date().toISOString() : null,
        },
      })
      setAnalyticsDraft(nextSnapshot.config.analytics)
      if (nextEnabled) {
        await trackAnalyticsEvent(
          nextSnapshot.config.analytics,
          {
            type: 'cta-click',
            screen: 'settings',
            action: 'save-consent',
            feature: 'analytics',
          },
          buildInfo,
        )
      }
    } finally {
      setAnalyticsAction(null)
    }
  }

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
    await trackAnalyticsEvent(
      snapshot.config.analytics,
      {
        type: 'cta-click',
        screen: 'settings',
        action: 'check-for-updates',
        feature: 'updater',
      },
      buildInfo,
    )
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
      await trackAnalyticsEvent(
        snapshot.config.analytics,
        {
          type: 'update-lifecycle',
          screen: 'settings',
          action: 'check',
          status: result.availability.available
            ? 'available'
            : result.availability.error
              ? 'error'
              : result.availability.supported
                ? 'uptodate'
                : 'unsupported',
          version: result.availability.version ?? null,
        },
        buildInfo,
      )
    } catch (error) {
      setUpdateAvailability(null)
      setPendingUpdate(null)
      setUpdateInstallState({
        phase: 'error',
        downloadedBytes: null,
        contentLength: null,
        message:
          error instanceof Error ? error.message : t('common.unavailable'),
      })
    }
  }

  async function handleDownloadAndInstallUpdate() {
    if (!snapshot || !pendingUpdate) {
      return
    }

    await trackAnalyticsEvent(
      snapshot.config.analytics,
      {
        type: 'cta-click',
        screen: 'settings',
        action: 'download-and-install',
        feature: 'updater',
      },
      buildInfo,
    )
    const result = await downloadAndInstallAppUpdate(
      pendingUpdate,
      setUpdateInstallState,
    )
    await trackAnalyticsEvent(
      snapshot.config.analytics,
      {
        type: 'update-lifecycle',
        screen: 'settings',
        action: 'download-and-install',
        status: result.phase,
        version: pendingUpdate.version,
      },
      buildInfo,
    )
  }

  async function handleRelaunchForUpdate() {
    if (!snapshot) {
      return
    }

    await trackAnalyticsEvent(
      snapshot.config.analytics,
      {
        type: 'cta-click',
        screen: 'settings',
        action: 'restart-after-update',
        feature: 'updater',
      },
      buildInfo,
    )
    await relaunchAfterUpdate()
  }

  async function handleOpenReleasePage() {
    await backend.openExternalUrl(
      updateAvailability?.downloadUrl ?? RELEASES_PAGE_URL,
    )
  }

  return {
    analytics: {
      action: analyticsAction,
      configDirty: analyticsConfigDirty,
      currentSettings: currentAnalyticsSettings,
      endpointConfigured: Boolean(CONFIGURED_ANALYTICS_ENDPOINT),
      onEnabledChange: handleAnalyticsEnabledChange,
      onSave: handleSaveAnalyticsConsent,
    },
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
