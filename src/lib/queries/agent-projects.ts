/**
 * Queries and edits for the Agent space's projects: every project's card
 * and the arrangement, one project with its folders, the folder
 * suggestions, and the person's edits with their Undo.
 *
 * Responsibilities: one cached copy of each answer, read only while the
 * kind is readable; the `version` every edit names (the newest one any
 * answer carried); after an edit, the projects and the conversation lists
 * read again (names and memberships show there); refetching on the
 * backend's change events. Not responsible for: any copy or notice.
 */
import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { useCallback, useEffect } from 'react'
import {
  AGENT_PROJECTS_CHANGED_EVENT,
  agentProjectsClient,
  type ProjectAction,
  type ProjectQuery,
} from '@/lib/backend-client/agent-projects'
import { AGENT_CONVERSATIONS_CHANGED_EVENT } from '@/lib/backend-client/agent-chats'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { listenDesktopEvent } from '@/lib/ipc/events'
import { queryClient, queryKeys } from '@/lib/query'
import { useAgentKind } from './agent-backup'

/** The newest project version an answer carried; edits are made against it. */
let knownVersion: number | null = null

function noteVersion<T extends { version: number }>(answer: T) {
  if (knownVersion === null || answer.version > knownVersion)
    knownVersion = answer.version
  return answer
}

/**
 * Projects for `query`. While another query loads, the answer before stays
 * on screen, unless `keepPrevious` is false: a list whose rows mean
 * something else for another query (Add folder's candidates).
 */
export function useProjectList(
  query: ProjectQuery,
  enabled = true,
  keepPrevious = true,
) {
  const { readable } = useAgentKind()
  return useQuery({
    queryKey: queryKeys.agentProjectList(query),
    queryFn: async () => noteVersion(await agentProjectsClient.list(query)),
    enabled: readable && enabled,
    placeholderData: keepPrevious ? keepPreviousData : undefined,
    staleTime: Infinity,
  })
}

export function useProjectDetail(id: string | null) {
  const { readable } = useAgentKind()
  return useQuery({
    queryKey: queryKeys.agentProject(id ?? ''),
    queryFn: async () => noteVersion(await agentProjectsClient.get(id!)),
    enabled: readable && id !== null,
    staleTime: Infinity,
  })
}

/** Folder suggestions: for one project, or every one (`null`). */
export function useProjectSuggestions(into: string | null, enabled = true) {
  const { readable } = useAgentKind()
  return useQuery({
    queryKey: queryKeys.agentProjectSuggestions(into),
    queryFn: async () =>
      noteVersion(await agentProjectsClient.suggestions(into ?? undefined)),
    enabled: readable && enabled,
    staleTime: Infinity,
  })
}

/** Projects, and the conversation lists and headers that name them. */
function refetchAfterEdit() {
  void queryClient.invalidateQueries({ queryKey: queryKeys.agentProjects })
  for (const part of ['list', 'counts'])
    void queryClient.invalidateQueries({
      queryKey: [...queryKeys.agentChats, part],
    })
  void queryClient.invalidateQueries({
    predicate: ({ queryKey }) =>
      queryKey[1] === queryKeys.agentChats[1] &&
      queryKey[2] === 'chat' &&
      queryKey.length === 4,
  })
}

async function currentVersion() {
  if (knownVersion !== null) return knownVersion
  return noteVersion(await agentProjectsClient.list({ limit: 1 })).version
}

/**
 * Makes one project edit against the newest version known. When another
 * window edited first (`stale_cursor`), reads the version again and makes
 * it once more: the edit is the person's choice either way.
 */
export async function editProject(action: ProjectAction) {
  const send = async () =>
    noteVersion(await agentProjectsClient.edit(await currentVersion(), action))
  let result
  try {
    result = await send()
  } catch (error) {
    if (commandErrorCode(error) !== 'stale_cursor') throw error
    knownVersion = null
    result = await send()
  }
  refetchAfterEdit()
  return result
}

/**
 * Undoes the edit that produced `version`. Only that edit can be undone,
 * and only while nothing was edited after it: then `stale_cursor`.
 */
export async function undoProjectEdit(version: number) {
  const result = noteVersion(
    await agentProjectsClient.edit(version, { kind: 'undo' }),
  )
  refetchAfterEdit()
  return result
}

/** The edit functions, stable across renders. */
export function useProjectEdits() {
  const edit = useCallback((action: ProjectAction) => editProject(action), [])
  const undo = useCallback((version: number) => undoProjectEdit(version), [])
  return { edit, undo }
}

/**
 * Keeps the projects current while mounted: a project edit in any window,
 * or a backup that published conversations (new projects, new totals).
 */
export function useAgentProjectsSync() {
  useEffect(() => {
    let stopped = false
    const stops: (() => void)[] = []
    const refetch = () => {
      knownVersion = null
      void queryClient.invalidateQueries({ queryKey: queryKeys.agentProjects })
    }
    for (const event of [
      AGENT_PROJECTS_CHANGED_EVENT,
      AGENT_CONVERSATIONS_CHANGED_EVENT,
    ])
      void listenDesktopEvent(event, refetch).then((stop) => {
        if (stopped) stop()
        else stops.push(stop)
      })
    return () => {
      stopped = true
      for (const stop of stops) stop()
    }
  }, [])
}
