import type { BrowserProfile } from './types'

const MANIFEST_BASE_BYTES = 256 * 1024
const MANIFEST_PER_PROFILE_BYTES = 64 * 1024

export interface OnboardingStorageEstimate {
  profileCount: number
  sourceBytes: number
  archiveDbBytes: number
  manifestBytes: number
  snapshotsBytes: number
  totalBytes: number
}

function selectedReadableProfiles(
  profiles: BrowserProfile[],
  selectedProfileIds: string[],
) {
  const selected = new Set(selectedProfileIds)
  return profiles.filter(
    (profile) => profile.historyExists && selected.has(profile.profileId),
  )
}

function detectedProfileBytes(profile: BrowserProfile) {
  return profile.historyBytes + profile.faviconsBytes + profile.supportingBytes
}

export function estimateOnboardingStorage(
  profiles: BrowserProfile[],
  selectedProfileIds: string[],
): OnboardingStorageEstimate {
  const readableProfiles = selectedReadableProfiles(
    profiles,
    selectedProfileIds,
  )
  const sourceBytes = readableProfiles.reduce(
    (total, profile) => total + detectedProfileBytes(profile),
    0,
  )
  const profileCount = readableProfiles.length
  const archiveDbBytes = Math.ceil((sourceBytes * 4) / 5)
  const manifestBytes =
    profileCount === 0
      ? 0
      : Math.max(MANIFEST_BASE_BYTES, profileCount * MANIFEST_PER_PROFILE_BYTES)
  const snapshotsBytes = sourceBytes

  return {
    profileCount,
    sourceBytes,
    archiveDbBytes,
    manifestBytes,
    snapshotsBytes,
    totalBytes: archiveDbBytes + manifestBytes + snapshotsBytes,
  }
}
