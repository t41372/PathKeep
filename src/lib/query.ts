/**
 * The single TanStack Query client and the query keys shared across screens.
 *
 * Every backend read goes through a query so results are cached, deduplicated
 * and refreshed by invalidating a key, instead of each screen keeping its own
 * copy and refetching on every mount. After a backup or import finishes, the
 * backup runner invalidates `archiveData`, which refreshes everything derived
 * from the archive in one pass. A read that meets a damaged store asks for
 * the kinds list again (`noticeStoreDamage`).
 */
import {
  MutationCache,
  QueryCache,
  QueryClient,
  type QueryKey,
} from '@tanstack/react-query'
import { isStoreDamage } from './backend-client/data-kinds'

let kindsAskedAgain = false

/**
 * A kind's store that goes bad while the window is open: the read or the
 * change that met it fails with a damage code, and the kinds list (kept
 * until something changes it) is read again, so the screens show the damage
 * instead of a failing card. The backend also announces such a change (the
 * kinds-changed event); this catches what that misses. Once per tick, and
 * never for the list itself, which would ask for itself again. Queries and
 * mutations come here on their own; a command called directly (a password
 * or encryption change) calls this from its own failure.
 */
export function noticeStoreDamage(error: unknown, key?: QueryKey) {
  if (!isStoreDamage(error)) return
  if (key?.[0] === 'archive' && key[1] === 'data-kinds') return
  if (kindsAskedAgain) return
  kindsAskedAgain = true
  setTimeout(() => {
    kindsAskedAgain = false
    void queryClient.invalidateQueries({ queryKey: queryKeys.dataKinds })
  }, 0)
}

export const queryClient = new QueryClient({
  queryCache: new QueryCache({
    onError: (error, query) => noticeStoreDamage(error, query.queryKey),
  }),
  mutationCache: new MutationCache({
    onError: (error) => noticeStoreDamage(error),
  }),
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

/**
 * Applies `patch` to each cached answer under `queryKey` that is still
 * current, so a live event or an action's answer shows at once.
 *
 * An answer marked to be read again that no screen shows is left alone.
 * Writing to it would mark it fresh, and the read its screen makes when it
 * opens would not happen: the screen would open on what the backend has
 * since changed (a build that moved from Waiting to Running, still under
 * Waiting). An answer on screen is being read again already; patching it
 * keeps it live until the new one lands. `patch` returns `undefined` to
 * leave an answer as it is.
 */
export function patchQueries<T>(
  queryKey: QueryKey,
  patch: (data: T) => T | undefined,
) {
  for (const query of queryClient.getQueryCache().findAll({ queryKey })) {
    if (query.state.isInvalidated && !query.isActive()) continue
    queryClient.setQueryData<T>(query.queryKey, (data) =>
      data === undefined ? undefined : patch(data),
    )
  }
}

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
  /** Every cached run's details, whichever ledger. */
  archiveRuns: ['archive', 'run'] as const,
  /** The first save the enable dialog started in this session; cleared by the lock. */
  firstSave: ['first-save'] as const,
  /**
   * Prefix for the conversation list and reader. A changed conversation or a
   * finished backup refetches everything under it.
   */
  agentChats: ['archive', 'agent-chats'] as const,
  agentChatList: (query: object) =>
    ['archive', 'agent-chats', 'list', query] as const,
  agentChatCounts: (query: object) =>
    ['archive', 'agent-chats', 'counts', query] as const,
  /** Where one record of a line sits in the conversation (find, search hits). */
  agentChatRank: (
    id: string,
    publication: string,
    line: string,
    sourceId: string,
    ordinal: number,
  ) =>
    [
      'archive',
      'agent-chats',
      'chat',
      id,
      'rank',
      publication,
      line,
      sourceId,
      ordinal,
    ] as const,
  /**
   * Prefix for the Agent space's projects: cards, the arrangement, one
   * project and the folder suggestions. Any project edit refetches all.
   */
  agentProjects: ['archive', 'agent-projects'] as const,
  agentProjectList: (query: object) =>
    ['archive', 'agent-projects', 'list', query] as const,
  agentProject: (id: string) =>
    ['archive', 'agent-projects', 'project', id] as const,
  agentProjectSuggestions: (into: string | null) =>
    ['archive', 'agent-projects', 'suggestions', into] as const,
  agentChat: (id: string) => ['archive', 'agent-chats', 'chat', id] as const,
  agentChatInfo: (id: string) =>
    ['archive', 'agent-chats', 'chat', id, 'info'] as const,
  agentChatChildren: (id: string, publication: string) =>
    ['archive', 'agent-chats', 'chat', id, 'children', publication] as const,
  agentChatVersions: (id: string, publication: string, ordinal: number) =>
    [
      'archive',
      'agent-chats',
      'chat',
      id,
      'versions',
      publication,
      ordinal,
    ] as const,
  /** A fixed-size window of one view of one line, by its index. */
  agentChatWindow: (
    id: string,
    publication: string,
    view: string,
    line: string,
    index: number,
  ) =>
    [
      'archive',
      'agent-chats',
      'chat',
      id,
      'window',
      publication,
      view,
      line,
      index,
    ] as const,
  /** Prefix for background tasks that live in the backend (conversation backups). */
  kindTasks: ['kind-tasks'] as const,
  taskSummary: ['kind-tasks', 'summary'] as const,
  taskGroup: (group: string) => ['kind-tasks', 'group', group] as const,
}
