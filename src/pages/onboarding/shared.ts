/**
 * @file shared.ts
 * @description Shared onboarding constants and pure helpers used by the extracted onboarding step renderers.
 * @module pages/onboarding
 *
 * ## 職責
 * - 提供 onboarding step keys、security draft 型別、與小型 pure helpers。
 * - 讓多個 step renderer 共用 schedule manual-step localization 規則。
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

const dueAfterHours = [6, 12, 24, 72] as const

/**
 * Names the stable onboarding step translation keys used by the stepper.
 */
export const onboardingStepKeys = [
  'stepWelcome',
  'stepBrowsers',
  'stepStorage',
  'stepSecurity',
  'stepSchedule',
  'stepReady',
] as const

/**
 * Exposes the allowed onboarding backup interval options.
 */
export const dueAfterOptions = [...dueAfterHours]

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

/**
 * Rewrites backend-provided schedule manual steps into onboarding-owned copy.
 *
 * The preview contract stays truthful to the backend artifact, but the stepper
 * still needs localized and human-readable copy.
 */
export function localizeScheduleManualStep(
  step: string,
  label: string,
  t: Translate,
) {
  if (step === `Save the plist to ~/Library/LaunchAgents/${label}.plist.`) {
    return t('scheduleManualStepLaunchAgentSave', { label })
  }
  if (
    step ===
    `Run \`launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/${label}.plist\` to load the new schedule.`
  ) {
    return t('scheduleManualStepLaunchAgentBootstrap', { label })
  }
  if (
    step ===
    'Open the desktop build to verify the LaunchAgent artifact and install status.'
  ) {
    return t('scheduleManualStepLaunchAgentReviewInstalled')
  }
  if (step === 'Remove the LaunchAgent if you no longer want automation.') {
    return t('scheduleManualStepLaunchAgentRemove')
  }
  if (
    step === 'Review the XML file before registering it with Task Scheduler.' ||
    step === 'Save the XML file and import it in Task Scheduler.'
  ) {
    return t('scheduleManualStepWindowsSaveXml')
  }
  if (
    step ===
      `PathKeep can register it with \`schtasks /Create /TN ${label} /XML <generated XML> /F\`.` ||
    step ===
      `Alternatively run \`schtasks /Create /TN ${label} /XML ${label}.task.xml\`.`
  ) {
    return t('scheduleManualStepWindowsCreateTask', { label })
  }
  if (step === 'Copy the files to ~/.config/systemd/user/.') {
    return t('scheduleManualStepLinuxCopy')
  }
  if (step === 'Run `systemctl --user daemon-reload`.') {
    return t('scheduleManualStepLinuxReload')
  }
  if (step === `Run \`systemctl --user enable --now ${label}.timer\`.`) {
    return t('scheduleManualStepLinuxEnable', { label })
  }
  if (
    step ===
    `Run \`systemctl --user list-timers ${label}.timer\` to verify the next scheduled run.`
  ) {
    return t('scheduleManualStepLinuxVerify', { label })
  }
  return step
}
