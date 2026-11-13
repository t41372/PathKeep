/**
 * Bottom of the rail: is anything running in the background? A ring that
 * fills (or spins while the total isn't known) and a count; hovering opens a
 * small list of what is running; clicking goes to the 背景任務 page. When
 * nothing runs it stays as a quiet icon in the same place, so the rail
 * doesn't shift.
 */
import { ChevronRight, CircleAlert, HardDriveDownload, ListChecks } from 'lucide-react'
import { toast } from 'sonner'
import { useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Popover, PopoverAnchor, PopoverContent } from '@/components/ui/popover'
import { Progress } from '@/components/ui/progress'
import { cn } from '@/lib/cn'
import { useTasks, type Task } from '~/data/tasks'

function Ring({ value }: { value: number | null }) {
  const r = 8
  const c = 2 * Math.PI * r
  return (
    <svg viewBox="0 0 20 20" className={cn('size-5', value === null && 'animate-spin [animation-duration:1.1s]')} aria-hidden>
      <circle cx="10" cy="10" r={r} fill="none" strokeWidth="2.2" className="stroke-foreground/12" />
      <circle
        cx="10"
        cy="10"
        r={r}
        fill="none"
        strokeWidth="2.2"
        strokeLinecap="round"
        className="stroke-brand transition-[stroke-dashoffset] duration-500"
        strokeDasharray={c}
        strokeDashoffset={value === null ? c * 0.72 : c * (1 - value)}
        transform="rotate(-90 10 10)"
      />
    </svg>
  )
}

function Row({ task }: { task: Task }) {
  return (
    <div className="flex flex-col gap-1.5 px-3 py-2">
      <div className="flex items-baseline justify-between gap-3 text-[13px]">
        <span className="font-medium">{task.title}</span>
        <span className="shrink-0 text-[12px] text-muted-foreground tabular">{task.progress === null ? '準備中' : `${Math.round(task.progress * 100)}%`}</span>
      </div>
      {task.progress === null ? (
        <div className="relative h-1 overflow-hidden rounded-full bg-muted">
          <span className="absolute inset-y-0 w-1/3 animate-indeterminate rounded-full bg-brand" />
        </div>
      ) : (
        <Progress value={task.progress * 100} className="h-1" />
      )}
      <span className="text-[12px] text-muted-foreground">{task.detail}</span>
    </div>
  )
}

export function TaskIndicator() {
  const tasks = useTasks()
  const navigate = useNavigate()
  const [open, setOpen] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const running = tasks.filter((t) => t.state === 'running')
  const queued = tasks.filter((t) => t.state === 'queued').length
  const failed = tasks.filter((t) => t.state === 'failed').length
  const known = running.filter((t) => t.progress !== null)
  const overall = running.length === 0 ? 0 : known.length === running.length ? known.reduce((s, t) => s + (t.progress ?? 0), 0) / known.length : null
  const hover = (on: boolean) => {
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(() => setOpen(on), on ? 180 : 220)
  }
  const label = running.length ? `背景有 ${running.length} 項任務在跑` : failed ? `背景任務：${failed} 項需要處理` : '背景沒有在跑的任務'

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverAnchor asChild>
        <button
          type="button"
          aria-label={label}
          onClick={() => {
            setOpen(false)
            void navigate('/tasks')
          }}
          onMouseEnter={() => hover(true)}
          onMouseLeave={() => hover(false)}
          onFocus={() => hover(true)}
          onBlur={() => hover(false)}
          className="relative flex h-9 w-10 shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          {running.length ? <Ring value={overall} /> : <ListChecks className="size-[17px]" />}
          {running.length > 1 && (
            <span className="absolute -top-0.5 right-0.5 flex size-4 items-center justify-center rounded-full bg-foreground text-[10px] font-semibold text-background tabular">{running.length}</span>
          )}
          {failed > 0 && <span className="absolute bottom-1 left-1.5 size-2 rounded-full border border-background bg-amber-500" />}
        </button>
      </PopoverAnchor>
      <PopoverContent
        side="right"
        align="end"
        sideOffset={8}
        className="w-[300px] p-0"
        onMouseEnter={() => hover(true)}
        onMouseLeave={() => hover(false)}
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        <div className="border-b px-3 py-2 text-[12px] font-semibold text-muted-foreground">背景任務</div>
        {running.length === 0 ? (
          <p className="px-3 py-3 text-[13px] text-muted-foreground">現在沒有在跑的任務。下次排程備份在 16:00。</p>
        ) : (
          <div className="flex flex-col divide-y">
            {running.map((t) => (
              <Row key={t.id} task={t} />
            ))}
          </div>
        )}
        {(queued > 0 || failed > 0) && (
          <div className="flex flex-col gap-1 border-t px-3 py-2 text-[12px] text-muted-foreground">
            {queued > 0 && <span>還有 {queued} 項在排隊</span>}
            {failed > 0 && (
              <span className="flex items-center gap-1 text-amber-700 dark:text-amber-300">
                <CircleAlert className="size-3.5" />
                {failed} 項今天沒完成
              </span>
            )}
          </div>
        )}
        <button
          type="button"
          onClick={() => {
            setOpen(false)
            toast.success('開始備份全部類型')
          }}
          className="flex w-full items-center gap-2 border-t px-3 py-2 text-left text-[13px] hover:bg-muted"
        >
          <HardDriveDownload className="size-4 text-muted-foreground" />
          全部立即備份
        </button>
        <button
          type="button"
          onClick={() => {
            setOpen(false)
            void navigate('/tasks')
          }}
          className="flex w-full items-center justify-between border-t px-3 py-2 text-left text-[13px] hover:bg-muted"
        >
          打開背景任務
          <ChevronRight className="size-4 text-muted-foreground" />
        </button>
      </PopoverContent>
    </Popover>
  )
}
