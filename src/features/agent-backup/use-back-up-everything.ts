/**
 * "Back up everything now": browsing history through the backup runner,
 * and Agent conversations unless they are off. The backend leaves out a
 * paused or off kind (the spec: paused means scheduled and "everything"
 * backups skip it; its own "Back up now" still works) and says so.
 *
 * Responsibilities: starting both; saying, as soon as one is known to
 * have been left out, which kind was skipped because it is paused or has
 * nothing chosen to back up (with the way to its sources), once per change
 * (an off kind is no news); and saying when the conversation backup could
 * not start. Each backup tells its own end (the runner's notice, and
 * `use-backup-notices.ts`). "Nothing was backed up" is never said: one kind
 * is always on, and an on kind either runs, fails, or has no source, which
 * is said.
 */
import { toast } from 'sonner'
import { useBackupRunner } from '@/app/backup-runner'
import {
  kindSkipReason,
  kindSkipSentence,
  type KindSkip,
} from '@/app/backup-skips'
import { agentBackupClient } from '@/lib/backend-client/agent-backup'
import { dataKindsErrorCode } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { refetchAfterBackup, useAgentKind } from '@/lib/queries/agent-backup'
import { spaceBackupPath } from '@/lib/spaces'

export function useBackUpEverything() {
  const { t } = useI18n()
  const backup = useBackupRunner()
  const { row } = useAgentKind()

  const run = () => {
    // One notice per press, replaced as the answers come in.
    const id = `back-up-everything-${crypto.randomUUID()}`
    const skips: KindSkip[] = []
    let told = ''
    const tell = () => {
      const shown = skips.filter((skip) => skip.reason !== 'kind-off')
      if (shown.length === 0) return
      const lines = shown
        .map((skip) => kindSkipSentence(t, skip))
        .join(t('agentTasks.finished.sentenceJoin'))
      // An answer that changes nothing does not bring the notice back.
      if (lines === told) return
      told = lines
      const lacking = shown.find((skip) => skip.reason === 'no-source')
      const action = lacking && {
        label: t('shell.backup.skipped.chooseSources'),
        onClick: () => {
          window.location.hash = spaceBackupPath(
            lacking.kind === 'browsing' ? 'browsing' : 'agent',
          )
        },
      }
      // A way out stays long enough to be used.
      const duration = action ? 10_000 : undefined
      toast(lines, { id, action, duration })
    }
    const answered = (outcome: 'ran' | KindSkip) => {
      if (outcome !== 'ran') skips.push(outcome)
      tell()
    }

    void backup.run({ quietSkip: true }).then((outcome) => {
      const reason =
        outcome?.ok && outcome.report.dueSkipped
          ? kindSkipReason(outcome.report.reasonCode)
          : null
      // `null`: a browsing backup was already running.
      answered(reason ? { kind: 'browsing', reason } : 'ran')
    })

    if (!row || row.availability !== 'available') return
    if (row.state === 'off') {
      skips.push({ kind: 'agent-history', reason: 'kind-off' })
      return
    }
    void agentBackupClient
      .start({ type: 'kind', kind: 'agent-history' }, crypto.randomUUID(), true)
      .then((result) => {
        const mine = result.kinds.find((item) => item.kind === 'agent-history')
        const reason =
          mine?.decision === 'skipped' ? kindSkipReason(mine.reason) : null
        answered(reason ? { kind: 'agent-history', reason } : 'ran')
      })
      .catch((error) => {
        const reason = kindSkipReason(dataKindsErrorCode(error))
        if (reason) {
          answered({ kind: 'agent-history', reason })
          return
        }
        answered('ran')
        if (dataKindsErrorCode(error) === 'lock-required') return
        toast.error(t('agentBackup.started.failed'), {
          description: describeError(error, 'start_backup'),
        })
      })
      .finally(() => refetchAfterBackup())
  }

  return { run, browsingRunning: backup.running }
}
