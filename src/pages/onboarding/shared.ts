/**
 * @file shared.ts
 * @description Shared onboarding constants and pure helpers used by the extracted onboarding step renderers.
 * @module pages/onboarding
 *
 * ## 職責
 * - 提供 onboarding step keys、security draft 型別、與小型 pure helpers。
 * - 保持 onboarding step modules 不必各自複製同一組字面值。
 *
 * ## 不負責
 * - 不持有 route state。
 * - 不渲染任何 UI。
 * - 不執行 backend mutation 或 navigation。
 *
 * ## 依賴關係
 * - 只依賴 TypeScript 基礎型別；保持為 pure helper module。
 *
 * ## 性能備注
 * - helpers 都是固定成本字串/字面值轉換，沒有資料查詢或重計算。
 */

import { scheduledBackupIntervalOptions } from '../../lib/schedule-options'

/**
 * Names the stable onboarding step translation keys used by the stepper.
 */
export const onboardingStepKeys = [
  'stepWelcome',
  'stepBrowsers',
  'stepStorage',
  'stepSecurity',
  'stepSchedule',
  'stepAi',
  'stepReady',
] as const

/**
 * Exposes the allowed onboarding backup interval options.
 */
export const dueAfterOptions = [...scheduledBackupIntervalOptions]

/**
 * Captures the local security draft used during encrypted onboarding.
 */
export interface SecurityDraftState {
  confirmPassword: string
  masterPassword: string
  rememberKey: boolean
}

type Translate = (key: string, vars?: Record<string, string | number>) => string

/**
 * Localizes the platform badge shown in the schedule preview step.
 */
export function schedulePlatformLabel(platform: string, t: Translate) {
  if (platform === 'macos') return t('platform.macosLabel')
  if (platform === 'windows') return t('platform.windowsLabel')
  if (platform === 'linux') return t('platform.linuxLabel')
  return platform
}
