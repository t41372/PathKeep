/**
 * App shell composition for the v0.3 paper redesign.
 *
 * Why this file exists:
 * - The shell stitches sidebar + topbar + outlet + status bar + palette into
 *   a coherent paper-aesthetic frame around every route.
 * - It owns shell-only UI state (theme, collapsed sidebar, palette open, active
 *   source filter, stable epigraph index) and forwards everything else to the
 *   useShellData context.
 *
 * Not responsible for:
 * - Backend orchestration (lives in shell-data-actions / shell-data-context).
 * - Page-specific UI (rendered through React Router's <Outlet />).
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Outlet, useMatches, useNavigate } from 'react-router-dom'
import { BackgroundProgress } from '@/components/primitives/background-progress'
import { BusyOverlay } from '@/components/primitives/busy-overlay'
import {
  PKSearchPalette,
  PKSidebar,
  PKStatusBar,
  PKTopbar,
  type PaletteResult,
  type PKStatusBarSource,
} from '@/components/shell'
import { backend } from '@/lib/backend-client'
import {
  formatBuildRevisionLabel,
  formatBuildVersionTitle,
} from '@/lib/build-info'
import { useI18n } from '@/lib/i18n'
import { useShellData } from './shell-data-context'
import { appScreens, readRouteHandle } from './router'
import {
  EPIGRAPH_POOL_SIZE,
  extractDomain,
  formatLastArchivedLabel,
  formatSinceLabel,
  humanizeBytes,
  readBoolean,
  readEpigraphIndex,
  sumStorageBytes,
} from './shell-helpers'
import {
  PAPER_PREFERENCES_EVENT,
  applyPaperPreferences,
  readPaperPreferences,
  type PaperPreferencesEventDetail,
} from '@/lib/paper-preferences'
import { useProfileScope } from '@/lib/profile-scope-context'

const SIDEBAR_KEY = 'pathkeep.sidebar.collapsed'
const EPIGRAPH_KEY = 'pathkeep.epigraph'

/**
 * Resolved at call time, not at module load — vitest's setup file replaces
 * `window.localStorage` with a Map-backed mock inside `beforeAll`, which
 * runs *after* this module is evaluated. Capturing the reference statically
 * would freeze it to the pre-mock object and silently miss test mutations.
 */
function shellStorage(): Storage {
  return window.localStorage
}

const SOURCE_COLORS: Record<string, string> = {
  Chrome: '#4285F4',
  Edge: '#0078D4',
  Firefox: '#FF6B35',
  Safari: '#FF7139',
  Brave: '#FB542B',
  Arc: '#8B65F2',
  ChatGPTAtlas: '#10A37F',
  PerplexityComet: '#7C3AED',
  Opera: '#FF1B2D',
  Vivaldi: '#EF3939',
}

/**
 * Renders the desktop app shell. Composes sidebar, topbar, status bar, route
 * outlet, and the global ⌘K search palette.
 */
export function AppShell() {
  const shell = useShellData()
  const navigate = useNavigate()
  const { language, t } = useI18n()

  const matches = useMatches()
  const activeScreen =
    [...matches].map((match) => readRouteHandle(match.handle)).find(Boolean)
      ?.screen ?? appScreens[0]

  const [sidebarCollapsed, setSidebarCollapsed] = useState<boolean>(() =>
    readBoolean(SIDEBAR_KEY, false, shellStorage()),
  )
  // Single source of truth for the paper appearance preferences. Settings
  // dispatches `PAPER_PREFERENCES_EVENT` whenever it mutates a preference, so
  // the shell mirror below stays in sync — toggling theme from either place
  // is visible everywhere.
  const [theme, setTheme] = useState<'light' | 'dark'>(
    () => readPaperPreferences().theme,
  )
  const [paletteOpen, setPaletteOpen] = useState(false)
  // The status-bar source picker reads + writes the global profile-scope
  // context, so selecting "Chrome · Default" actually filters every route
  // that consumes `useProfileScope().activeProfileId` (Explorer queries,
  // Intelligence aggregations, Dashboard cards). Setting it to null
  // restores the all-profiles view. Previously this was a shell-local
  // `useState` no-op that the v0.2 status bar wrote to but no route ever
  // read — Codex flagged it in the review pass, deferred until Phase 5.
  const { activeProfileId, setActiveProfileId } = useProfileScope()
  const [epigraphIndex] = useState<number>(() =>
    readEpigraphIndex(EPIGRAPH_KEY, EPIGRAPH_POOL_SIZE, shellStorage()),
  )

  // Mount-pass: push the read preferences into the document so
  // <html data-theme>, fonts, density, paper-texture line up with persisted
  // state. Subsequent updates flow through PAPER_PREFERENCES_EVENT.
  useEffect(() => {
    applyPaperPreferences(null)
  }, [])

  // Listen for preference mutations dispatched by `applyPaperPreferences`
  // (Settings → Appearance, or any future caller) so the shell's theme
  // mirror stays in sync without each owner persisting separately.
  useEffect(() => {
    function handlePreferencesChange(event: Event) {
      const detail = (event as CustomEvent<PaperPreferencesEventDetail>).detail
      if (detail?.preferences) setTheme(detail.preferences.theme)
    }
    window.addEventListener(PAPER_PREFERENCES_EVENT, handlePreferencesChange)
    return () =>
      window.removeEventListener(
        PAPER_PREFERENCES_EVENT,
        handlePreferencesChange,
      )
  }, [])

  useEffect(() => {
    try {
      window.localStorage.setItem(
        SIDEBAR_KEY,
        sidebarCollapsed ? 'true' : 'false',
      )
    } catch {
      // localStorage may be unavailable.
    }
  }, [sidebarCollapsed])

  useEffect(() => {
    function handleKey(event: KeyboardEvent) {
      const meta = event.metaKey || event.ctrlKey
      if (meta && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setPaletteOpen((value) => !value)
      } else if (event.key === 'Escape' && paletteOpen) {
        setPaletteOpen(false)
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [paletteOpen])

  const handleToggleTheme = useCallback(() => {
    // Route the toggle through applyPaperPreferences so the document
    // attribute, persistence layer, and Settings appearance card all
    // synchronise via the shared event channel.
    const current = readPaperPreferences()
    applyPaperPreferences({
      ...current,
      theme: current.theme === 'dark' ? 'light' : 'dark',
    })
  }, [])

  const handleLockNow = useCallback(() => {
    void shell.lockAppSession('manual')
  }, [shell])

  const handleBackupNow = useCallback(() => {
    void shell.runBackup()
  }, [shell])

  const handleManageSources = useCallback(() => {
    void navigate('/settings#sources')
  }, [navigate])

  const handleSearchQuery = useCallback(
    async (query: string): Promise<PaletteResult[]> => {
      const trimmed = query.trim()
      // Defense-in-depth: never let an empty query reach queryHistory.
      // The backend treats q='' as "no text filter", which on a populated
      // archive forces a full relevance scan that violates the AGENTS.md
      // performance contract. PKSearchPalette also guards upstream, so
      // this branch is unreachable from current production callers.
      // Stryker disable next-line ConditionalExpression: defensive guard.
      if (!trimmed) return []
      try {
        const response = await backend.queryHistory({
          q: trimmed,
          limit: 8,
          sort: 'relevance',
        })
        const items = response.items ?? []
        return items.slice(0, 8).map((entry) => ({
          id: String(entry.id),
          title: entry.title ?? entry.url ?? '(untitled)',
          domain: entry.domain || extractDomain(entry.url),
          url: entry.url ?? '',
          visitDate: entry.visitedAt?.slice(0, 10) ?? null,
          visitTime: entry.visitedAt?.slice(11, 16) ?? null,
        }))
      } catch {
        return []
      }
    },
    [],
  )

  const handlePaletteSelect = useCallback(
    (result: PaletteResult) => {
      if (result.visitDate) {
        void navigate(
          `/explorer?date=${encodeURIComponent(result.visitDate)}&entry=${encodeURIComponent(result.id)}`,
        )
      } else {
        void navigate('/explorer')
      }
    },
    [navigate],
  )

  const sources = useMemo<PKStatusBarSource[]>(() => {
    const profiles = shell.snapshot?.browserProfiles ?? []
    if (profiles.length === 0) return []
    return profiles.map((profile) => ({
      id: profile.profileId,
      label: profile.browserName,
      profile: profile.profileName,
      color:
        SOURCE_COLORS[profile.browserFamily] ??
        SOURCE_COLORS[profile.browserName] ??
        '#8a7f70',
      // `historyBytes` is bytes on disk for the source's history database,
      // not a row count. Leaving `pages` undefined so the picker shows
      // just the byte size instead of falsely reporting "N pages".
      size: humanizeBytes(profile.historyBytes),
    }))
  }, [shell.snapshot])

  const totalPages = shell.dashboard?.totalVisits ?? null
  const totalSize =
    humanizeBytes(sumStorageBytes(shell.dashboard?.storage)) || null
  const sinceLabel = shell.dashboard?.lastSuccessfulBackupAt
    ? formatSinceLabel(shell.dashboard.lastSuccessfulBackupAt, t, language)
    : null
  const lastArchivedLabel = shell.dashboard?.lastSuccessfulBackupAt
    ? formatLastArchivedLabel(
        shell.dashboard.lastSuccessfulBackupAt,
        t,
        language,
      )
    : null
  const archiving = Boolean(shell.busyAction)
  const initialized = shell.snapshot?.archiveStatus?.initialized ?? false
  const archiveHealthy = initialized && !shell.snapshot?.archiveStatus?.warning
  const buildVersion = shell.buildInfo?.version
    ? `v${shell.buildInfo.version}`
    : null
  const buildRevision = formatBuildRevisionLabel(shell.buildInfo)
  const buildTitle = formatBuildVersionTitle(shell.buildInfo)

  return (
    <div
      className="bg-page flex h-full min-h-0 w-full text-ink"
      data-sidebar-collapsed={sidebarCollapsed ? 'true' : 'false'}
      data-testid="app-shell"
    >
      <PKSidebar
        activeId={activeScreen.id}
        collapsed={sidebarCollapsed}
        onToggleCollapse={() => setSidebarCollapsed((v) => !v)}
        theme={theme}
        onToggleTheme={handleToggleTheme}
        onLockNow={handleLockNow}
        buildVersion={buildVersion}
        buildRevision={buildRevision}
        buildTitle={buildTitle}
        archiveHealthy={archiveHealthy}
      />
      <div className="bg-paper flex min-w-0 min-h-0 flex-1 flex-col overflow-hidden">
        <PKTopbar
          screen={activeScreen}
          onOpenPalette={() => setPaletteOpen(true)}
          onBackupNow={handleBackupNow}
          backupRunning={archiving}
          archiveInitialized={initialized}
        />
        <main
          className="pk-scrollbar flex-1 min-h-0 overflow-y-auto px-7 pb-7"
          data-testid="app-scroll"
        >
          <Outlet />
        </main>
        <PKStatusBar
          archiving={archiving}
          initialized={initialized}
          totalPages={totalPages}
          totalSize={totalSize}
          sinceLabel={sinceLabel}
          lastArchivedLabel={lastArchivedLabel}
          sources={sources}
          selectedSourceId={activeProfileId}
          onSelectSource={setActiveProfileId}
          onManageSources={handleManageSources}
          epigraphIndex={epigraphIndex}
        />
      </div>

      <PKSearchPalette
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        onSearch={handleSearchQuery}
        onSelect={handlePaletteSelect}
      />

      {shell.busyAction ? (
        shell.busyOverlay?.background ? (
          <BackgroundProgress
            state={shell.busyOverlay}
            fallbackLabel={shell.busyAction}
          />
        ) : (
          <BusyOverlay
            label={shell.busyOverlay?.label ?? shell.busyAction}
            detail={shell.busyOverlay?.detail}
            progressLabel={shell.busyOverlay?.progressLabel}
            progressValue={shell.busyOverlay?.progressValue}
            steps={shell.busyOverlay?.steps}
            activeStep={shell.busyOverlay?.activeStep}
            logLines={shell.busyOverlay?.logLines}
          />
        )
      ) : null}
    </div>
  )
}
