/**
 * Keeps every conversation-backup answer current for the whole window, and
 * tells the person when a backup of Agent conversations ended, however it
 * was started (this page, the rail, another window, the schedule): done
 * with what was added or nothing new, done without saving everything,
 * stopped, or failed, each with the run's details a click away
 * (`data-spaces.md` §4). A done backup also says how many files left new
 * notes (notes a backup saw before are not repeated every hour) and which
 * format versions it could not read yet.
 *
 * Responsibilities: the notice. The enable dialog shows its own first
 * save's end, so a first save that ends while the dialog is open gets no
 * notice. Mounted once, by the frame.
 */
import { toast } from 'sonner'
import type { KindBackupFinishedEvent } from '@/lib/backend-client/agent-backup'
import { useI18n } from '@/lib/i18n'
import { useAgentBackupSync, wireNumber } from '@/lib/queries/agent-backup'
import { spaceBackupPath } from '@/lib/spaces'
import { formatNoticeText, runSummaryText } from './words'

function openRun(runRef: string) {
  window.location.hash = spaceBackupPath(
    'agent',
    `?run=${encodeURIComponent(runRef)}`,
  )
}

/** The enable dialog is open: it shows the first save's end itself. */
function enableDialogOpen() {
  return /[?&]enable=agent(&|$)/.test(window.location.hash)
}

export function useAgentBackupNotices() {
  const { t } = useI18n()
  useAgentBackupSync((event: KindBackupFinishedEvent) => {
    if (event.kind !== 'agent-history') return
    if (event.trigger === 'enable' && enableDialogOpen()) return
    const details = {
      label: t('agentTasks.finished.details'),
      onClick: () => openRun(event.runRef),
    }
    const did = runSummaryText(t, event.summary, t('dataKinds.listSeparator'))
    switch (event.result) {
      case 'success': {
        const fresh = wireNumber(event.newNoteCount)
        const notices = event.formatNotices ?? []
        const description = [
          did,
          ...notices.map((notice) => formatNoticeText(t, notice)),
          fresh > 0 && t('agentTasks.finished.notes', { count: fresh }),
        ]
          .filter(Boolean)
          .join(t('agentTasks.finished.sentenceJoin'))
        toast.success(t('agentTasks.finished.done'), {
          description,
          ...(notices.length > 0 || fresh > 0
            ? {
                action: details,
                duration: notices.length > 0 ? 10_000 : undefined,
              }
            : {}),
        })
        return
      }
      case 'partial':
        toast.warning(t('agentTasks.finished.partial'), {
          description: did,
          duration: 10_000,
          action: details,
        })
        return
      case 'interrupted':
        toast(t('agentTasks.finished.interrupted'), {
          description: t('agentTasks.finished.interruptedBody'),
          action: details,
        })
        return
      default:
        toast.error(t('agentTasks.finished.failed'), {
          duration: 10_000,
          action: details,
        })
    }
  })
}
