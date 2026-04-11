/**
 * This module contains reusable front-end helper logic for Ipc.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `UpdaterProgressListener`
 * - `subscribeToUpdaterProgress`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type { UpdateInstallState } from '../types'

/**
 * Defines the type-level contract for updater progress listener.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export type UpdaterProgressListener = (event: UpdateInstallState) => void

/**
 * Subscribes to to updater progress.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export async function subscribeToUpdaterProgress(
  listener: UpdaterProgressListener,
) {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<UpdateInstallState>(
      'pathkeep://updater-progress',
      ({ payload }) => {
        if (payload) listener(payload)
      },
    )
  } catch {
    return () => {}
  }
}
