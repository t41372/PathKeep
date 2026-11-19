/**
 * "Back up everything now": browsing history through the backup runner,
 * and Agent conversations when they are on. A paused kind is skipped (the
 * spec: paused means scheduled and "everything" backups leave it out; its
 * own "Back up now" still works).
 *
 * Responsibilities: starting both and saying when the conversation backup
 * could not start. Each backup tells its own end (the runner's notice, and
 * `use-backup-notices.ts`).
 */
import { toast } from 'sonner'
import { useBackupRunner } from '@/app/backup-runner'
import { agentBackupClient } from '@/lib/backend-client/agent-backup'
import { dataKindsErrorCode } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { refetchAfterBackup, useAgentKind } from '@/lib/queries/agent-backup'

export function useBackUpEverything() {
  const { t } = useI18n()
  const backup = useBackupRunner()
  const { row } = useAgentKind()
  const conversations = row?.state === 'on' && row.availability === 'available'

  const run = () => {
    void backup.run()
    if (!conversations) return
    void agentBackupClient
      .start({ type: 'kind', kind: 'agent-history' }, crypto.randomUUID())
      .catch((error) => {
        if (dataKindsErrorCode(error) === 'lock-required') return
        toast.error(t('agentBackup.started.failed'), {
          description: describeError(error, 'start_backup'),
        })
      })
      .finally(() => refetchAfterBackup())
  }

  return { run, browsingRunning: backup.running }
}
