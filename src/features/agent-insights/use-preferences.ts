/**
 * Saving a change to the viewer's Insights choices (read with
 * `useInsightsPreferences` in `src/lib/queries/agent-insights.ts`).
 *
 * Responsible for: showing the change at once, and putting the saved
 * choices back and saying so when the backend refuses it.
 * Not responsible for: what changes when they change (`use-report.ts`).
 */
import { useCallback } from 'react'
import { toast } from 'sonner'
import {
  agentInsightsClient,
  type InsightsPreferencePatch,
  type InsightsPreferences,
} from '@/lib/backend-client/agent-insights'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryClient } from '@/lib/query'
import { insightsPreferencesKey } from '@/lib/queries/agent-insights'

export function useSavePreferences() {
  const { t } = useI18n()
  return useCallback(
    async (patch: InsightsPreferencePatch) => {
      const before = queryClient.getQueryData<InsightsPreferences>(
        insightsPreferencesKey,
      )
      if (before)
        queryClient.setQueryData(insightsPreferencesKey, {
          ...before,
          ...patch,
        })
      try {
        const saved = await agentInsightsClient.setPreferences(patch)
        queryClient.setQueryData(insightsPreferencesKey, saved)
      } catch (error) {
        if (before) queryClient.setQueryData(insightsPreferencesKey, before)
        toast.error(t('agentInsights.saveFailed'), {
          description: describeError(error),
        })
      }
    },
    [t],
  )
}
