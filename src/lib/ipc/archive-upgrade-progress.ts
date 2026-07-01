/**
 * This module contains reusable front-end helper logic for Ipc.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `ArchiveUpgradeProgressListener`
 * - `subscribeToArchiveUpgradeProgress`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type { ArchiveUpgradeProgress } from '../types'

/**
 * Defines the type-level contract for archive-upgrade progress listener.
 *
 * This helper should stay small, explicit, and easy to test because the upgrade
 * screen relies on it as a shared contract.
 */
export type ArchiveUpgradeProgressListener = (
  event: ArchiveUpgradeProgress,
) => void

/**
 * Subscribes to the one-time archive-upgrade progress channel.
 *
 * This helper should stay small, explicit, and easy to test because the upgrade
 * screen relies on it as a shared contract. It degrades to a no-op unsubscribe
 * when the Tauri event bridge is unavailable (browser preview / non-desktop).
 */
export async function subscribeToArchiveUpgradeProgress(
  listener: ArchiveUpgradeProgressListener,
) {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    return await listen<ArchiveUpgradeProgress>(
      'pathkeep://archive-upgrade',
      ({ payload }) => {
        if (payload) listener(payload)
      },
    )
  } catch {
    return () => {}
  }
}
