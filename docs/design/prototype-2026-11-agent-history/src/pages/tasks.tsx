/**
 * 背景任務: what runs now, what waits and why, what finished or failed today.
 * Shared by every data type; each task says which one it serves. Schedules
 * and limits stay in Settings → 背景執行.
 */
import { CircleAlert, CircleCheck, Clock, HardDriveDownload, Pause, Play } from 'lucide-react'
import { toast } from 'sonner'
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { PageHeader, PageScroll, SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { time } from '~/data/format'
import { pauseTask, useTasks, type Task } from '~/data/tasks'
import { kindById } from '~/kinds'
import { SpaceTile } from '~/shell/space-rail'

function Owner({ task }: { task: Task }) {
  if (task.space === 'app') return <span className="text-[12px] text-muted-foreground">整個存檔</span>
  const kind = kindById[task.space]
  return (
    <span className="flex items-center gap-1.5 text-[12px] text-muted-foreground">
      <SpaceTile kind={kind} size="sm" />
    </span>
  )
}

function TaskRow({ task, action }: { task: Task; action?: ReactNode }) {
  return (
    <div className="flex items-center gap-3 py-2.5">
      <Owner task={task} />
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <div className="flex items-baseline justify-between gap-3">
          <span className="text-[13.5px] font-medium">{task.title}</span>
          <span className="shrink-0 text-[12px] text-muted-foreground tabular">
            {task.state === 'running' ? (task.progress === null ? '準備中' : `${Math.round(task.progress * 100)}%`) : time(task.at)}
          </span>
        </div>
        {task.state === 'running' || task.state === 'paused' ? (
          task.progress === null ? (
            <div className="relative h-1.5 overflow-hidden rounded-full bg-muted">
              <span className="absolute inset-y-0 w-1/3 animate-indeterminate rounded-full bg-brand" />
            </div>
          ) : (
            <Progress value={task.progress * 100} className="h-1.5" />
          )
        ) : null}
        <span className="text-[12px] text-muted-foreground">
          {task.detail}
          {task.note && ` · ${task.note}`}
        </span>
      </div>
      {action}
    </div>
  )
}

export default function TasksPage() {
  const tasks = useTasks()
  const running = tasks.filter((t) => t.state === 'running')
  const waiting = tasks.filter((t) => t.state === 'queued' || t.state === 'paused')
  const done = tasks.filter((t) => t.state === 'done')
  const failed = tasks.filter((t) => t.state === 'failed')
  return (
    <PageScroll>
      <PageHeader
        title="背景任務"
        subtitle={running.length ? `${running.length} 項在跑，關掉視窗也會繼續。` : '現在沒有在跑的任務。'}
        actions={
          <>
            <Button variant="outline" asChild>
              <Link to="/settings/background">排程與限制</Link>
            </Button>
            <Button onClick={() => toast.success('開始備份全部類型')}>
              <HardDriveDownload />
              全部立即備份
            </Button>
          </>
        }
      />
      {failed.length > 0 && (
        <SectionCard title={<span className="flex items-center gap-1.5"><CircleAlert className="size-4 text-amber-600" />需要你看一下</span>}>
          <div className="flex flex-col divide-y">
            {failed.map((t) => (
              <TaskRow key={t.id} task={t} action={<Button size="sm" variant="outline">再試一次</Button>} />
            ))}
          </div>
        </SectionCard>
      )}
      <SectionCard title="正在跑">
        {running.length === 0 ? (
          <p className="text-[13px] text-muted-foreground">沒有。下次排程備份在 16:00。</p>
        ) : (
          <div className="flex flex-col divide-y">
            {running.map((t) => (
              <TaskRow
                key={t.id}
                task={t}
                action={
                  <Button size="icon" variant="ghost" aria-label="暫停" onClick={() => pauseTask(t.id)}>
                    <Pause />
                  </Button>
                }
              />
            ))}
          </div>
        )}
      </SectionCard>
      {waiting.length > 0 && (
        <SectionCard title={<span className="flex items-center gap-1.5"><Clock className="size-4 text-muted-foreground" />等待中</span>}>
          <div className="flex flex-col divide-y">
            {waiting.map((t) => (
              <TaskRow
                key={t.id}
                task={t}
                action={
                  t.state === 'paused' ? (
                    <Button size="icon" variant="ghost" aria-label="繼續" onClick={() => pauseTask(t.id)}>
                      <Play />
                    </Button>
                  ) : undefined
                }
              />
            ))}
          </div>
        </SectionCard>
      )}
      <SectionCard title={<span className="flex items-center gap-1.5"><CircleCheck className="size-4 text-green" />今天完成</span>}>
        <div className="flex flex-col divide-y">
          {done.map((t) => (
            <TaskRow key={t.id} task={t} />
          ))}
        </div>
      </SectionCard>
    </PageScroll>
  )
}
