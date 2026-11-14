/**
 * 自動備份: how often the native scheduler runs PathKeep, when it runs next,
 * and whether the scheduler is healthy. Choosing an interval, turning it off
 * or fixing a problem opens a preview first; 詳情 shows what PathKeep
 * verified. Mirrors `src/features/backup/schedule-card.tsx`, with one design
 * change: it lives in Settings › 排程與背景工作 and says that one schedule
 * backs up every type that is on.
 */
import { ChevronRight, TriangleAlert } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'
import type { ScheduleStatus } from '@/lib/types'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { dayAndTime, formatInterval } from './format'
import { ManualSteps } from './plan-view'
import { ScheduleDetailsSheet } from './schedule-details-sheet'
import { ScheduleDialog, type ScheduleIntent } from './schedule-dialog'
import { recheck, useScheduleStatus } from './schedule-store'

type Frequency = 'hourly' | 'sixHours' | 'daily' | 'off'
type Choice = Frequency | 'custom'

const frequencyHours: Record<Exclude<Frequency, 'off'>, number> = { hourly: 1, sixHours: 6, daily: 24 }
const choiceLabels: Record<Choice, string> = { hourly: '每小時', sixHours: '每 6 小時', daily: '每天', custom: '自訂', off: '關閉' }
const platformNames: Record<string, string> = { macos: 'launchd', windows: 'Task Scheduler', linux: 'systemd' }
const stateLabels: Record<string, string> = {
  installed: '已安裝',
  'not-installed': '未安裝',
  mismatch: '需要修復',
  'permission-warning': '需要授權',
  'legacy-install-detected': '發現舊版安裝',
  'manual-review': '需手動設定',
}

function frequencyForHours(hours: number) {
  const match = Object.entries(frequencyHours).find(([, value]) => value === hours)
  return (match?.[0] as Exclude<Frequency, 'off'> | undefined) ?? null
}

/** `needsSchedulerReview` (`src/lib/platform-guidance.ts`). */
const needsReview = (s: ScheduleStatus) => ['mismatch', 'permission-warning', 'legacy-install-detected', 'manual-review'].includes(s.installState)

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 text-[13px]">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-right font-mono text-[13px]">{children}</span>
    </div>
  )
}

const segmentClass =
  'h-8 flex-1 rounded-md px-2 text-[13px] font-normal text-muted-foreground hover:bg-transparent data-[state=on]:bg-card data-[state=on]:text-foreground data-[state=on]:shadow-card'

export function ScheduleCard() {
  const status = useScheduleStatus()
  const [intent, setIntent] = useState<ScheduleIntent | null>(null)
  const [details, setDetails] = useState(false)
  const [checking, setChecking] = useState(false)
  useEffect(() => {
    if (!checking) return
    const timer = window.setTimeout(() => {
      recheck()
      setChecking(false)
    }, 900)
    return () => window.clearTimeout(timer)
  }, [checking])

  const applySupported = status.applySupported
  const active = status.installState === 'installed' || needsReview(status)
  const savedHours = status.dueAfterHours
  const preset = frequencyForHours(savedHours)
  const value: Choice = applySupported && !active ? 'off' : (preset ?? 'custom')
  const next = status.installState === 'installed' && status.nextScheduledBackupAt ? Date.parse(status.nextScheduledBackupAt) : null
  const changeMode = applySupported && active ? 'update' : 'install'

  function choose(choice: Choice) {
    if (choice === value) return
    if (choice === 'off') setIntent({ kind: 'off' })
    else if (choice === 'custom') setIntent({ kind: 'custom', hours: savedHours, mode: changeMode })
    else setIntent({ kind: 'set', hours: frequencyHours[choice], mode: changeMode })
  }

  const choices: Choice[] = applySupported ? ['hourly', 'sixHours', 'daily', 'custom', 'off'] : ['hourly', 'sixHours', 'daily', 'custom']

  return (
    <SectionCard
      title="自動備份"
      subtitle={
        <>
          在背景執行，即使 PathKeep 沒有開啟也會繼續。
          {/* Design change: the one added sentence. */}
          一個排程備份所有「開啟」的類型。
        </>
      }
    >
      <ToggleGroup
        type="single"
        spacing={1}
        value={value}
        onValueChange={(next) => next && choose(next as Choice)}
        aria-label="備份頻率"
        className="w-full rounded-lg bg-muted p-[3px]"
      >
        {choices.map((choice) => (
          <ToggleGroupItem key={choice} value={choice} className={segmentClass}>
            {choiceLabels[choice]}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
      {value === 'custom' && (
        <p className="flex items-center justify-between gap-3 text-[13px] text-muted-foreground">
          每 {formatInterval(savedHours)} 備份一次。
          <Button size="xs" variant="ghost" onClick={() => setIntent({ kind: 'custom', hours: savedHours, mode: changeMode })}>
            變更
          </Button>
        </p>
      )}
      <div className="flex flex-col gap-2.5">
        <Row label="下次執行">{next ? dayAndTime(next) : '—'}</Row>
        {active && <Row label="上次自動備份">{status.lastScheduledSuccessAt ? dayAndTime(Date.parse(status.lastScheduledSuccessAt)) : '還沒有'}</Row>}
        <Row label="排程器">
          {platformNames[status.platform] ?? 'launchd'} · {stateLabels[status.installState] ?? stateLabels['manual-review']}
        </Row>
      </div>
      {applySupported && needsReview(status) && (
        <Attention
          status={status}
          onFix={() => setIntent(status.installState === 'legacy-install-detected' ? { kind: 'legacy' } : { kind: 'set', hours: savedHours, mode: 'reinstall' })}
        />
      )}
      {!applySupported && (
        <div className="flex flex-col gap-2 rounded-lg bg-muted p-3 text-[13px]">
          <span className="font-medium">需要你手動設定</span>
          <span className="text-muted-foreground">PathKeep 無法在這個系統上自動安裝排程器。請新增一個依下列步驟執行的工作：</span>
          <ManualSteps details={status.manualStepDetails} steps={status.manualSteps} />
        </div>
      )}
      <Button size="sm" variant="ghost" className="-ml-2 w-fit text-muted-foreground" onClick={() => setDetails(true)}>
        詳情
        <ChevronRight />
      </Button>
      <ScheduleDialog intent={intent} status={status} onClose={() => setIntent(null)} />
      <ScheduleDetailsSheet open={details} status={status} checking={checking} onCheck={() => setChecking(true)} onClose={() => setDetails(false)} />
    </SectionCard>
  )
}

function Attention({ status, onFix }: { status: ScheduleStatus; onFix: () => void }) {
  const body =
    status.installState === 'legacy-install-detected'
      ? '舊版 PathKeep 留下了一個舊的自動備份工作。'
      : status.installState === 'permission-warning'
        ? '系統還沒有允許這個排程工作執行，備份可能不會準時執行。'
        : '已安裝的工作與 PathKeep 的設定不一致，備份可能不會準時執行。'
  return (
    <div className="flex items-start gap-3 rounded-lg bg-brand-soft p-3">
      <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5 text-[13px]">
        <span className="font-medium">排程器需要處理</span>
        <span className="text-muted-foreground">{body}</span>
      </div>
      <Button size="sm" variant="outline" onClick={onFix}>
        修復
      </Button>
    </div>
  )
}
