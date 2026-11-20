/**
 * This computer's Agent preferences (`agent-history.md` §7): the reader's
 * defaults, the scratch folders and the AI access switches.
 *
 * Responsibilities: one cached copy, read while Agent conversations are on
 * or paused, replaced when the backend says another window (or Settings)
 * changed it; saving a change, shown at once and put back when the backend
 * refuses it. Not responsible for: saying the save failed (the caller
 * decides where), or what changes when they change.
 */
import { useQuery } from '@tanstack/react-query'
import { useCallback, useEffect } from 'react'
import {
  AGENT_VIEWER_PREFERENCES_CHANGED_EVENT,
  agentViewerClient,
  type AgentViewerPatch,
  type AgentViewerPreferences,
} from '@/lib/backend-client/agent-viewer'
import { listenDesktopEvent } from '@/lib/ipc/events'
import { queryClient } from '@/lib/query'
import { useAgentKind } from './agent-backup'

/** Under `archive`, so a lock clears it with every other answer (a locked app refuses the read). */
export const viewerPreferencesKey = [
  'archive',
  'agent-viewer',
  'preferences',
] as const

/** What the reader does before the preferences are read: the backend's defaults. */
export const READER_DEFAULTS = {
  readerCollapseToolCalls: true,
  readerShowThinking: true,
} as const

export function useViewerPreferences() {
  const { row } = useAgentKind()
  const query = useQuery({
    queryKey: viewerPreferencesKey,
    queryFn: agentViewerClient.preferences,
    enabled: row !== undefined && row.state !== 'off',
    staleTime: Infinity,
  })
  useEffect(() => {
    let stop: (() => void) | null = null
    let gone = false
    void listenDesktopEvent<AgentViewerPreferences>(
      AGENT_VIEWER_PREFERENCES_CHANGED_EVENT,
      (next) => queryClient.setQueryData(viewerPreferencesKey, next),
    ).then((unlisten) => {
      if (gone) unlisten()
      else stop = unlisten
    })
    return () => {
      gone = true
      stop?.()
    }
  }, [])
  return query
}

/**
 * Saves a change. The new value shows at once; a refused change puts the
 * saved preferences back and rejects, so the caller can say so.
 */
export function useSaveViewerPreferences() {
  return useCallback(async (patch: AgentViewerPatch) => {
    const before =
      queryClient.getQueryData<AgentViewerPreferences>(viewerPreferencesKey)
    if (before)
      queryClient.setQueryData(viewerPreferencesKey, { ...before, ...patch })
    try {
      const saved = await agentViewerClient.setPreferences(patch)
      queryClient.setQueryData(viewerPreferencesKey, saved)
      return saved
    } catch (error) {
      if (before) queryClient.setQueryData(viewerPreferencesKey, before)
      throw error
    }
  }, [])
}
