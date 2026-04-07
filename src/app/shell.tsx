import { Outlet, useMatches } from 'react-router-dom'
import { Sidebar } from '../components/sidebar'
import { Topbar } from '../components/topbar'
import { appScreens, readRouteHandle } from './router'

export function AppShell() {
  const activeScreen =
    [...useMatches()]
      .map((match) => readRouteHandle(match.handle))
      .find(Boolean)?.screen ?? appScreens[0]

  return (
    <div className="app-frame" data-testid="app-shell">
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
        <Sidebar />
        <div className="workspace-frame">
          <Topbar screen={activeScreen} />
          <main className="workspace-scroll">
            <Outlet />
          </main>
        </div>
      </div>
    </div>
  )
}
