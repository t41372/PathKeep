import type { ReactElement } from 'react'
import { Navigate, useLocation } from 'react-router-dom'
import { LoadingState } from '../components/primitives/loading-state'
import { useI18n } from '../lib/i18n'
import { LockPage } from '../pages/lock'
import { useShellData } from './shell-data-context'

function RouteLoadingGate() {
  const { t } = useI18n()

  return (
    <section className="page-shell">
      <LoadingState label={t('common.loading')} />
    </section>
  )
}

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
