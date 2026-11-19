/**
 * The third step, "Save": the first save as it runs (one line per tool, in
 * the units the backup has at that moment, under a bar that slides because
 * no total is known) and once it ended (`Saved` with what was saved, or why
 * it stopped, with "Try again").
 *
 * Responsibilities: drawing the run and its end, and the retry. A tool
 * whose files were kept as they are (format not read yet) is part of a
 * normal save, not a failure; a folder that could not be read says why.
 * Not responsible for: starting the save (`use-start-saving.ts`), the
 * dialog's buttons (`enable-agent-dialog.tsx`).
 */
import { CircleAlert, CircleCheck, Info } from 'lucide-react'
import { useId } from 'react'
import { CardError } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import type {
  ArchiveRunDetail,
  KindTask,
  SourceAvailability,
} from '@/lib/backend-client/agent-backup'
import type { AgentToolId } from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import {
  cachedTask,
  useKindTasks,
  wireNumber,
} from '@/lib/queries/agent-backup'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import { useTaskControl } from '@/features/agent-backup/use-task-control'
import {
  availabilityText,
  isProblem,
  taskReasonText,
  toolProgressText,
} from '@/features/agent-backup/words'
import { rememberFirstSave, type ShownSave } from './first-save'
import type { useShownRun } from './use-shown-run'

type ShownRun = ReturnType<typeof useShownRun>

function RunningTools({
  tools,
  task,
}: {
  tools: readonly AgentToolId[]
  task: KindTask | undefined
}) {
  const { t } = useI18n()
  const format = useFormat()
  const listId = useId()
  return (
    <>
      <span id={listId} className="sr-only">
        {t('enableAgent.run.toolsLabel')}
      </span>
      <ul
        aria-labelledby={listId}
        className="flex flex-col divide-y rounded-lg border"
      >
        {tools.map((tool) => {
          const text = toolProgressText(
            t,
            format.bytes,
            task?.perTool?.find((item) => item.tool === tool),
          )
          return (
            <li
              key={tool}
              data-tool={tool}
              className="flex flex-col gap-2 px-3.5 py-3 text-[13px]"
            >
              <div className="flex items-center justify-between gap-3">
                <ToolBadge id={tool} />
                <span
                  aria-live="polite"
                  className="text-right text-xs text-muted-foreground tabular"
                >
                  {text}
                </span>
              </div>
              <Progress
                aria-label={text}
                className="h-1 bg-muted [&>[data-slot=progress-indicator]]:bg-brand"
              />
            </li>
          )
        })}
      </ul>
    </>
  )
}

function RunningView({ shown }: { shown: ShownRun }) {
  const { t } = useI18n()
  if (shown.tools.length === 0) {
    return (
      <div className="flex flex-col gap-2" aria-busy>
        <span className="sr-only">{t('enableAgent.run.loading')}</span>
        <Skeleton className="h-14 w-full" />
        <Skeleton className="h-14 w-full" />
      </div>
    )
  }
  return (
    <div className="flex flex-col gap-3">
      <RunningTools tools={shown.tools} task={shown.task} />
      {shown.task?.phase === 'finishing' || shown.task?.phase === 'index' ? (
        <p className="text-xs text-muted-foreground">
          {t(
            shown.task.phase === 'index'
              ? 'agentTasks.progress.indexing'
              : 'agentTasks.progress.finishing',
          )}
        </p>
      ) : null}
      <p className="text-[13px] text-muted-foreground">
        {t('enableAgent.run.background')}
      </p>
    </div>
  )
}

interface Note {
  key: string
  tool: AgentToolId
  path: string | null
  availability: SourceAvailability
}

/** The tools (or, for a tool with several folders, the folders) that need a word. */
function Notes({ detail }: { detail: ArchiveRunDetail }) {
  const { t } = useI18n()
  const notes = (detail.perTool ?? []).flatMap((tool): Note[] => {
    if (tool.availability === 'available') return []
    const roots = (detail.roots ?? []).filter(
      (root) => root.tool === tool.tool && root.availability !== 'available',
    )
    if (tool.availability === 'partial' && roots.length > 0)
      return roots.map((root) => ({
        key: root.rootId,
        tool: root.tool,
        path: root.path,
        availability: root.availability,
      }))
    return [
      {
        key: tool.tool,
        tool: tool.tool,
        path: null,
        availability: tool.availability,
      },
    ]
  })
  if (notes.length === 0) return null
  return (
    <div className="flex flex-col gap-2">
      <p className="text-[13px]">{t('enableAgent.done.notes')}</p>
      <ul className="flex flex-col divide-y rounded-lg border">
        {notes.map((note) => {
          const problem = isProblem(note.availability)
          const Icon = problem ? CircleAlert : Info
          return (
            <li
              key={note.key}
              data-tool={note.tool}
              className="flex items-start gap-3 px-3.5 py-3 text-[13px]"
            >
              <Icon
                aria-hidden
                className={cn(
                  'mt-0.5 size-4 shrink-0',
                  problem ? 'text-brand' : 'text-muted-foreground',
                )}
              />
              <span className="flex min-w-0 flex-col gap-1">
                <ToolBadge id={note.tool} />
                {note.path && (
                  <span className="font-mono text-[11.5px] [overflow-wrap:anywhere] text-muted-foreground">
                    {note.path}
                  </span>
                )}
                <span className="text-muted-foreground">
                  {availabilityText(t, note.availability)}
                </span>
              </span>
            </li>
          )
        })}
      </ul>
    </div>
  )
}

/** The ended task, for its actions: from the cache, else from today's and attention's lists. */
function useEndedTask(taskId: string) {
  const today = useKindTasks('today')
  const attention = useKindTasks('attention')
  const all = [
    ...(attention.data?.pages ?? []),
    ...(today.data?.pages ?? []),
  ].flatMap((page) => page.items)
  return all.find((task) => task.taskId === taskId) ?? cachedTask(taskId)
}

/** Runs the same save again; the dialog then follows the new run. */
function Retry({
  task,
  save,
}: {
  task: KindTask | undefined
  save: ShownSave
}) {
  const { t } = useI18n()
  const control = useTaskControl()
  if (!task?.actions.includes('retry')) return null
  return (
    <Button
      variant="outline"
      size="sm"
      className="w-fit"
      disabled={control.busy !== null}
      onClick={() =>
        void control.send(task, 'retry').then((result) => {
          if (!result?.replacementTaskId) return
          rememberFirstSave({
            taskId: result.replacementTaskId,
            runRef: result.task.runRef,
            tools: save.tools,
          })
        })
      }
    >
      {t('agentTasks.actions.retry')}
    </Button>
  )
}

function EndedView({ shown, save }: { shown: ShownRun; save: ShownSave }) {
  const { t } = useI18n()
  const format = useFormat()
  const detail = shown.detail!
  const summary = 'newRoots' in detail.summary ? detail.summary : null
  const conversations = summary
    ? wireNumber(summary.newRoots) + wireNumber(summary.updatedRoots)
    : 0
  const subagents = summary ? wireNumber(summary.newChildren) : 0
  const ended = useEndedTask(save.taskId)
  const reason = taskReasonText(t, ended?.reason)

  if (detail.result === 'failed' || detail.result === 'interrupted') {
    const failed = detail.result === 'failed'
    return (
      <div role="status" className="flex flex-col gap-3">
        <p className="text-[13px] text-muted-foreground">
          {t(
            failed
              ? 'enableAgent.done.failedBody'
              : 'enableAgent.done.interruptedBody',
          )}
          {reason && ` ${reason}`}
        </p>
        <Notes detail={detail} />
        <Retry task={ended} save={save} />
      </div>
    )
  }

  return (
    <div role="status" className="flex flex-col gap-3">
      <p className="text-[13px]">
        {conversations + subagents === 0
          ? t('enableAgent.done.nothingNew')
          : t('enableAgent.done.body', {
              conversations: t('agentTasks.progress.conversations', {
                count: conversations,
              }),
              subagents: t('agentTasks.progress.subagents', {
                count: subagents,
              }),
              size: format.bytes(wireNumber(summary?.sourceBytes)),
            })}
      </p>
      {detail.result === 'partial' && (
        <>
          <Notes detail={detail} />
          <p className="text-xs text-muted-foreground">
            {t('enableAgent.done.inTasks')}
          </p>
          <Retry task={ended} save={save} />
        </>
      )}
    </div>
  )
}

const endTitles: Record<string, MessageKey> = {
  success: 'enableAgent.done.title',
  partial: 'enableAgent.done.title',
  failed: 'enableAgent.done.failedTitle',
  interrupted: 'enableAgent.done.interruptedTitle',
}

/** The dialog's title once the save ended: how it ended, with its mark. */
export function EndedTitle({ result }: { result: string | undefined }) {
  const { t } = useI18n()
  const saved = result === 'success' || result === 'partial'
  const Icon = saved ? CircleCheck : CircleAlert
  return (
    <span className="flex items-center gap-2">
      <Icon
        aria-hidden
        className={cn('size-[18px]', saved ? 'text-green' : 'text-brand')}
      />
      {t(endTitles[result ?? ''] ?? 'enableAgent.done.failedTitle')}
    </span>
  )
}

export function SaveStep({
  shown,
  save,
}: {
  shown: ShownRun
  save: ShownSave
}) {
  if (shown.error && !shown.detail) {
    return <CardError error={shown.error} onRetry={shown.retryRead} />
  }
  if (shown.finished && shown.detail) {
    return <EndedView shown={shown} save={save} />
  }
  return <RunningView shown={shown} />
}
