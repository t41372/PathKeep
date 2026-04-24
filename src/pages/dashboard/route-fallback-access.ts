/**
 * @file route-fallback-access.ts
 * @description Dashboard archive-access probe used when shell bootstrap fails before dashboard data exists.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Fetch the small Security read model needed to distinguish onboarding, locked archive, and generic read-error fallbacks.
 * - Keep the archive-access fallback DTO beside the Dashboard fallback resolver that consumes it.
 * - Reset stale fallback state as soon as the route has a normal dashboard or snapshot again.
 *
 * ## Not responsible for
 * - Rendering Dashboard fallback UI.
 * - Loading the full dashboard snapshot or mutating app-lock/security state.
 * - Deciding final fallback branch order; `route-fallback-state.ts` remains that pure decision owner.
 *
 * ## Dependencies
 * - Depends on the backend client Security status command and Dashboard fallback-state types.
 * - Consumed by `src/pages/dashboard/index.tsx`.
 *
 * ## Performance notes
 * - Runs only on Dashboard bootstrap error paths, so healthy dashboard renders do not add backend traffic.
 * - Reads only the small Security status payload instead of retrying the heavier dashboard read model.
 */

import { useEffect, useState } from 'react'
import { backend } from '../../lib/backend-client'
import type {
  AppSnapshot,
  DashboardSnapshot,
  SecurityStatus,
} from '../../lib/types'
import type { DashboardArchiveAccessFallback } from './route-fallback-state'

interface DashboardArchiveAccessProbeInputs {
  dashboard: DashboardSnapshot | null
  error: string | null
  snapshot: AppSnapshot | null
}

interface DashboardArchiveAccessFallbackOptions extends DashboardArchiveAccessProbeInputs {
  refreshKey: number
}

/**
 * Returns true when Dashboard should ask Security for a tiny archive-access fallback.
 *
 * The route only needs this when the shell reports an error before either the dashboard or
 * snapshot read models are available; otherwise the normal shell data is already more authoritative.
 */
export function shouldProbeDashboardArchiveAccessFallback({
  dashboard,
  error,
  snapshot,
}: DashboardArchiveAccessProbeInputs) {
  return Boolean(error && !dashboard && !snapshot)
}

/**
 * Narrows the Security status payload to the three fields Dashboard fallback resolution needs.
 *
 * Keeping this conversion explicit prevents the Dashboard route from depending on the full
 * Security read model shape just to tell locked and uninitialized archives apart.
 */
export function toDashboardArchiveAccessFallback(
  status: Pick<SecurityStatus, 'initialized' | 'encrypted' | 'unlocked'>,
): DashboardArchiveAccessFallback {
  return {
    initialized: status.initialized,
    encrypted: status.encrypted,
    unlocked: status.unlocked,
  }
}

/**
 * Loads Dashboard's archive-access fallback status only on bootstrap error paths.
 *
 * The hook returns `null` for normal route states and failed probes. `resolveDashboardRouteFallback`
 * remains responsible for combining this best-effort signal with shell loading/error state.
 */
export function useDashboardArchiveAccessFallback({
  dashboard,
  error,
  refreshKey,
  snapshot,
}: DashboardArchiveAccessFallbackOptions) {
  const [archiveAccessFallback, setArchiveAccessFallback] =
    useState<DashboardArchiveAccessFallback | null>(null)

  useEffect(() => {
    let cancelled = false

    if (
      !shouldProbeDashboardArchiveAccessFallback({ dashboard, error, snapshot })
    ) {
      queueMicrotask(() => {
        if (!cancelled) {
          setArchiveAccessFallback(null)
        }
      })
      return () => {
        cancelled = true
      }
    }

    void backend
      .securityStatus()
      .then((status) => {
        if (cancelled) {
          return
        }
        setArchiveAccessFallback(toDashboardArchiveAccessFallback(status))
      })
      .catch(() => {
        if (!cancelled) {
          setArchiveAccessFallback(null)
        }
      })

    return () => {
      cancelled = true
    }
  }, [dashboard, error, refreshKey, snapshot])

  return archiveAccessFallback
}
