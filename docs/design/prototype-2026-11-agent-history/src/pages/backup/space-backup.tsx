/**
 * A space's own Backup page (`/agent/backup`, `/browsing/backup`): is this
 * type's data safe, and back it up now. "立即備份" here backs up only this
 * type; backing up everything lives in the switcher menu. The schedule is
 * shared by every type and set in Settings → 排程與背景工作, so this page
 * only says what it is and when it runs next, right under the header.
 */
import { ChevronDown, CircleCheck, CircleMinus, HardDriveDownload, TriangleAlert } from 'lucide-react'
import { useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { toast } from 'sonner'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { FluidPage } from '~/components/fluid-page'
import { Button } from '@/components/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { cn } from '@/lib/cn'
import { NOW } from '~/data/fixture'
import { bytes, time, when } from '~/data/format'
import { useTasks } from '~/data/tasks'
import { kindById, spaceOfPath, type KindId } from '~/kinds'
import { Progress } from '@/components/ui/progress'
import { AutoDeleteNotice, runs, seconds } from './cards'
import { AGENT_ARCHIVE_RATIO, archiveBytes, kindCounts, sourceBytesOfKind } from './sources-data'
import { SpaceSources } from './space-sources'

function Runs({ space }: { space: KindId }) {
  const [open, setOpen] = useState(false)
  const mine = runs.filter((r) => r.parts[space] || r.result === 'skipped')
  return (
    <Collapsible open={open} onOpenChange={setOpen} className="overflow-hidden rounded-xl border bg-card shadow-card">
      <CollapsibleTrigger className="group flex w-full items-center gap-3 px-5 py-3.5 text-left">
        <span className="text-sm font-semibold">最近的備份</span>
        <span className="text-[13px] text-muted-foreground">
          {when(mine[0].at)} · {mine.filter((r) => r.result === 'warn').length ? '有 1 次留了警告' : '都正常'}
        </span>
        <ChevronDown className="ml-auto size-4 text-muted-foreground transition-transform group-data-[state=open]:rotate-180" />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <ul>
          {mine.map((run) => {
            const Icon = run.result === 'ok' ? CircleCheck : run.result === 'warn' ? TriangleAlert : CircleMinus
            return (
              <li key={run.at} className="flex items-start gap-3 border-t px-5 py-3">
                <Icon
                  className={cn(
                    'mt-0.5 size-4 shrink-0',
                    run.result === 'ok' && 'text-green',
                    run.result === 'warn' && 'text-amber-600 dark:text-amber-300',
                    run.result === 'skipped' && 'text-muted-foreground',
                  )}
                />
                <div className="flex min-w-0 flex-1 flex-col gap-0.5 text-[13px]">
                  <span className="flex items-baseline gap-2">
                    <span className="font-medium">{when(run.at)}</span>
                    {run.ms > 0 && <span className="text-xs text-muted-foreground">花了 {seconds(run.ms)}</span>}
                  </span>
                  {run.parts[space] && <span className="text-xs text-muted-foreground">{run.parts[space]}</span>}
                  {run.warning && <span className="text-xs text-muted-foreground">{run.warning}</span>}
                </div>
              </li>
            )
          })}
        </ul>
      </CollapsibleContent>
    </Collapsible>
  )
}

export default function SpaceBackupPage() {
  const { pathname } = useLocation()
  const space = spaceOfPath(pathname) ?? 'browsing'
  const kind = kindById[space]
  const last = NOW - 78 * 60_000
  const schedule = { label: '每 2 小時', ms: 2 * 3600_000 }
  const running = useTasks().find((t) => t.space === space && t.state === 'running' && t.title.startsWith('備份'))
  const [hideNotice, setHideNotice] = useState(false)
  const size = archiveBytes[space]
  const source = sourceBytesOfKind[space]

  // Agent pages fill the window; browsing keeps today's centred column until it is redone.
  const Page = space === 'agent' ? FluidPage : PageScroll
  return (
    <Page>
      <PageHeader
        title="備份"
        subtitle={`${kind.label}${kindCounts[space] ? ` · ${kindCounts[space]}` : ''}。只讀來源，不改也不刪。`}
        actions={
          <>
            <span className="flex items-center gap-1.5 rounded-full border bg-card px-3 py-1 text-xs text-muted-foreground shadow-card">
              <span className="size-1.5 rounded-full bg-green" />
              上次備份：{when(last)}
            </span>
            {running ? (
              <span className="flex items-center gap-2.5 rounded-lg border bg-card px-3 py-1.5 text-[13px] shadow-card">
                正在備份 · {running.detail}
                <Progress value={(running.progress ?? 0) * 100} className="h-1.5 w-24" />
              </span>
            ) : (
              <Button onClick={() => toast.success(`開始備份「${kind.label}」`, { description: '只備份這個類型。要全部一起備份，用左下角背景任務選單裡的「全部立即備份」。' })}>
                <HardDriveDownload />
                立即備份
              </Button>
            )}
          </>
        }
      />
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl border bg-card px-5 py-3 shadow-card">
        <span className="text-sm font-semibold">自動備份</span>
        <span className="text-[13px] text-muted-foreground tabular">
          {schedule.label} · 下次 {time(last + schedule.ms)}
        </span>
        <Button variant="outline" size="sm" className="ml-auto" asChild>
          <Link to="/settings/background">更改排程</Link>
        </Button>
      </div>
      {space === 'agent' && !hideNotice && (
        <div className="relative">
          <AutoDeleteNotice />
          <button type="button" aria-label="知道了，收起" onClick={() => setHideNotice(true)} className="absolute top-3 right-3 rounded-md px-2 py-1 text-[12px] text-muted-foreground hover:bg-muted hover:text-foreground">
            知道了
          </button>
        </div>
      )}
      <SpaceSources space={space} />
      <Runs space={space} />
      <div className="flex flex-col gap-1.5 px-1 text-[13px] text-muted-foreground">
        <span>要一次備份全部類型，用左下角背景任務選單裡的「全部立即備份」。</span>
        {size !== undefined && source !== undefined && (
          <span>
            存檔裡的 {kind.label} 約 {bytes(size)}（來源 {bytes(source)}
            {space === 'agent' ? `；Agent 對話的存法目前比原始檔大，約 ${AGENT_ARCHIVE_RATIO} 倍，之後要壓縮` : ''}）。
          </span>
        )}
      </div>
    </Page>
  )
}
