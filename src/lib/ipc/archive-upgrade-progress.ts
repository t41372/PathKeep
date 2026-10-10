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
import { listenDesktopEvent } from './events'

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
 * screen relies on it as a shared contract. In the app and through the dev
 * bridge alike (`listenDesktopEvent`).
 */
export async function subscribeToArchiveUpgradeProgress(
  listener: ArchiveUpgradeProgressListener,
) {
  return listenDesktopEvent<ArchiveUpgradeProgress | null>(
    'pathkeep://archive-upgrade',
    (payload) => {
      if (payload) listener(payload)
    },
  )
}
