/**
 * The projects the filter offers and the header names
 * (`list_agent_projects`): every one at once, newest activity first.
 *
 * Responsibilities: one cached copy, read while the filter is open or a
 * project is chosen, and again when conversations or projects change.
 */
import { useQuery } from '@tanstack/react-query'
import { useEffect } from 'react'
import { agentInsightsClient } from '@/lib/backend-client/agent-insights'
import { subscribeToAgentConversations } from '@/lib/ipc/agent-chats-events'
import { queryClient } from '@/lib/query'
import { useAgentKind } from '@/lib/queries/agent-backup'

const projectsKey = ['archive', 'agent-insights', 'projects'] as const

export function useProjectOptions(enabled: boolean) {
  const { readable } = useAgentKind()
  useEffect(() => {
    let stop: (() => void) | null = null
    let gone = false
    void subscribeToAgentConversations(
      () => void queryClient.invalidateQueries({ queryKey: projectsKey }),
    ).then((unlisten) => {
      if (gone) unlisten()
      else stop = unlisten
    })
    return () => {
      gone = true
      stop?.()
    }
  }, [])
  return useQuery({
    queryKey: projectsKey,
    queryFn: agentInsightsClient.projects,
    enabled: readable && enabled,
    staleTime: Infinity,
  })
}
