/**
 * The single TanStack Query client and the query keys shared across screens.
 *
 * Every backend read goes through a query so results are cached, deduplicated
 * and refreshed by invalidating a key, instead of each screen keeping its own
 * copy and refetching on every mount. After a backup or import finishes, the
 * backup runner invalidates `archiveData`, which refreshes everything derived
 * from the archive in one pass.
 */
import { QueryClient } from '@tanstack/react-query'

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Archive reads change only after a backup/import, which invalidates
      // explicitly. Refetching on focus would just hammer the backend.
      staleTime: 5 * 60_000,
      gcTime: 15 * 60_000,
      refetchOnWindowFocus: false,
      retry: false,
    },
  },
})

export const queryKeys = {
  snapshot: ['snapshot'] as const,
  buildInfo: ['build-info'] as const,
  lockStatus: ['lock-status'] as const,
  /** Prefix for every query whose result depends on archive contents. */
  archiveData: ['archive'] as const,
  dashboard: ['archive', 'dashboard'] as const,
  /** The zone Insights put days in; a backup can finish a rebuild in a new one. */
  insightsZone: ['archive', 'insights-zone'] as const,
  schedule: ['schedule-status'] as const,
  exportProgress: (exportId: string) => ['export-progress', exportId] as const,
  /** Each kind of data's state and the count the archive holds; a backup changes the count. */
  dataKinds: ['archive', 'data-kinds'] as const,
  /** The Agent tools' switches and reading `.git`, stored on this device. */
  agentSettings: ['agent-settings'] as const,
  /** Which source folders exist on this computer, per kind of data. */
  dataSources: (kinds: readonly string[]) =>
    ['data-sources', ...kinds] as const,
  /** The kinds again with their sizes in the archive (Manage data types). */
  dataKindSizes: ['archive', 'data-kinds', 'sizes'] as const,
  /** Prefix for what the Agent backup page reads: overview, sources, runs. */
  agentBackup: ['archive', 'agent'] as const,
  agentOverview: (purpose: 'backup' | 'space') =>
    ['archive', 'agent', 'overview', purpose] as const,
  agentSources: ['archive', 'agent', 'sources'] as const,
  /** Run pages are bound to the stored choices; a change starts from the first page. */
  agentRuns: (policyToken: string) =>
    ['archive', 'agent', 'runs', policyToken] as const,
  /** One run's detail, Agent or browsing, by its public reference. */
  archiveRun: (runRef: string) => ['archive', 'run', runRef] as const,
  /** The first save the enable dialog started in this session; cleared by the lock. */
  firstSave: ['first-save'] as const,
  /** Prefix for background tasks that live in the backend (conversation backups). */
  kindTasks: ['kind-tasks'] as const,
  taskSummary: ['kind-tasks', 'summary'] as const,
  taskGroup: (group: string) => ['kind-tasks', 'group', group] as const,
}
