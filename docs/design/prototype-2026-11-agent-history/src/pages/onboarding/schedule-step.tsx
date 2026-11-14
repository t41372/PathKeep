/**
 * 排程: today's step (src/features/onboarding/schedule-step.tsx): how often to
 * back up, a warning when background backups could not open the archive, and
 * a preview of exactly what the system scheduler will get (the file can be
 * expanded), or why it cannot be installed here. §3.3 adds one sentence under
 * the title. The plan is fake; `preview` picks how preparing it goes.
 */
import { ChevronRight, TriangleAlert } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/cn'
import { ChoiceCard, ChoiceGroup, RadioMark } from './choice'
import type { Frequency } from './draft'
import { InlineError } from './inline-error'
import { StepHeading } from './steps'

/** Prototype-only: how preparing the schedule goes. */
export type PreviewCase = 'ready' | 'unsupported' | 'failed'

export const frequencyTitles: Record<Frequency, string> = { hourly: '每小時', sixHours: '每 6 小時', daily: '每天一次', off: '手動' }
const frequencyBodies: Record<Frequency, string> = {
  hourly: '幾乎不會漏掉任何記錄',
  sixHours: '次數少一些，一天仍有好幾次',
  daily: '錯過了會在登入時補上',
  off: '只在你點「立即備份」時運行',
}
const options: Frequency[] = ['hourly', 'sixHours', 'daily', 'off']
const LABEL = 'com.yi-ting.pathkeep.backup'
const wakeSeconds: Record<Frequency, number> = { hourly: 3600, sixHours: 21_600, daily: 21_600, off: 0 }

export function ScheduleStep({
  frequency,
  onChange,
  backgroundCanUnlock,
  preview,
  mac,
}: {
  frequency: Frequency
  onChange: (frequency: Frequency) => void
  backgroundCanUnlock: boolean
  preview: PreviewCase
  mac: boolean
}) {
  // Plans are prepared once per interval, as the app caches them.
  const [prepared, setPrepared] = useState<Set<string>>(new Set())
  const key = `${frequency}|${preview}|${mac}`
  const pending = !prepared.has(key)
  useEffect(() => {
    if (frequency === 'off' || !pending) return
    const timer = window.setTimeout(() => setPrepared((s) => new Set(s).add(key)), 700)
    return () => window.clearTimeout(timer)
  }, [frequency, key, pending])

  return (
    <>
      <StepHeading
        title="多久備份一次？"
        body={
          <>
            PathKeep 會安裝一個系統排程，在背景運行，不需要開著視窗。
            <br />
            一個排程，所有資料類型一起備份。
          </>
        }
      />
      <ChoiceGroup value={frequency} onValueChange={onChange} label="備份頻率">
        {options.map((option) => (
          <ChoiceCard
            key={option}
            value={option}
            leading={<RadioMark />}
            title={frequencyTitles[option]}
            body={frequencyBodies[option]}
            trailing={option === 'hourly' && <Badge className="self-center rounded-md border-0 bg-brand-soft text-[11px] text-brand">推薦</Badge>}
          />
        ))}
      </ChoiceGroup>

      {frequency !== 'off' && !backgroundCanUnlock && (
        <p className="flex animate-rise items-start gap-2.5 rounded-lg bg-brand-soft px-3.5 py-3 text-[13px]">
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />
          存檔會被加密，而密碼不在鑰匙圈裡，所以背景備份將打不開它。
        </p>
      )}

      {frequency !== 'off' &&
        (pending ? (
          <div className="flex flex-col gap-2 rounded-xl border bg-card p-4" aria-busy aria-label="正在準備排程…">
            <Skeleton className="h-4 w-40" />
            <Skeleton className="h-4 w-full" />
          </div>
        ) : preview === 'failed' ? (
          <InlineError
            title="無法準備排程。"
            detail="failed to save config: Permission denied (os error 13)"
            actions={
              <Button size="sm" variant="outline" onClick={() =>
                  setPrepared((s) => {
                    const next = new Set(s)
                    next.delete(key)
                    return next
                  })
                }>
                再試一次
              </Button>
            }
          />
        ) : preview === 'ready' ? (
          <PlanPreview frequency={frequency} mac={mac} />
        ) : (
          <div className="flex flex-col gap-0.5 rounded-xl border bg-card px-4 py-3.5 text-[13px]">
            <span className="font-medium">PathKeep 無法在這裡安裝排程</span>
            <span className="text-muted-foreground">設定完成後，「備份」頁面會列出手動設定的步驟。</span>
          </div>
        ))}
    </>
  )
}

function PlanPreview({ frequency, mac }: { frequency: Frequency; mac: boolean }) {
  const [open, setOpen] = useState(false)
  const path = mac ? `~/Library/LaunchAgents/${LABEL}.plist` : `windows/${LABEL}.task.xml`
  return (
    <Collapsible open={open} onOpenChange={setOpen} className="flex animate-rise flex-col gap-2.5 rounded-xl border bg-card px-4 py-3.5 text-[13px] shadow-card">
      <div className="flex flex-col gap-0.5">
        <span className="font-medium">將要安裝的內容</span>
        <span className="text-muted-foreground">完成設定之前不會安裝任何東西。</span>
      </div>
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5">
        <dt className="text-muted-foreground">排程</dt>
        <dd className="font-mono text-xs leading-5">
          {mac ? 'launchd' : 'Task Scheduler'} · {LABEL}
        </dd>
        <dt className="text-muted-foreground">檔案</dt>
        <dd className="font-mono text-xs leading-5 break-all">{path}</dd>
      </dl>
      <CollapsibleTrigger asChild>
        <button type="button" className="flex w-fit items-center gap-1 text-[13px] text-muted-foreground transition-colors hover:text-foreground">
          <ChevronRight className={cn('size-3.5 transition-transform duration-150', open && 'rotate-90')} />
          {open ? '收起檔案' : '查看檔案'}
        </button>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <pre className="max-h-56 overflow-auto rounded-lg bg-muted p-3 font-mono text-[11px] leading-relaxed whitespace-pre">{mac ? plist(wakeSeconds[frequency]) : taskXml(wakeSeconds[frequency])}</pre>
      </CollapsibleContent>
    </Collapsible>
  )
}

const LOGS = '/Users/you/Library/Application Support/PathKeep/schedule/logs'

function plist(seconds: number) {
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
\t<key>Label</key>
\t<string>${LABEL}</string>
\t<key>ProgramArguments</key>
\t<array>
\t\t<string>/Applications/PathKeep.app/Contents/MacOS/pathkeep</string>
\t\t<string>--worker</string>
\t\t<string>backup</string>
\t\t<string>--due-only</string>
\t</array>
\t<key>RunAtLoad</key>
\t<true/>
\t<key>StartInterval</key>
\t<integer>${seconds}</integer>
\t<key>StandardOutPath</key>
\t<string>${LOGS}/worker.stdout.log</string>
\t<key>StandardErrorPath</key>
\t<string>${LOGS}/worker.stderr.log</string>
</dict>
</plist>`
}

function taskXml(seconds: number) {
  return `<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger><Enabled>true</Enabled></LogonTrigger>
    <TimeTrigger>
      <Repetition><Interval>PT${seconds / 3600}H</Interval></Repetition>
      <StartBoundary>2026-11-13T00:00:00</StartBoundary>
      <Enabled>true</Enabled>
    </TimeTrigger>
  </Triggers>
  <Actions Context="Author">
    <Exec>
      <Command>C:\\Program Files\\PathKeep\\pathkeep.exe</Command>
      <Arguments>--worker backup --due-only</Arguments>
    </Exec>
  </Actions>
</Task>`
}
