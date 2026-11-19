/**
 * Format versions PathKeep cannot read yet, on the Agent backup page: one
 * card per backup that met them first, each notice saying which tool and
 * version, that the files are saved as they are, and that an update will
 * read them, with a Dismiss of its own; and the same list in that backup's
 * details. Dismissing a notice puts away only that tool and version: a
 * backup that also could not save something stays under Needs a look.
 * Dismissed, a version stays quiet for good; a newer one raises its own.
 *
 * Responsibilities: finding the backups that still carry a notice (the
 * "needs a look" tasks, or the run's own task).
 * Not responsible for: the words (`words.ts`) or what Dismiss does
 * (`use-task-control.ts`).
 */
import { Info } from 'lucide-react'
import type { FormatNotice, KindTask } from '@/lib/backend-client/agent-backup'
import { useKindTasks } from '@/lib/queries/agent-backup'
import { NoticeLine } from './notice-line'
import { useTaskControl } from './use-task-control'

/** Conversation backups whose format notices nobody has dismissed. */
function useFormatNoticeTasks(): KindTask[] {
  const attention = useKindTasks('attention')
  return (attention.data?.pages.flatMap((page) => page.items) ?? []).filter(
    (task) =>
      task.kind === 'agent-history' && (task.formatNotices?.length ?? 0) > 0,
  )
}

/** In a backup's details: its notices still waiting, each with Dismiss. */
export function RunFormatNotices({
  task,
  notices,
}: {
  /** The run's own task (`get_archive_run`), when it has one. */
  task: KindTask | undefined
  notices: readonly FormatNotice[]
}) {
  const control = useTaskControl()
  return (
    <ul className="flex flex-col gap-1.5">
      {notices.map((notice) => (
        <li
          key={`${notice.tool}-${notice.version}`}
          className="rounded-lg border border-brand/25 bg-brand-soft px-3 py-2 text-[13px]"
        >
          <NoticeLine notice={notice} task={task} control={control} />
        </li>
      ))}
    </ul>
  )
}

export function FormatNoticeCards() {
  const tasks = useFormatNoticeTasks()
  const control = useTaskControl()
  if (tasks.length === 0) return null
  return (
    <>
      {tasks.map((task) => (
        <div
          key={task.taskId}
          role="status"
          data-format-notice-task={task.taskId}
          className="flex animate-in items-start gap-3 rounded-xl border bg-card px-5 py-3.5 text-[13px] duration-200 fade-in-0 slide-in-from-top-1"
        >
          <Info
            aria-hidden
            className="mt-1 size-4 shrink-0 text-muted-foreground"
          />
          <div className="flex min-w-0 flex-1 flex-col gap-1.5">
            {task.formatNotices!.map((notice) => (
              <NoticeLine
                key={`${notice.tool}-${notice.version}`}
                notice={notice}
                task={task}
                control={control}
              />
            ))}
          </div>
        </div>
      ))}
    </>
  )
}
