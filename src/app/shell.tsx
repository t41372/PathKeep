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
import type { HistoryQueryResponse, StorageSummary } from '@/lib/types'
import { useShellData } from './shell-data-context'
import { appScreens, readRouteHandle } from './router'

const SIDEBAR_KEY = 'pathkeep.sidebar.collapsed'
const THEME_KEY = 'pathkeep.theme'
const EPIGRAPH_KEY = 'pathkeep.epigraph'
const EPIGRAPH_POOL_SIZE = 6

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

  const matches = useMatches()
  const activeScreen =
    [...matches]
      .map((match) => readRouteHandle(match.handle))
      .find(Boolean)?.screen ?? appScreens[0]

  const [sidebarCollapsed, setSidebarCollapsed] = useState<boolean>(() =>
    readBoolean(SIDEBAR_KEY, false),
  )
  const [theme, setTheme] = useState<'light' | 'dark'>(() =>
    readTheme(THEME_KEY),
  )
  const [paletteOpen, setPaletteOpen] = useState(false)
  const [sourceFilter, setSourceFilter] = useState<string | null>(null)
  const [epigraphIndex] = useState<number>(() => readEpigraphIndex())

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
      window.localStorage.setItem(SIDEBAR_KEY, sidebarCollapsed ? 'true' : 'false')
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
      pages: Math.max(0, profile.historyBytes),
      size: humanizeBytes(profile.historyBytes),
    }))
  }, [shell.snapshot])

  const totalPages = shell.dashboard?.totalVisits ?? null
  const totalSize = humanizeBytes(sumStorageBytes(shell.dashboard?.storage)) || null
  const sinceLabel = shell.dashboard?.lastSuccessfulBackupAt
    ? formatSinceLabel(shell.dashboard.lastSuccessfulBackupAt)
    : null
  const lastArchivedLabel = shell.dashboard?.lastSuccessfulBackupAt
    ? formatLastArchivedLabel(shell.dashboard.lastSuccessfulBackupAt)
    : null
  const archiving = Boolean(shell.busyAction)
  const initialized = shell.snapshot?.archiveStatus?.initialized ?? false
  const archiveHealthy =
    initialized && !shell.snapshot?.archiveStatus?.warning
  const buildVersion = shell.buildInfo?.version
    ? `v${shell.buildInfo.version}`
    : null

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

function readBoolean(key: string, fallback: boolean): boolean {
  if (typeof window === 'undefined') return fallback
  try {
    const raw = window.localStorage.getItem(key)
    if (raw === 'true') return true
    if (raw === 'false') return false
    return fallback
  } catch {
    return fallback
  }
}

function readTheme(key: string): 'light' | 'dark' {
  if (typeof window === 'undefined') return 'light'
  try {
    const raw = window.localStorage.getItem(key)
    if (raw === 'dark') return 'dark'
    return 'light'
  } catch {
    return 'light'
  }
}

function readEpigraphIndex(): number {
  if (typeof window === 'undefined') return 0
  try {
    const today = new Date().toISOString().slice(0, 10)
    const stored = window.localStorage.getItem(EPIGRAPH_KEY)
    if (stored) {
      const [storedDate, indexString] = stored.split(':')
      if (storedDate === today) {
        const parsed = Number.parseInt(indexString ?? '', 10)
        if (!Number.isNaN(parsed)) return parsed
      }
    }
    const next = Math.floor(Math.random() * EPIGRAPH_POOL_SIZE)
    window.localStorage.setItem(EPIGRAPH_KEY, `${today}:${next}`)
    return next
  } catch {
    return 0
  }
}

function extractDomain(url: string | undefined): string {
  if (!url) return ''
  try {
    return new URL(url).hostname
  } catch {
    return ''
  }
}

function sumStorageBytes(storage: StorageSummary | undefined): number {
  if (!storage) return 0
  return (
    storage.archiveDatabaseBytes +
    storage.sourceEvidenceDatabaseBytes +
    storage.searchDatabaseBytes +
    storage.intelligenceDatabaseBytes +
    storage.manifestBytes +
    storage.snapshotBytes +
    storage.exportBytes +
    storage.stagingBytes +
    storage.quarantineBytes +
    storage.semanticSidecarBytes +
    storage.intelligenceBlobBytes
  )
}

function humanizeBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return ''
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`
}

function formatSinceLabel(isoTimestamp: string): string {
  try {
    const date = new Date(isoTimestamp)
    if (Number.isNaN(date.getTime())) return ''
    const month = date.toLocaleString('en-US', { month: 'short' })
    return `Since ${month} ${date.getFullYear()}`
  } catch {
    return ''
  }
}

function formatLastArchivedLabel(isoTimestamp: string): string {
  try {
    const date = new Date(isoTimestamp)
    if (Number.isNaN(date.getTime())) return ''
    const time = date.toLocaleTimeString([], {
      hour: '2-digit',
      minute: '2-digit',
    })
    return `Last archived ${time}`
  } catch {
    return ''
  }
}
