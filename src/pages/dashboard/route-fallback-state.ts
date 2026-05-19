/**
 * @file route-fallback-state.ts
 * @description Pure fallback-state resolver for the Dashboard route before the main dashboard shell can render.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Decide which Dashboard fallback branch should render from the current shell bootstrap inputs.
 * - Keep loading, onboarding, unlock-required, read-error, and unavailable decisions in one deterministic owner.
 *
 * ## Not responsible for
 * - Rendering fallback UI.
 * - Fetching dashboard data or security status.
 * - Defining shared loading/empty/error primitives.
 *
 * ## Dependencies
 * - Depends on the dashboard/app snapshot types and archive-access unlock detection helper.
 *
 * ## Performance notes
 * - Pure synchronous logic so Dashboard bootstrap gating stays cheap on cold and error paths.
 */

import { isArchiveUnlockRequiredMessage } from '../../lib/archive-access'
import type { AppSnapshot, DashboardSnapshot } from '../../lib/types'

/**
 * Minimal archive access snapshot used to recover a truthful Dashboard fallback
 * after shell bootstrap fails before `dashboard` exists.
 */
export interface DashboardArchiveAccessFallback {
  encrypted: boolean
  initialized: boolean
  unlocked: boolean
}

/**
 * Route fallback states the Dashboard shell can resolve before the full page is renderable.
 */
export type DashboardRouteFallbackState =
  | { kind: 'loading' }
  | { kind: 'onboarding-zero-state' }
  | { kind: 'unlock-required' }
  | { description: string; kind: 'read-error' }
  | { kind: 'archive-unavailable' }
  | { kind: 'ready' }

interface ResolveDashboardRouteFallbackArgs {
  archiveAccessFallback: DashboardArchiveAccessFallback | null
  dashboard: DashboardSnapshot | null
  dashboardLoading: boolean
  error: string | null
  loading: boolean
  snapshot: AppSnapshot | null
}

/**
 * Resolves the Dashboard fallback state before the populated route shell can render.
 *
 * This keeps the route bootstrap behavior deterministic and easy to test: the
 * same inputs always map to the same loading/unlock/onboarding/unavailable
 * branch.
 */
export function resolveDashboardRouteFallback({
  archiveAccessFallback,
  dashboard,
  dashboardLoading,
  error,
  loading,
  snapshot,
}: ResolveDashboardRouteFallbackArgs): DashboardRouteFallbackState {
  if ((loading || dashboardLoading) && !dashboard) {
    return { kind: 'loading' }
  }

  if (error && !dashboard) {
    const needsArchiveUnlock =
      isArchiveUnlockRequiredMessage(error) ||
      (archiveAccessFallback?.initialized === true &&
        archiveAccessFallback.encrypted &&
        !archiveAccessFallback.unlocked)

    if (archiveAccessFallback?.initialized === false) {
      return { kind: 'onboarding-zero-state' }
    }

    if (needsArchiveUnlock) {
      return { kind: 'unlock-required' }
    }

    return { description: error, kind: 'read-error' }
  }

  if (snapshot && !snapshot.config.initialized) {
    return { kind: 'onboarding-zero-state' }
  }

  if (!snapshot || !dashboard) {
    return { kind: 'archive-unavailable' }
  }

  return { kind: 'ready' }
}
