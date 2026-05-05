/**
 * This module wraps the desktop updater boundary in small front-end helpers and preview-aware fallbacks.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `RELEASES_PAGE_URL`
 * - `initialUpdateInstallState`
 * - `checkForAppUpdate`
 * - `downloadAndInstallAppUpdate`
 * - `relaunchAfterUpdate`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { backend } from './backend-client'
import { subscribeToUpdaterProgress } from './ipc/updater-progress'
import { hasDesktopCommandTransport, hasTauriGuestApi } from './runtime'
import type {
  AppUpdateCheckResult,
  PendingAppUpdate,
  UpdateAvailability,
  UpdateInstallState,
} from './types'

export type { PendingAppUpdate }

export const RELEASES_PAGE_URL = 'https://github.com/t41372/PathKeep/releases'

/**
 * Explains how now iso works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function nowIso() {
  return new Date().toISOString()
}

/**
 * Explains how preview availability works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function previewAvailability(
  currentVersion?: string | null,
): AppUpdateCheckResult {
  return {
    availability: {
      supported: false,
      checkedAt: nowIso(),
      available: false,
      currentVersion: currentVersion ?? null,
      version: null,
      notes: null,
      publishedAt: null,
      error:
        'In-browser preview cannot check desktop update channels. Use a packaged desktop build instead.',
      downloadUrl: RELEASES_PAGE_URL,
    },
    pendingUpdate: null,
  }
}

/**
 * Builds the initial update install state.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function initialUpdateInstallState(): UpdateInstallState {
  return {
    phase: 'idle',
    version: null,
    downloadedBytes: null,
    contentLength: null,
    message: null,
  }
}

/**
 * Explains how check for app update works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export async function checkForAppUpdate(
  currentVersion?: string | null,
): Promise<AppUpdateCheckResult> {
  if (!hasDesktopCommandTransport()) {
    return previewAvailability(currentVersion)
  }

  const result = await backend.checkForAppUpdate()
  return {
    availability: {
      ...result.availability,
      currentVersion:
        result.availability.currentVersion ?? currentVersion ?? null,
      downloadUrl: result.availability.downloadUrl ?? RELEASES_PAGE_URL,
    } satisfies UpdateAvailability,
    pendingUpdate: result.pendingUpdate,
  }
}

/**
 * Explains how download and install app update works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export async function downloadAndInstallAppUpdate(
  pendingUpdate: PendingAppUpdate,
  onStateChange?: (state: UpdateInstallState) => void,
) {
  if (!hasDesktopCommandTransport()) {
    const unsupported = {
      phase: 'unsupported',
      version: pendingUpdate.version,
      message: 'In-browser preview cannot download or install desktop updates.',
      downloadedBytes: null,
      contentLength: null,
    } satisfies UpdateInstallState
    onStateChange?.(unsupported)
    return unsupported
  }

  let lastProgressPhase: UpdateInstallState['phase'] | null = null
  /**
   * Explains how unsubscribe works.
   *
   * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
   */
  let unsubscribe = () => {}
  if (hasTauriGuestApi()) {
    unsubscribe = await subscribeToUpdaterProgress((state) => {
      lastProgressPhase = state.phase
      onStateChange?.(state)
    })
  }

  try {
    const result = await backend.downloadAndInstallAppUpdate(
      pendingUpdate.version,
    )
    if (lastProgressPhase !== result.phase) {
      onStateChange?.(result)
    }
    return result
  } catch (error) {
    const failed = {
      phase: 'error',
      version: pendingUpdate.version,
      downloadedBytes: null,
      contentLength: null,
      message:
        error instanceof Error
          ? error.message
          : `PathKeep could not install ${pendingUpdate.version}.`,
    } satisfies UpdateInstallState
    onStateChange?.(failed)
    return failed
  } finally {
    unsubscribe()
  }
}

/**
 * Explains how relaunch after update works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export async function relaunchAfterUpdate() {
  if (!hasDesktopCommandTransport()) {
    return false
  }

  return await backend.relaunchAfterUpdate()
}
