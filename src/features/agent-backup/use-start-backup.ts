/**
 * "Back up now" for Agent conversations only (`start_backup`, kind scope).
 *
 * Responsibilities: starting it (a second request while one runs gets the
 * running one back, and says so), the notice that it started, opening the
 * enable flow when this computer has nothing approved to back up, and
 * reading the task list again so the rail and the header show it at once.
 * Not responsible for: backing up every kind (the rail's "Back up
 * everything now").
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { agentBackupClient } from '@/lib/backend-client/agent-backup'
import { dataKindsErrorCode } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { refetchAfterBackup } from '@/lib/queries/agent-backup'
import { useOpenEnableAgent } from '@/features/enable-agent/links'

export function useStartAgentBackup() {
  const { t } = useI18n()
  const openEnable = useOpenEnableAgent()
  const [starting, setStarting] = useState(false)
  const name = t('spaces.agent.name')

  const start = async () => {
    setStarting(true)
    try {
      const result = await agentBackupClient.start(
        { type: 'kind', kind: 'agent-history' },
        crypto.randomUUID(),
      )
      const decision = result.kinds.find(
        (item) => item.kind === 'agent-history',
      )?.decision
      if (decision === 'already-running')
        toast(t('agentBackup.started.already', { name }))
      else
        toast(t('agentBackup.started.title', { name }), {
          description: t('agentBackup.started.body'),
        })
    } catch (error) {
      if (dataKindsErrorCode(error) === 'enable-preview-required') {
        openEnable()
        return
      }
      if (dataKindsErrorCode(error) === 'lock-required') return
      toast.error(t('agentBackup.started.failed'), {
        description: describeError(error, 'start_backup'),
      })
    } finally {
      refetchAfterBackup()
      setStarting(false)
    }
  }

  return { starting, start }
}
