/**
 * This module turns raw platform facts into the warning grammar used by schedule, security, and settings surfaces.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `SupportedPlatform`
 * - `normalizePlatform`
 * - `platformLabelKey`
 * - `platformSummaryKey`
 * - `macosFullDiskAccessSettingsUrl`
 * - `hasSafariAccessIssue`
 * - `needsSchedulerReview`
 * - `keyringNeedsReview`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type { BrowserProfile, ScheduleStatus, SecurityStatus } from './types'
import type { TranslationKey } from './i18n'

/**
 * Defines the type-level contract for supported platform.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export type SupportedPlatform = 'macos' | 'windows' | 'linux'

/**
 * Provides the single native settings URL PathKeep may open for Safari access recovery.
 *
 * This constant stays in the shared platform-policy module so Import, onboarding,
 * and future permission callouts use the same tightly scoped launcher target.
 */
export const macosFullDiskAccessSettingsUrl =
  'x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles'

/**
 * Returns whether a discovered browser profile can be backed up without asking
 * the user for extra OS permissions first.
 */
export function isBrowserProfileReadable(profile: BrowserProfile) {
  return profile.historyExists && profile.historyReadable !== false
}

/**
 * Returns whether the profile exists but cannot be read because of host access policy.
 */
export function hasBrowserProfileAccessIssue(profile: BrowserProfile) {
  return profile.historyExists && profile.historyReadable === false
}

/**
 * Normalizes platform into the canonical UI shape.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function normalizePlatform(platform?: string | null): SupportedPlatform {
  if (platform === 'windows') return 'windows'
  if (platform === 'linux') return 'linux'
  return 'macos'
}

/**
 * Explains how platform label key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function platformLabelKey(platform?: string | null): TranslationKey {
  const normalized = normalizePlatform(platform)
  if (normalized === 'windows') return 'platform.windowsLabel'
  if (normalized === 'linux') return 'platform.linuxLabel'
  return 'platform.macosLabel'
}

/**
 * Explains how platform summary key works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function platformSummaryKey(platform?: string | null): TranslationKey {
  const normalized = normalizePlatform(platform)
  if (normalized === 'windows') return 'platform.windowsSummary'
  if (normalized === 'linux') return 'platform.linuxSummary'
  return 'platform.macosSummary'
}

/**
 * Returns whether safari access issue.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function hasSafariAccessIssue(profiles: BrowserProfile[]) {
  return profiles.some(
    (profile) =>
      profile.browserFamily === 'safari' &&
      (hasBrowserProfileAccessIssue(profile) ||
        (!profile.historyExists && Boolean(profile.historyPath))),
  )
}

/**
 * Explains how needs scheduler review works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function needsSchedulerReview(status: ScheduleStatus | null) {
  if (!status) return false
  return (
    status.installState === 'mismatch' ||
    status.installState === 'permission-warning' ||
    status.installState === 'legacy-install-detected' ||
    status.installState === 'manual-review'
  )
}

/**
 * Explains how keyring needs review works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function keyringNeedsReview(status: SecurityStatus | null) {
  if (!status) return false
  return !status.keyringStatus.available || status.warnings.length > 0
}
