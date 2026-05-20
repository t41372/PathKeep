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
import { BusyOverlay } from '@/components/primitives/busy-overlay'
import {
  PKSearchPalette,
  PKSidebar,
  PKStatusBar,
  PKTopbar,
  type PaletteResult,
  type PKStatusBarSource,
} from '@/components/shell'
import { invokeCommand } from '@/lib/ipc/bridge'
import {
  formatBuildRevisionLabel,
  formatBuildVersionTitle,
} from '@/lib/build-info'
import type { HistoryQueryResponse } from '@/lib/types'
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
  readTheme,
  sumStorageBytes,
} from './shell-helpers'

const SIDEBAR_KEY = 'pathkeep.sidebar.collapsed'
const THEME_KEY = 'pathkeep.theme'
const EPIGRAPH_KEY = 'pathkeep.epigraph'

const SHELL_STORAGE: Storage | null =
  typeof window !== 'undefined' ? window.localStorage : null

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
    readBoolean(SIDEBAR_KEY, false, SHELL_STORAGE),
  )
  const [theme, setTheme] = useState<'light' | 'dark'>(() =>
    readTheme(THEME_KEY, SHELL_STORAGE),
  )
  const [paletteOpen, setPaletteOpen] = useState(false)
  const [sourceFilter, setSourceFilter] = useState<string | null>(null)
  const [epigraphIndex] = useState<number>(() =>
    readEpigraphIndex(EPIGRAPH_KEY, EPIGRAPH_POOL_SIZE, SHELL_STORAGE),
  )

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme)
    try {
      window.localStorage.setItem(THEME_KEY, theme)
    } catch {
      // localStorage may be unavailable in some test environments.
    }
  }, [theme])

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
    setTheme((prev) => (prev === 'dark' ? 'light' : 'dark'))
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
      if (!query.trim()) return []
      try {
        const response = await invokeCommand<HistoryQueryResponse>(
          'query_history',
          {
            query: {
              search: query,
              limit: 8,
              offset: 0,
            },
          },
        )
        const rows = (response as unknown as { rows?: PaletteRow[] }).rows ?? []
        return rows.slice(0, 8).map((row, index) => ({
          id: row.visit_id ?? row.url_id ?? row.url ?? String(index),
          title: row.title ?? row.url ?? '(untitled)',
          domain: extractDomain(row.url),
          url: row.url ?? '',
          visitDate: row.visited_at_iso?.slice(0, 10) ?? null,
          visitTime: row.visited_at_iso?.slice(11, 16) ?? null,
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
        archiveHealthy={archiveHealthy ?? false}
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
          selectedSourceId={sourceFilter}
          onSelectSource={setSourceFilter}
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
        <BusyOverlay
          label={shell.busyOverlay?.label ?? shell.busyAction}
          detail={shell.busyOverlay?.detail}
          progressLabel={shell.busyOverlay?.progressLabel}
          progressValue={shell.busyOverlay?.progressValue}
          steps={shell.busyOverlay?.steps}
          activeStep={shell.busyOverlay?.activeStep}
          logLines={shell.busyOverlay?.logLines}
        />
      ) : null}
    </div>
  )
}

interface PaletteRow {
  visit_id?: string
  url_id?: string
  url?: string
  title?: string
  visited_at_iso?: string
}

