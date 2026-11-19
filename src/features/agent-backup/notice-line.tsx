/**
 * One format notice with its own Dismiss: which tool and version PathKeep
 * cannot read yet, that the files are saved as they are, and putting this
 * one notice away (`dismiss-notice`). Whatever else the task asks a look
 * for, such as files it could not save, stays.
 *
 * Not responsible for: what Dismiss does (`use-task-control.ts`) or which
 * notices a task has.
 */
import { Button } from '@/components/ui/button'
import type { FormatNotice, KindTask } from '@/lib/backend-client/agent-backup'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { toolName } from '@/features/data-kinds/tools'
import { actionKey, type useTaskControl } from './use-task-control'
import { formatNoticeText } from './words'

export function NoticeLine({
  notice,
  task,
  control,
  className,
}: {
  notice: FormatNotice
  /** The task to dismiss it on; without one, the notice is only read. */
  task: KindTask | undefined
  control: ReturnType<typeof useTaskControl>
  className?: string
}) {
  const { t } = useI18n()
  const dismissable = task?.actions.includes('dismiss-notice') ?? false
  return (
    <span
      data-format-notice={`${notice.tool}@${notice.version}`}
      className={cn('flex flex-wrap items-center gap-x-3 gap-y-1', className)}
    >
      <span className="min-w-0 flex-1">{formatNoticeText(t, notice)}</span>
      {dismissable && task && (
        <Button
          size="sm"
          variant="outline"
          className="h-7"
          aria-label={t('agentTasks.actions.dismissNoticeLabel', {
            tool: t(toolName(notice.tool)),
            version: notice.version,
          })}
          disabled={control.busy !== null}
          aria-busy={
            control.busy === actionKey(task, 'dismiss-notice', notice) ||
            undefined
          }
          onClick={() => void control.send(task, 'dismiss-notice', notice)}
        >
          {t('agentTasks.actions.dismiss')}
        </Button>
      )}
    </span>
  )
}
