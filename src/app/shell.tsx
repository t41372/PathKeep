/**
 * This module renders the persistent desktop chrome around route content, including sidebar, topbar, and shell-wide busy UI.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `AppShell`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import { useEffect, useState } from 'react'
import { Outlet, useMatches } from 'react-router-dom'
import { BusyOverlay } from '../components/primitives/busy-overlay'
import { Sidebar } from '../components/sidebar'
import { Topbar } from '../components/topbar'
import { useShellData } from './shell-data-context'
import { appScreens, readRouteHandle } from './router'

/**
 * Renders the app shell wrapper.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export function AppShell() {
  const { busyAction, busyOverlay } = useShellData()
  const matchMedia = window.matchMedia as Window['matchMedia'] | undefined
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() =>
    matchMedia ? matchMedia('(max-width: 1200px)').matches : false,
  )
  const activeScreen =
    [...useMatches()]
      .map((match) => readRouteHandle(match.handle))
      .find(Boolean)?.screen ?? appScreens[0]

  useEffect(() => {
    if (!matchMedia) {
      return
    }
    const mediaQuery = matchMedia('(max-width: 1200px)')
    /**
     * Handles change.
     *
     * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
     */
    const handleChange = (event: MediaQueryListEvent) => {
      setSidebarCollapsed(event.matches)
    }

    mediaQuery.addEventListener('change', handleChange)
    return () => {
      mediaQuery.removeEventListener('change', handleChange)
    }
  }, [matchMedia])

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
          logLines={busyOverlay?.logLines}
        />
      ) : null}
    </div>
  )
}
