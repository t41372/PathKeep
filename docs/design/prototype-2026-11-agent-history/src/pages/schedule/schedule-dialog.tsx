/**
 * Preview → confirm → apply for every change to the automatic backup:
 * install or change an interval (preset or custom), reinstall, turn off, and
 * remove a job an earlier version left. Nothing changes while it is open.
 * Mirrors `src/features/backup/schedule-dialog.tsx`; the plan comes from the
 * fake `preview_schedule` after a short wait, as it would over IPC.
 */
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import type { ScheduleStatus } from '@/lib/types'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { formatInterval, hoursToMinutes, intervalHours, splitInterval, type IntervalUnit } from './format'
import { ManualSteps, PathLine, PlanFiles } from './plan-view'
import { planFor, type Platform } from './schedule-data'
import { applySchedule, removeSchedule, repairLegacy } from './schedule-store'

export type ScheduleIntent =
  | { kind: 'set'; hours: number; mode: 'install' | 'update' | 'reinstall' }
  | { kind: 'custom'; hours: number; mode: 'install' | 'update' }
  | { kind: 'off' }
  | { kind: 'legacy' }

const schedulerNames: Record<string, string> = { macos: 'launchd', windows: 'Task Scheduler', linux: 'systemd' }
const unitLabels: Record<IntervalUnit, string> = { minutes: '分鐘', hours: '小時', days: '天' }

export function ScheduleDialog({ intent, status, onClose }: { intent: ScheduleIntent | null; status: ScheduleStatus; onClose: () => void }) {
  return (
    <Dialog open={intent !== null} onOpenChange={(open) => !open && onClose()}>
      {intent?.kind === 'set' || intent?.kind === 'custom' ? (
        <InstallForm intent={intent} platform={status.platform as Platform} onClose={onClose} />
      ) : intent ? (
        <RemoveForm kind={intent.kind} status={status} onClose={onClose} />
      ) : null}
    </Dialog>
  )
}

function ErrorLine({ children }: { children: string }) {
  return (
    <p role="alert" className="text-[13px] [overflow-wrap:anywhere] text-destructive">
      {children}
    </p>
  )
}

function PlanSkeleton() {
  return (
    <div className="flex flex-col gap-2" aria-busy aria-label="正在準備檔案…">
      <Skeleton className="h-4 w-2/3" />
      <Skeleton className="h-40 w-full" />
    </div>
  )
}

/** Stands in for the debounced `useSchedulePlan` query: settles 250 ms after typing stops, then loads. */
function usePlanFetch(hours: number | null) {
  const [settledHours, setSettled] = useState(hours)
  const [loadedFor, setLoadedFor] = useState<number | null>(null)
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(hours), 250)
    return () => window.clearTimeout(timer)
  }, [hours])
  useEffect(() => {
    if (settledHours === null) return
    const timer = window.setTimeout(() => setLoadedFor(settledHours), 450)
    return () => window.clearTimeout(timer)
  }, [settledHours])
  return { settledHours, loaded: settledHours !== null && loadedFor === settledHours }
}

/** Stands in for a mutation: pending for a moment, then done. */
function usePending() {
  const [pending, setPending] = useState(false)
  const run = (done: () => void) => {
    setPending(true)
    window.setTimeout(() => {
      setPending(false)
      done()
    }, 900)
  }
  return { pending, run }
}

function IntervalInput({ amount, unit, onAmount, onUnit, invalid }: { amount: string; unit: IntervalUnit; onAmount: (v: string) => void; onUnit: (v: IntervalUnit) => void; invalid: boolean }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2 text-[13px]">
        <span>每隔</span>
        <Input type="number" inputMode="numeric" min={1} step={1} value={amount} onChange={(e) => onAmount(e.target.value)} aria-label="數值" aria-invalid={invalid} className="h-8 w-24 tabular" />
        <Select value={unit} onValueChange={(v) => onUnit(v as IntervalUnit)}>
          <SelectTrigger size="sm" aria-label="單位" className="w-28">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {(['minutes', 'hours', 'days'] as const).map((v) => (
              <SelectItem key={v} value={v}>
                {unitLabels[v]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {invalid && <ErrorLine>請選一個 1 分鐘到 30 天之間的整數。</ErrorLine>}
    </div>
  )
}

function InstallForm({ intent, platform, onClose }: { intent: Extract<ScheduleIntent, { kind: 'set' | 'custom' }>; platform: Platform; onClose: () => void }) {
  const initial = splitInterval(hoursToMinutes(intent.hours))
  const [amount, setAmount] = useState(String(initial.amount))
  const [unit, setUnit] = useState<IntervalUnit>(initial.unit)
  const typedHours = intent.kind === 'custom' ? intervalHours(Number(amount), unit) : intent.hours
  const { settledHours: hours, loaded } = usePlanFetch(typedHours)
  const apply = usePending()
  const settled = hours === typedHours
  const plan = hours === null ? null : planFor(platform, hours)
  const ready = hours !== null && settled && loaded

  const title = intent.mode === 'install' ? '開啟自動備份' : intent.mode === 'reinstall' ? '重新安裝自動備份' : '變更自動備份'
  const supported = plan?.applySupported ?? true
  const confirmLabel = !supported ? '儲存間隔' : intent.mode === 'install' ? '安裝' : intent.mode === 'reinstall' ? '重新安裝' : '更新'

  function confirm(value: number) {
    apply.run(() => {
      applySchedule(value)
      toast.success(supported ? `自動備份已開啟：每 ${formatInterval(value)}` : '間隔已儲存。請同步更新你安裝的檔案。')
      onClose()
    })
  }

  return (
    <DialogContent
      className="sm:max-w-xl"
      showCloseButton={!apply.pending}
      onInteractOutside={(e) => apply.pending && e.preventDefault()}
      onEscapeKeyDown={(e) => apply.pending && e.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
        <DialogDescription>
          {supported ? '這是 PathKeep 要交給系統排程器的內容。確認之前不會改動任何東西。' : 'PathKeep 無法在這個系統上安裝排程器。請先在這裡儲存間隔，再自己加入下面的檔案。'}
        </DialogDescription>
      </DialogHeader>

      <div className="flex min-w-0 flex-col gap-4">
        {intent.kind === 'custom' && <IntervalInput amount={amount} unit={unit} onAmount={setAmount} onUnit={setUnit} invalid={typedHours === null && amount !== ''} />}

        {typedHours === null ? null : ready && plan ? (
          <p className="text-[13px] text-muted-foreground">
            PathKeep 大約每 {formatInterval(typedHours)} 備份一次，在最接近這個間隔的檢查時執行。系統每 {formatInterval(plan.wakeIntervalSeconds / 3600)}{' '}
            檢查一次，登入時也會檢查，所以關機時錯過的備份會在登入後盡快補上。
          </p>
        ) : (
          <div className="flex flex-col gap-1.5 py-0.5" aria-hidden>
            <Skeleton className="h-3.5 w-full" />
            <Skeleton className="h-3.5 w-3/4" />
          </div>
        )}

        {hours === null || !plan ? null : !ready ? (
          <PlanSkeleton />
        ) : (
          <>
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-[13px]">
              <dt className="text-muted-foreground">排程器</dt>
              <dd className="font-mono text-xs leading-5 break-all">
                {schedulerNames[plan.platform] ?? plan.platform} · {plan.label}
              </dd>
            </dl>
            {plan.applySupported ? <PlanFiles plan={plan} /> : <ManualSteps details={plan.manualStepDetails} steps={plan.manualSteps} />}
          </>
        )}
      </div>

      <DialogFooter>
        <Button variant="ghost" disabled={apply.pending} onClick={onClose}>
          取消
        </Button>
        <Button disabled={!ready || apply.pending} onClick={() => hours !== null && confirm(hours)}>
          {apply.pending && <Spinner />}
          {apply.pending ? '正在處理…' : confirmLabel}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}

/** Turn off, or remove an old job: lists what goes, then removes it. */
function RemoveForm({ kind, status, onClose }: { kind: 'off' | 'legacy'; status: ScheduleStatus; onClose: () => void }) {
  const { loaded } = usePlanFetch(status.dueAfterHours)
  const action = usePending()
  const files = status.detectedFiles

  function confirm() {
    action.run(() => {
      if (kind === 'off') removeSchedule()
      else repairLegacy()
      toast.success(kind === 'off' ? '自動備份已關閉' : '排程器已修復')
      onClose()
    })
  }

  return (
    <DialogContent
      className="sm:max-w-lg"
      showCloseButton={!action.pending}
      onInteractOutside={(e) => action.pending && e.preventDefault()}
      onEscapeKeyDown={(e) => action.pending && e.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>{kind === 'off' ? '關閉自動備份？' : '移除舊的自動備份？'}</DialogTitle>
        <DialogDescription>
          {kind === 'off'
            ? 'PathKeep 會從系統排程器移除它的工作。存檔和你選的間隔都會保留，之後可以再開啟。'
            : '舊版 PathKeep 以舊名稱留下了一個工作。PathKeep 會移除它，不會動到現在的自動備份。'}
        </DialogDescription>
      </DialogHeader>
      <div className="flex min-w-0 flex-col gap-2 text-[13px]">
        {files.length > 0 ? (
          <>
            <span className="text-muted-foreground">PathKeep 會移除：</span>
            <div className="flex flex-col gap-1 rounded-lg bg-muted px-3 py-2">
              {files.map((file) => (
                <PathLine key={file} path={file} />
              ))}
            </div>
          </>
        ) : (
          <span className="text-muted-foreground">沒有找到工作檔案。PathKeep 仍會請排程器忘掉這個工作。</span>
        )}
      </div>
      <DialogFooter>
        <Button variant="ghost" disabled={action.pending} onClick={onClose}>
          取消
        </Button>
        <Button variant={kind === 'off' ? 'destructive' : 'default'} disabled={!loaded || action.pending} onClick={confirm}>
          {action.pending && <Spinner />}
          {action.pending ? '正在處理…' : kind === 'off' ? '關閉' : '移除舊工作'}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
