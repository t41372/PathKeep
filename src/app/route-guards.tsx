/**
 * This module enforces the app-lock routing boundary between the unlocked shell and the standalone lock screen.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `RequireUnlockedShell`
 * - `RequireLockScreen`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import type { ReactElement } from 'react'
import { Navigate, useLocation } from 'react-router-dom'
import { LoadingState } from '../components/primitives/loading-state'
import { useI18n } from '../lib/i18n'
import { LockPage } from '../pages/lock'
import { useShellData } from './shell-data-context'

/**
 * Explains how route loading gate works.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
function RouteLoadingGate() {
  const { t } = useI18n()

  return (
    <section className="page-shell">
      <LoadingState label={t('common.loading')} />
    </section>
  )
}

/**
 * Renders the require unlocked shell wrapper.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export function RequireUnlockedShell({ children }: { children: ReactElement }) {
  const { appLockStatus, loading } = useShellData()
  const location = useLocation()

  if (loading && appLockStatus === null) {
    return <RouteLoadingGate />
  }

  if (appLockStatus?.locked) {
    const next = `${location.pathname}${location.search}${location.hash}`
    return <Navigate replace to={`/lock?next=${encodeURIComponent(next)}`} />
  }

  return children
}

/**
 * Enforces the require lock screen boundary before rendering child UI.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export function RequireLockScreen() {
  const { appLockStatus, loading, snapshot } = useShellData()
  const location = useLocation()
  const next =
    new URLSearchParams(location.search).get('next')?.trim() ||
    (snapshot?.config.initialized ? '/' : '/onboarding')

  if (loading && appLockStatus === null) {
    return <RouteLoadingGate />
  }

  if (!appLockStatus?.enabled || !appLockStatus.locked) {
    return <Navigate replace to={next} />
  }

  return <LockPage />
}
