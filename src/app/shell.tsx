import { useEffect, useState } from 'react'
import { Outlet, useLocation, useMatches } from 'react-router-dom'
import { BusyOverlay } from '../components/primitives/busy-overlay'
import { Sidebar } from '../components/sidebar'
import { Topbar } from '../components/topbar'
import { trackAnalyticsEvent } from '../lib/analytics'
import { useShellData } from './shell-data-context'
import { appScreens, readRouteHandle } from './router'

export function AppShell() {
  const { buildInfo, busyAction, busyOverlay, snapshot } = useShellData()
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() =>
    typeof window !== 'undefined' && 'matchMedia' in window
      ? window.matchMedia('(max-width: 1200px)').matches
      : false,
  )
  const location = useLocation()
  const activeScreen =
    [...useMatches()]
      .map((match) => readRouteHandle(match.handle))
      .find(Boolean)?.screen ?? appScreens[0]

  useEffect(() => {
    if (typeof window === 'undefined' || !('matchMedia' in window)) return
    const mediaQuery = window.matchMedia('(max-width: 1200px)')
    const handleChange = (event: MediaQueryListEvent) => {
      setSidebarCollapsed(event.matches)
    }

    mediaQuery.addEventListener('change', handleChange)
    return () => {
      mediaQuery.removeEventListener('change', handleChange)
    }
  }, [])

  useEffect(() => {
    if (!snapshot || !buildInfo) {
      return
    }

    void trackAnalyticsEvent(
      snapshot.config.analytics,
      {
        type: 'route-view',
        route: location.pathname,
        screen: activeScreen.id,
        language: snapshot.config.preferredLanguage,
      },
      buildInfo,
    )
  }, [activeScreen.id, buildInfo, location.pathname, snapshot])

  return (
    <div
      className="app-frame"
      data-sidebar-collapsed={sidebarCollapsed ? 'true' : 'false'}
      data-testid="app-shell"
    >
      <div aria-hidden className="shell-dot-grid" />
      <span aria-hidden className="corner-mark corner-mark--tl">
        +
      </span>
      <span aria-hidden className="corner-mark corner-mark--tr">
        +
      </span>
      <span aria-hidden className="corner-mark corner-mark--bl">
        +
      </span>
      <span aria-hidden className="corner-mark corner-mark--br">
        +
      </span>
      <div className="shell-grid">
        <Sidebar
          collapsed={sidebarCollapsed}
          onToggle={() => setSidebarCollapsed((value) => !value)}
        />
        <div className="workspace-frame">
          <Topbar screen={activeScreen} />
          <main className="workspace-scroll">
            <Outlet />
          </main>
        </div>
      </div>
      {busyAction ? (
        <BusyOverlay
          label={busyOverlay?.label ?? busyAction}
          detail={busyOverlay?.detail}
          progressLabel={busyOverlay?.progressLabel}
          progressValue={busyOverlay?.progressValue}
          steps={busyOverlay?.steps}
          activeStep={busyOverlay?.activeStep}
        />
      ) : null}
    </div>
  )
}
