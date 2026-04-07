import type { BrowserProfile, ScheduleStatus, SecurityStatus } from './types'
import type { TranslationKey } from './i18n'

export type SupportedPlatform = 'macos' | 'windows' | 'linux'

export function normalizePlatform(platform?: string | null): SupportedPlatform {
  if (platform === 'windows') return 'windows'
  if (platform === 'linux') return 'linux'
  return 'macos'
}

export function platformLabelKey(platform?: string | null): TranslationKey {
  const normalized = normalizePlatform(platform)
  if (normalized === 'windows') return 'platform.windowsLabel'
  if (normalized === 'linux') return 'platform.linuxLabel'
  return 'platform.macosLabel'
}

export function platformSummaryKey(platform?: string | null): TranslationKey {
  const normalized = normalizePlatform(platform)
  if (normalized === 'windows') return 'platform.windowsSummary'
  if (normalized === 'linux') return 'platform.linuxSummary'
  return 'platform.macosSummary'
}

export function hasSafariAccessIssue(profiles: BrowserProfile[]) {
  return profiles.some(
    (profile) =>
      profile.browserFamily === 'safari' &&
      !profile.historyExists &&
      Boolean(profile.historyPath),
  )
}

export function needsSchedulerReview(status: ScheduleStatus | null) {
  if (!status) return false
  return (
    status.installState === 'mismatch' ||
    status.installState === 'permission-warning' ||
    status.installState === 'legacy-install-detected' ||
    status.installState === 'manual-review'
  )
}

export function keyringNeedsReview(status: SecurityStatus | null) {
  if (!status) return false
  return !status.keyringStatus.available || status.warnings.length > 0
}
