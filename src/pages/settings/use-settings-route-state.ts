/**
 * @file use-settings-route-state.ts
 * @description Composes the smaller Settings domain hooks into one route-level state surface for the Settings page shell.
 * @module pages/settings
 *
 * ## 職責
 * - 作為 Settings route 的 canonical state composition point。
 * - 合併 support、updater、AI、derived-state、與 remote-backup hooks。
 * - 對 `settings/index.tsx` 暴露穩定的 grouped section state。
 *
 * ## 不負責
 * - 不直接實作各 domain 的 draft state 或副作用。
 * - 不渲染任何 Settings section。
 * - 不建立平行的 state owner。
 *
 * ## 依賴關係
 * - 依賴各個 `use-settings-*-state` domain hooks。
 * - 依賴 shell snapshot/actions 作為 shared input。
 *
 * ## 性能備注
 * - aggregator 本身只做 hook composition，不做額外查詢或重計算。
 */

import { useCallback } from 'react'
import type {
  AppBuildInfo,
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  DashboardSnapshot,
} from '../../lib/types'
import { useSettingsAiState } from './use-settings-ai-state'
import { useSettingsDerivedState } from './use-settings-derived-state'
import { useSettingsSupportState } from './use-settings-support-state'
import { useSettingsUpdaterState } from './use-settings-updater-state'

interface UseSettingsRouteStateArgs {
  appLockStatus: AppLockStatus | null
  buildInfo: AppBuildInfo | null
  clearAppLockPasscode: () => Promise<AppLockStatus>
  dashboard: DashboardSnapshot | null
  enableAiIntegrationPreview?: boolean
  enableDerivedRuntime?: boolean
  enableRetentionPreview?: boolean
  lockAppSession: (reason?: string | null) => Promise<AppLockStatus>
  refreshAppData: () => Promise<void>
  refreshKey: number
  saveConfig: (
    config: AppConfig,
    options?: { quiet?: boolean },
  ) => Promise<AppSnapshot>
  setAppLockPasscode: (request: {
    passcode: string
    recoveryHint?: string | null
  }) => Promise<AppLockStatus>
  setLanguagePreference: (language: AppConfig['preferredLanguage']) => void
  snapshot: AppSnapshot | null
}

/**
 * Composes the smaller Settings domain hooks into one route-level state object.
 *
 * The Settings page stays simple by consuming one grouped state object, while
 * each workflow keeps its own focused owner under the hood.
 */
export function useSettingsRouteState(args: UseSettingsRouteStateArgs) {
  // The Settings page is all-auto-save: every individual toggle / select / blur
  // persists immediately. Route those writes through `quiet` so a tiny config
  // write never throws the blocking full-screen `BusyOverlay` (which would freeze
  // the main thread on every control). The shell still refreshes the snapshot,
  // language, app-lock, and dashboard exactly the same — only the overlay is
  // suppressed, leaving each section's inline "Saved" chip as the confirmation.
  // Wrapping here (not per-hook) keeps every settings save path quiet by default.
  const { saveConfig } = args
  const quietSaveConfig = useCallback(
    (config: AppConfig) => saveConfig(config, { quiet: true }),
    [saveConfig],
  )
  const supportState = useSettingsSupportState({
    appLockStatus: args.appLockStatus,
    clearAppLockPasscode: args.clearAppLockPasscode,
    enableRetentionPreview: args.enableRetentionPreview,
    lockAppSession: args.lockAppSession,
    refreshAppData: args.refreshAppData,
    refreshKey: args.refreshKey,
    saveConfig: quietSaveConfig,
    setAppLockPasscode: args.setAppLockPasscode,
    setLanguagePreference: args.setLanguagePreference,
    snapshot: args.snapshot,
  })
  const updaterState = useSettingsUpdaterState({
    buildInfo: args.buildInfo,
    snapshot: args.snapshot,
  })
  const aiState = useSettingsAiState({
    enableIntegrationPreview: args.enableAiIntegrationPreview,
    refreshAppData: args.refreshAppData,
    saveConfig: quietSaveConfig,
    snapshot: args.snapshot,
  })
  const derivedState = useSettingsDerivedState({
    dashboard: args.dashboard,
    enabled: args.enableDerivedRuntime,
    refreshAppData: args.refreshAppData,
    refreshKey: args.refreshKey,
    saveConfig: quietSaveConfig,
    snapshot: args.snapshot,
  })
  return {
    ...supportState,
    ...updaterState,
    ...aiState,
    ...derivedState,
  }
}
