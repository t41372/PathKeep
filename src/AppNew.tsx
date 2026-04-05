import { AppProvider, useApp } from './lib/app-context'
import { Sidebar } from './components/sidebar'
import { DashboardPage } from './pages/dashboard'
import { ExplorerPage } from './pages/explorer'
import { InsightsPage } from './pages/insights'
import { ActivityLogPage } from './pages/activity-log'
import { ImportPage } from './pages/import'
import { SettingsPage } from './pages/settings'
import { OnboardingPage } from './pages/onboarding'
import { Glyph } from './components/ui'
import './App.css'

function AppShell() {
  const {
    activePage,
    busyLabel,
    notice,
    error,
    setNotice,
    setError,
    initialized,
  } = useApp()

  const renderPage = () => {
    // Show onboarding when not initialized
    if (activePage === 'onboarding' || !initialized) {
      return <OnboardingPage />
    }

    switch (activePage) {
      case 'dashboard':
        return <DashboardPage />
      case 'explorer':
        return <ExplorerPage />
      case 'insights':
        return <InsightsPage />
      case 'activity':
        return <ActivityLogPage />
      case 'import':
        return <ImportPage />
      case 'settings':
        return <SettingsPage />
      /* v8 ignore next 2 -- all pages covered above; fallback is defensive */
      default:
        return <DashboardPage />
    }
  }

  const showSidebar = initialized && activePage !== 'onboarding'

  return (
    <div className="appShell" data-testid="app-shell">
      {showSidebar && <Sidebar />}
      <main className="mainPane">
        {renderPage()}

        {/* Busy overlay */}
        {busyLabel && (
          <div className="busyOverlay" role="status">
            <div className="busyCard">
              <div className="spinner" />
              <span>{busyLabel}</span>
            </div>
          </div>
        )}

        {/* Toast notices */}
        {notice && (
          <output className="toast success" role="log">
            <Glyph icon="check_circle" />
            <span>{notice}</span>
            <button
              className="toastClose"
              type="button"
              onClick={() => setNotice(null)}
            >
              <Glyph icon="close" />
            </button>
          </output>
        )}

        {/* Error banner */}
        {error && (
          <output className="toast danger" role="alert">
            <Glyph icon="error" />
            <span>{error}</span>
            <button
              className="toastClose"
              type="button"
              onClick={() => setError(null)}
            >
              <Glyph icon="close" />
            </button>
          </output>
        )}
      </main>
    </div>
  )
}

export default function App() {
  return (
    <AppProvider>
      <AppShell />
    </AppProvider>
  )
}
