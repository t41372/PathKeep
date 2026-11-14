/**
 * Side sheet with one run's audit detail in three tabs: 摘要 (times, counts,
 * error, a rekey's safety copy, notes on side work), 檔案 (manifest and kept
 * copies, each with copy / show in folder) and 警告 with its count. Mirrors
 * `src/features/backup/run-sheet.tsx`, loading skeleton included.
 */
import { ShieldCheck } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { bytes, dayAndTime, number, runDuration } from './format'
import { PathLine } from './path-actions'
import { describe } from './run-entries'
import { NoteList, WarningList } from './run-warnings'
import { RunIcon } from './run-icon'
import { details, runningDetail, type Artifact, type Run, type RunDetail } from './runs-data'

const knownReasons: Record<string, string> = {
  'before-rekey': '加密變更前的副本',
  'periodic-checkpoint': '瀏覽器歷史檔案的定期副本',
  'source-schema-changed': '瀏覽器格式變了，因此保留的副本',
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex flex-col gap-0.5 rounded-lg bg-muted px-3 py-2">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-lg font-semibold tabular">{number(value)}</span>
    </div>
  )
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex justify-between gap-4">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-right">{children}</dd>
    </div>
  )
}

function ArtifactBlock({ artifact }: { artifact: Artifact }) {
  const reason = artifact.reason && artifact.reason in knownReasons ? knownReasons[artifact.reason] : artifact.reason
  return (
    <li className="flex flex-col gap-1 rounded-lg bg-muted px-3 py-2">
      <span className="text-[13px] font-medium">{reason ?? '儲存的副本'}</span>
      <PathLine path={artifact.path} />
      <span className="text-xs text-muted-foreground tabular">
        {dayAndTime(artifact.createdAt)}
        {artifact.sizeBytes != null && ` · 大小 ${bytes(artifact.sizeBytes)}`}
      </span>
    </li>
  )
}

function Summary({ run, detail }: { run: Run; detail: RunDetail }) {
  const { notes } = describe(detail.warnings)
  const trigger = detail.trigger === 'manual' ? '你手動觸發' : detail.trigger === 'schedule' ? '排程器' : detail.trigger
  const safety = detail.artifacts.find((artifact) => artifact.reason === 'before-rekey')
  const error = detail.errorMessage ?? run.errorMessage
  return (
    <div className="flex flex-col gap-5">
      {run.status === 'partial' && (
        <div role="status" className="rounded-lg bg-brand-soft p-3 text-[13px]">
          <p className="font-medium">備份未完成</p>
          <p className="mt-1 text-muted-foreground">部分所選瀏覽器未能完整備份。已儲存的歷史仍會保留，原因見「警告」。</p>
        </div>
      )}
      <dl className="flex flex-col gap-2 text-[13px]">
        <Fact label="開始時間">{dayAndTime(run.startedAt)}</Fact>
        <Fact label="耗時">{runDuration(run) ?? '—'}</Fact>
        <Fact label="觸發方式">
          {trigger}
          {detail.dueOnly && <span className="block text-xs text-muted-foreground">因為到了備份時間而執行</span>}
        </Fact>
        {detail.timezone && <Fact label="時區">{detail.timezone}</Fact>}
      </dl>
      <div className="grid grid-cols-3 gap-2">
        <Stat label="新增瀏覽" value={run.newVisits} />
        <Stat label="新增頁面" value={run.newUrls} />
        <Stat label="來源" value={run.profilesProcessed} />
      </div>
      {safety && (
        <section className="flex flex-col gap-1.5 rounded-lg bg-brand-soft p-3 text-[13px]" aria-label="安全副本">
          <h3 className="flex items-center gap-1.5 font-medium">
            <ShieldCheck className="size-4 text-brand" />
            安全副本
          </h3>
          <p className="text-muted-foreground">PathKeep 保留了加密變更前的存檔。打開它需要當時的密碼；如果當時沒有加密，則不需要密碼。</p>
          <PathLine path={safety.path} />
        </section>
      )}
      {error && (
        <section className="flex flex-col gap-1.5">
          <h3 className="text-[13px] font-medium text-destructive">錯誤</h3>
          <p className="rounded-lg bg-muted p-3 font-mono text-xs break-words whitespace-pre-wrap">{error}</p>
        </section>
      )}
      {detail.manifestPath && (
        <section className="flex flex-col gap-1">
          <h3 className="text-[13px] font-medium">清單</h3>
          <PathLine path={detail.manifestPath} />
        </section>
      )}
      <NoteList entries={notes} />
    </div>
  )
}

function Files({ detail }: { detail: RunDetail }) {
  return (
    <div className="flex flex-col gap-4 text-[13px]">
      <section className="flex flex-col gap-1">
        <h3 className="font-medium">清單</h3>
        {detail.manifestPath ? (
          <>
            <PathLine path={detail.manifestPath} />
            {detail.manifestHash && (
              <span className="font-mono text-[11px] break-all text-muted-foreground">指紋 {detail.manifestHash}</span>
            )}
          </>
        ) : (
          <p className="text-muted-foreground">這次執行沒有清單。</p>
        )}
      </section>
      {detail.artifacts.length === 0 ? (
        <p className="text-muted-foreground">這次執行沒有保留其他檔案。</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {detail.artifacts.map((artifact) => (
            <ArtifactBlock key={artifact.path} artifact={artifact} />
          ))}
        </ul>
      )}
    </div>
  )
}

function Detail({ run, detail }: { run: Run; detail: RunDetail }) {
  const { warnings } = describe(detail.warnings)
  return (
    <Tabs defaultValue="summary" className="gap-4 px-4 pb-6">
      <TabsList className="w-full">
        <TabsTrigger value="summary">摘要</TabsTrigger>
        <TabsTrigger value="files">檔案</TabsTrigger>
        <TabsTrigger value="warnings">{`警告（${warnings.length}）`}</TabsTrigger>
      </TabsList>
      <TabsContent value="summary">
        <Summary run={run} detail={detail} />
      </TabsContent>
      <TabsContent value="files">
        <Files detail={detail} />
      </TabsContent>
      <TabsContent value="warnings">
        <WarningList entries={warnings} />
      </TabsContent>
    </Tabs>
  )
}

export function RunSheet({ run, onClose }: { run: Run | null; onClose: () => void }) {
  // `load_audit_run_detail` takes a moment: skeletons first, as in the app.
  const [loadedId, setLoadedId] = useState<number | null>(null)
  useEffect(() => {
    if (!run) return
    const timer = window.setTimeout(() => setLoadedId(run.id), 350)
    return () => window.clearTimeout(timer)
  }, [run])
  const detail = run && loadedId === run.id ? (run.status === 'running' ? runningDetail : details[run.id]) : undefined

  return (
    <Sheet open={run !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-[460px] overflow-y-auto sm:max-w-[460px]">
        <SheetHeader>
          <SheetTitle className="flex items-center gap-2">
            {detail && run && <RunIcon status={run.status} />}
            執行詳情
          </SheetTitle>
          <SheetDescription>這次執行做了什麼，來自稽核記錄。</SheetDescription>
        </SheetHeader>
        {run && detail ? (
          <Detail run={run} detail={detail} />
        ) : (
          <div className="flex flex-col gap-3 px-4">
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-24 w-full" />
          </div>
        )}
      </SheetContent>
    </Sheet>
  )
}
