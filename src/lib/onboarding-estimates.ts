/**
 * This module estimates onboarding storage impact so setup screens can stay informative before a first backup runs.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `OnboardingStorageEstimate`
 * - `estimateOnboardingStorage`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { isBrowserProfileReadable } from './platform-guidance'
import type { BrowserProfile } from './types'

const MANIFEST_BASE_BYTES = 256 * 1024
const MANIFEST_PER_PROFILE_BYTES = 64 * 1024

/**
 * Defines the typed shape for onboarding storage estimate.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export interface OnboardingStorageEstimate {
  profileCount: number
  sourceBytes: number
  archiveDbBytes: number
  manifestBytes: number
  snapshotsBytes: number
  totalBytes: number
}

/**
 * Returns the selected readable profiles.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function selectedReadableProfiles(
  profiles: BrowserProfile[],
  selectedProfileIds: string[],
) {
  const selected = new Set(selectedProfileIds)
  return profiles.filter(
    (profile) =>
      isBrowserProfileReadable(profile) && selected.has(profile.profileId),
  )
}

/**
 * Explains how detected profile bytes works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function detectedProfileBytes(profile: BrowserProfile) {
  return profile.historyBytes + profile.faviconsBytes + profile.supportingBytes
}

/**
 * Explains how estimate onboarding storage works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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
