/**
 * The viewer's saved Insights choices (`agent-history.md` §1.2, §1.8,
 * §1.12): the range Insights opens on, how the "≈" beside hours reads,
 * and the two cost switches the reader shares.
 *
 * Responsible for: one cached copy, changed at once when this window saves
 * and again when the backend says another window (or Settings) did.
 * Not responsible for: what changes when they change (`use-report.ts`).
 */
import { useQuery } from '@tanstack/react-query'
import { useCallback, useEffect } from 'react'
import { toast } from 'sonner'
import {
  AGENT_INSIGHTS_PREFERENCES_CHANGED_EVENT,
  agentInsightsClient,
  type InsightsPreferencePatch,
  type InsightsPreferences,
} from '@/lib/backend-client/agent-insights'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { listenDesktopEvent } from '@/lib/ipc/events'
import { queryClient } from '@/lib/query'
import { useAgentKind } from '@/lib/queries/agent-backup'

/** Under `archive`, so a lock clears it with every other answer. */
const preferencesKey = ['archive', 'agent-insights', 'preferences'] as const

export function useInsightsPreferences() {
  const { readable } = useAgentKind()
  const query = useQuery({
    queryKey: preferencesKey,
    queryFn: agentInsightsClient.preferences,
    enabled: readable,
    staleTime: Infinity,
  })
  useEffect(() => {
    let stop: (() => void) | null = null
    let gone = false
    void listenDesktopEvent<InsightsPreferences>(
      AGENT_INSIGHTS_PREFERENCES_CHANGED_EVENT,
      (next) => queryClient.setQueryData(preferencesKey, next),
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
 * Saves a change. The screen shows it at once; a refusal puts the saved
 * choices back and says so.
 */
export function useSavePreferences() {
  const { t } = useI18n()
  return useCallback(
    async (patch: InsightsPreferencePatch) => {
      const before =
        queryClient.getQueryData<InsightsPreferences>(preferencesKey)
      if (before)
        queryClient.setQueryData(preferencesKey, { ...before, ...patch })
      try {
        const saved = await agentInsightsClient.setPreferences(patch)
        queryClient.setQueryData(preferencesKey, saved)
      } catch (error) {
        if (before) queryClient.setQueryData(preferencesKey, before)
        toast.error(t('agentInsights.saveFailed'), {
          description: describeError(error),
        })
      }
    },
    [t],
  )
}
