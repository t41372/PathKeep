/**
 * The viewer's saved Insights choices (`agent-history.md` §1.2, §1.8,
 * §1.12): the range Insights opens on, how the "≈" beside hours reads,
 * and the two cost switches, which the reader's info panel follows too.
 *
 * Responsibilities: one cached copy, read only while the kind is
 * readable, and replaced when the backend says another window (or
 * Settings) changed it. Not responsible for: saving a change and saying
 * when that failed (`features/agent-insights/use-preferences.ts`), or what
 * changes when they change.
 */
import { useQuery } from '@tanstack/react-query'
import { useEffect } from 'react'
import {
  AGENT_INSIGHTS_PREFERENCES_CHANGED_EVENT,
  agentInsightsClient,
  type InsightsPreferences,
} from '@/lib/backend-client/agent-insights'
import { listenDesktopEvent } from '@/lib/ipc/events'
import { queryClient } from '@/lib/query'
import { useAgentKind } from './agent-backup'

/** Under `archive`, so a lock clears it with every other answer. */
export const insightsPreferencesKey = [
  'archive',
  'agent-insights',
  'preferences',
] as const

export function useInsightsPreferences() {
  const { readable } = useAgentKind()
  const query = useQuery({
    queryKey: insightsPreferencesKey,
    queryFn: agentInsightsClient.preferences,
    enabled: readable,
    staleTime: Infinity,
  })
  useEffect(() => {
    let stop: (() => void) | null = null
    let gone = false
    void listenDesktopEvent<InsightsPreferences>(
      AGENT_INSIGHTS_PREFERENCES_CHANGED_EVENT,
      (next) => queryClient.setQueryData(insightsPreferencesKey, next),
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
