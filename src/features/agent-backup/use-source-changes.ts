/**
 * Changing what a conversation backup reads, from the backup page: a
 * tool's switch, reading `.git`, and adding folders a preview looked at.
 *
 * Responsibilities: sending each change with the current settings token,
 * once more with a fresh one when it changed elsewhere meanwhile; adding a
 * folder by sending every approved folder plus the new ones (the list
 * replaces the set) with the preview that looked at them; telling the
 * caller when a tool has no approved folder yet and needs a look first;
 * refreshing what the change touched.
 * Not responsible for: whether a change is allowed now (the card disables
 * the switches while a backup runs: any change stops it).
 */
import { useState } from 'react'
import { toast } from 'sonner'
import {
  dataKindsClient,
  dataKindsErrorCode,
  type AgentSourcesPatch,
  type AgentToolId,
} from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'

async function token() {
  const settings = await queryClient.fetchQuery({
    queryKey: queryKeys.agentSettings,
    queryFn: dataKindsClient.agentSettings,
    staleTime: 0,
  })
  return settings.policyToken
}

/** Sends a change; refused because the token moved, sends it once more. */
export async function sendSourceChange(
  patch: Omit<AgentSourcesPatch, 'expectedToken'>,
) {
  const send = async () =>
    dataKindsClient.updateAgentSources({
      ...patch,
      expectedToken: await token(),
    })
  let saved
  try {
    saved = await send()
  } catch (error) {
    if (dataKindsErrorCode(error) !== 'policy-changed') throw error
    saved = await send()
  }
  queryClient.setQueryData(queryKeys.agentSettings, saved)
  for (const queryKey of [
    queryKeys.agentBackup,
    queryKeys.dataKinds,
    queryKeys.kindTasks,
  ])
    void queryClient.invalidateQueries({ queryKey })
  return saved
}

export type SwitchOutcome = 'saved' | 'needs-preview' | 'failed'

export function useSourceChanges() {
  const { t } = useI18n()
  const [pending, setPending] = useState<string | null>(null)

  const run = async (
    key: string,
    patch: Omit<AgentSourcesPatch, 'expectedToken'>,
  ): Promise<SwitchOutcome> => {
    setPending(key)
    try {
      await sendSourceChange(patch)
      return 'saved'
    } catch (error) {
      const code = dataKindsErrorCode(error)
      if (code === 'enable-preview-required') return 'needs-preview'
      if (code === 'source-selection-empty')
        toast.error(t('agentBackup.sources.lastSourceRefused'))
      else if (code !== 'lock-required')
        toast.error(t('agentBackup.sources.saveFailed'), {
          description: describeError(error, 'update_agent_sources'),
        })
      return 'failed'
    } finally {
      setPending(null)
    }
  }

  return {
    /** The switch whose change is on its way: a tool id or `git`. */
    pending,
    setTool: (tool: AgentToolId, on: boolean) =>
      run(tool, { toolChanges: { [tool]: on } }),
    setReadGit: (on: boolean) => run('git', { readGit: on }),
  }
}
