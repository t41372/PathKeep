/**
 * @file route-fallback.tsx
 * @description Canonical loading and error-state renderer for Dashboard route fallback branches.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Render the shared loading, unlock-required, onboarding-zero-state, and unavailable states with one consistent owner.
 *
 * ## Not responsible for
 * - Deciding which fallback branch should render.
 * - Fetching dashboard data or security status.
 * - Rendering the populated dashboard or dashboard zero-state branch after data is available.
 *
 * ## Dependencies
 * - Depends on dashboard i18n copy, the fallback-state resolver, and the existing shared loading/empty/error primitives.
 *
 * ## Performance notes
 * - Render-only component; keep it side-effect free so first-paint gating stays cheap.
 */

import { Link } from 'react-router-dom'
import { EmptyState } from '../../components/primitives/empty-state'
import { ErrorState } from '../../components/primitives/error-state'
import { DashboardSkeleton } from '../../components/primitives/skeleton'
import type { DashboardRouteFallbackState } from './route-fallback-state'

type Translate = (key: string, vars?: Record<string, string | number>) => string

interface DashboardRouteFallbackProps {
  state: Exclude<DashboardRouteFallbackState, { kind: 'ready' }>
  t: Translate
}

/**
 * Renders the Dashboard fallback branch chosen by `resolveDashboardRouteFallback`.
 *
 * The route shell can return early through this component so the remaining
 * dashboard composition only deals with the ready/data-backed branches.
 */
export function DashboardRouteFallback({
  state,
  t,
}: DashboardRouteFallbackProps) {
  return (
    <section className="page-shell" data-testid="dashboard-page">
      {state.kind === 'loading' ? (
        <DashboardSkeleton label={t('common.loadingDashboard')} />
      ) : null}
      {state.kind === 'onboarding-zero-state' ? (
        <EmptyState
          eyebrow={t('dashboard.zeroStateEyebrow')}
          title={t('dashboard.zeroStateTitle')}
          description={t('dashboard.zeroStateBody')}
          action={
            <Link className="btn-primary" to="/onboarding">
              {t('dashboard.openOnboardingFlow')}
            </Link>
          }
        />
      ) : null}
      {state.kind === 'unlock-required' ? (
        <ErrorState
          eyebrow={t('dashboard.archiveNeedsUnlock')}
          title={t('dashboard.archiveUnlockRequiredTitle')}
          description={t('dashboard.archiveUnlockRequiredBody')}
          action={
            <Link className="btn-primary" to="/security#unlock-archive">
              {t('dashboard.archiveUnlockAction')}
            </Link>
          }
        />
      ) : null}
      {state.kind === 'read-error' ? (
        <ErrorState
          title={t('dashboard.archiveReadError')}
          description={state.description}
        />
      ) : null}
      {state.kind === 'archive-unavailable' ? (
        <ErrorState
          title={t('dashboard.archiveUnavailable')}
          description={t('dashboard.archiveUnavailableBody')}
        />
      ) : null}
    </section>
  )
}
