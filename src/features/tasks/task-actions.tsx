/**
 * The buttons on a row of the background tasks page, in the order the spec
 * gives them (Try again, Dismiss; Pause, Continue, Cancel), whichever order
 * the backend listed them in. A conversation backup's buttons keep every
 * name the Settings list gave them. One press at a time on the whole page:
 * while anything is on its way, every button waits.
 *
 * Not responsible for: what a button does (`use-task-control.ts`,
 * `use-browsing-control.ts`, `useSemanticIndexControl`) or which a task
 * offers (the backend).
 */
import { Pause, Play, RotateCcw, X } from 'lucide-react'
import type { ReactNode } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import type { KindTask, TaskAction } from '@/lib/backend-client/agent-backup'
import type { SemanticTask } from '@/lib/backend-client/agent-semantic'
import type {
  BrowsingTask,
  BrowsingTaskCommand,
} from '@/lib/backend-client/background-tasks'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import type { useSemanticIndexControl } from '@/lib/queries/agent-semantic'
import {
  actionKey,
  type useTaskControl,
} from '@/features/agent-backup/use-task-control'
import { taskName } from '@/features/agent-backup/words'
import {
  browsingActionKey,
  type useBrowsingControl,
} from './use-browsing-control'

export interface Controls {
  agent: ReturnType<typeof useTaskControl>
  browsing: ReturnType<typeof useBrowsingControl>
  semantic: ReturnType<typeof useSemanticIndexControl>
  /** Something is on its way: every other button waits (`data-spaces.md` §5.2). */
  busy: boolean
}

function ActionButton({
  label,
  aria,
  icon,
  outline,
  disabled,
  pressing,
  onClick,
}: {
  label: string
  aria: string
  icon?: ReactNode
  outline?: boolean
  disabled: boolean
  pressing: boolean
  onClick: () => void
}) {
  return (
    <Button
      size="sm"
      variant={outline ? 'outline' : 'ghost'}
      className="h-7"
      aria-label={aria}
      aria-busy={pressing || undefined}
      disabled={disabled}
      onClick={onClick}
    >
      {icon}
      {label}
    </Button>
  )
}

/** The order the spec lists them in, whichever the backend sent first. */
const agentOrder: TaskAction[] = [
  'retry',
  'dismiss',
  'pause',
  'resume',
  'cancel',
]

export function AgentActions({
  task,
  controls,
}: {
  task: KindTask
  controls: Controls
}) {
  const { t } = useI18n()
  const name = taskName(t, task)
  const control = controls.agent
  const words: Partial<
    Record<TaskAction, { label: string; aria: string; icon?: ReactNode }>
  > = {
    retry: {
      label: t('agentTasks.actions.retry'),
      aria: t('agentTasks.actions.retryLabel', { name }),
      icon: <RotateCcw aria-hidden />,
    },
    dismiss: {
      label: t('agentTasks.actions.dismiss'),
      aria: t('agentTasks.actions.dismissLabel', { name }),
    },
    pause: {
      label: t('tasks.actions.pause'),
      aria: t('tasks.actions.pauseLabel', { name }),
      icon: <Pause aria-hidden />,
    },
    resume: {
      label: t('tasks.actions.resume'),
      aria: t('tasks.actions.resumeLabel', { name }),
      icon: <Play aria-hidden />,
    },
    cancel: {
      label: t('agentTasks.actions.cancel'),
      aria: t('agentTasks.actions.cancelLabel', { name }),
      icon: <X aria-hidden />,
    },
  }
  return agentOrder
    .filter((action) => task.actions.includes(action))
    .map((action) => (
      <ActionButton
        key={action}
        {...words[action]!}
        outline={action === 'retry'}
        disabled={controls.busy}
        pressing={control.busy === actionKey(task, action)}
        onClick={() => void control.send(task, action)}
      />
    ))
}

const semanticOrder: SemanticTask['actions'] = ['pause', 'resume', 'cancel']

/** The index build's buttons: its job's controls, named as a backup's are. */
export function SemanticActions({
  task,
  name,
  controls,
}: {
  task: SemanticTask
  name: string
  controls: Controls
}) {
  const { t } = useI18n()
  const control = controls.semantic
  const pressing = control.isPending ? control.variables?.action : undefined
  const icons = {
    pause: <Pause aria-hidden />,
    resume: <Play aria-hidden />,
    cancel: <X aria-hidden />,
  }
  return semanticOrder
    .filter((action) => task.actions.includes(action))
    .map((action) => (
      <ActionButton
        key={action}
        label={t(`tasks.actions.${action}`)}
        aria={t(`tasks.actions.${action}Label`, { name })}
        icon={icons[action]}
        disabled={controls.busy}
        pressing={pressing === action}
        onClick={() =>
          control.mutate(
            { action, jobId: task.jobId },
            {
              onError: (error) =>
                toast.error(t('settingsAgent.ai.index.controlFailed'), {
                  description: describeError(error),
                }),
            },
          )
        }
      />
    ))
}

const browsingOrder: BrowsingTaskCommand[] = [
  'retry_intelligence_job',
  'replay_ai_job',
  'dismiss',
  'pause_ai_job',
  'cancel_intelligence_job',
  'cancel_ai_job',
]

export function BrowsingActions({
  task,
  name,
  controls,
}: {
  task: BrowsingTask
  name: string
  controls: Controls
}) {
  const { t } = useI18n()
  // A paused or interrupted index build goes on from where it stopped.
  const resumes = task.state === 'paused' || task.state === 'stale'
  return browsingOrder
    .filter((command) => task.commands.includes(command))
    .map((command) => {
      // Dismiss reads as it does on a conversation backup's row.
      if (command === 'dismiss')
        return (
          <ActionButton
            key={command}
            label={t('agentTasks.actions.dismiss')}
            aria={t('agentTasks.actions.dismissLabel', { name })}
            disabled={controls.busy}
            pressing={
              controls.browsing.busy === browsingActionKey(task, command)
            }
            onClick={() => void controls.browsing.send(task, command)}
          />
        )
      const kind = command.startsWith('cancel')
        ? 'cancel'
        : command === 'pause_ai_job'
          ? 'pause'
          : command === 'replay_ai_job' && resumes
            ? 'resume'
            : 'retry'
      const icon = {
        cancel: <X aria-hidden />,
        pause: <Pause aria-hidden />,
        resume: <Play aria-hidden />,
        retry: <RotateCcw aria-hidden />,
      }[kind]
      return (
        <ActionButton
          key={command}
          label={t(`tasks.actions.${kind}`)}
          aria={t(`tasks.actions.${kind}Label`, { name })}
          icon={icon}
          outline={kind === 'retry'}
          disabled={controls.busy}
          pressing={controls.browsing.busy === browsingActionKey(task, command)}
          onClick={() => void controls.browsing.send(task, command)}
        />
      )
    })
}
