/**
 * A hint about Agent conversations that, once put away, stays away on this
 * computer (`dismiss_agent_hint`; the backend keeps it with this device's
 * settings, so another computer still shows it).
 *
 * Responsibilities: whether it is put away (unknown until the settings
 * answer, so nothing flashes), hiding it at once, and showing it again if
 * the backend refused.
 */
import { toast } from 'sonner'
import {
  dataKindsClient,
  dataKindsErrorCode,
  type AgentHint,
  type AgentSettings,
} from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import { useAgentSettings } from '@/lib/queries/data-kinds'

function setDismissed(hint: AgentHint, dismissed: boolean) {
  queryClient.setQueryData<AgentSettings>(queryKeys.agentSettings, (now) => {
    if (!now) return now
    const rest = (now.dismissedHints ?? []).filter((item) => item !== hint)
    return { ...now, dismissedHints: dismissed ? [...rest, hint] : rest }
  })
}

export function useAgentHint(hint: AgentHint, enabled = true) {
  const { t } = useI18n()
  const settings = useAgentSettings(enabled)
  const dismissed = settings.data
    ? (settings.data.dismissedHints ?? []).includes(hint)
    : undefined

  const dismiss = () => {
    setDismissed(hint, true)
    dataKindsClient.dismissAgentHint(hint).catch((error) => {
      setDismissed(hint, false)
      if (dataKindsErrorCode(error) === 'lock-required') return
      toast.error(t('agentBackup.reminder.dismissFailed'), {
        description: describeError(error, 'dismiss_agent_hint'),
      })
    })
  }

  return { dismissed, dismiss }
}
