/**
 * Format versions PathKeep cannot read yet, on the Agent backup page: one
 * card per backup that met them first, saying which tool and version, that
 * the files are saved as they are, and that an update will read them, with
 * Dismiss; and the same list with Dismiss in that backup's details.
 * Dismissed, a version stays quiet for good; a newer version raises a card
 * of its own.
 *
 * Responsibilities: finding the backups that still carry a notice (the
 * "needs a look" tasks) and dismissing one.
 * Not responsible for: the words (`words.ts`) or what Dismiss does
 * (`use-task-control.ts`).
 */
import { Info } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { FormatNotice, KindTask } from '@/lib/backend-client/agent-backup'
import { useI18n } from '@/lib/i18n'
import { useKindTasks } from '@/lib/queries/agent-backup'
import { FormatNotices } from './run-entries'
import { useTaskControl } from './use-task-control'
import { formatNoticeText, taskName } from './words'

/** Conversation backups whose format notices nobody has dismissed. */
function useFormatNoticeTasks(): KindTask[] {
  const attention = useKindTasks('attention')
  return (attention.data?.pages.flatMap((page) => page.items) ?? []).filter(
    (task) =>
      task.kind === 'agent-history' && (task.formatNotices?.length ?? 0) > 0,
  )
}

function DismissButton({ task }: { task: KindTask }) {
  const { t } = useI18n()
  const control = useTaskControl()
  if (!task.actions.includes('dismiss')) return null
  return (
    <Button
      size="sm"
      variant="outline"
      className="w-fit"
      aria-label={t('agentTasks.actions.dismissLabel', {
        name: taskName(t, task),
      })}
      disabled={control.busy !== null}
      onClick={() => void control.send(task, 'dismiss')}
    >
      {t('agentTasks.actions.dismiss')}
    </Button>
  )
}

/** In a backup's details: its notices still waiting, and Dismiss. */
export function RunFormatNotices({
  runRef,
  notices,
}: {
  runRef: string
  notices: readonly FormatNotice[]
}) {
  const task = useFormatNoticeTasks().find((item) => item.runRef === runRef)
  return (
    <>
      <FormatNotices notices={notices} />
      {task && <DismissButton task={task} />}
    </>
  )
}

export function FormatNoticeCards() {
  const { t } = useI18n()
  const tasks = useFormatNoticeTasks()
  if (tasks.length === 0) return null
  return (
    <>
      {tasks.map((task) => (
        <div
          key={task.taskId}
          role="status"
          data-format-notice-task={task.taskId}
          className="flex flex-wrap items-start gap-3 rounded-xl border bg-card px-5 py-3.5 text-[13px] duration-200 animate-in fade-in-0 slide-in-from-top-1"
        >
          <Info
            aria-hidden
            className="mt-0.5 size-4 shrink-0 text-muted-foreground"
          />
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            {task.formatNotices!.map((notice) => (
              <span
                key={`${notice.tool}-${notice.version}`}
                data-format-notice={`${notice.tool}@${notice.version}`}
              >
                {formatNoticeText(t, notice)}
              </span>
            ))}
          </div>
          <DismissButton task={task} />
        </div>
      ))}
    </>
  )
}
