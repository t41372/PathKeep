/**
 * 匯入: bring in a Google Takeout export. Says how to make one, then preview →
 * confirm → import, with stop. Mirrors `src/features/backup/import-card.tsx`
 * step by step, including the typed-path form a plain browser gets instead of
 * the native picker, the drop highlight, My Activity's HTML, an export with
 * nothing in it, files set aside, and a run that carried on an earlier one.
 */
import { ChevronRight, FileArchive, Loader2 } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/cn'
import { baseName, date, number } from './format'
import { TAKEOUT_PATH, type ImportProgress, type Inspection } from './import-data'
import type { useTakeoutImport } from './import-flows'
import { ImportEntries } from './run-warnings'
import { useScene } from './scene'

function DropZone({ hovering, disabled, onPath }: { hovering: boolean; disabled: boolean; onPath: (path: string) => void }) {
  const { nativeFiles } = useScene()
  const [typed, setTyped] = useState('')
  return (
    <div
      className={cn(
        'flex flex-col items-center gap-3 rounded-lg border border-dashed px-4 py-5 text-center transition-colors',
        hovering && 'border-brand bg-brand-soft',
      )}
    >
      <FileArchive className="size-5 text-muted-foreground" />
      <p className="text-[13px]">{hovering ? '放開即可檢查' : '把 Takeout 的 .zip 拖到這裡'}</p>
      {nativeFiles ? (
        <Button
          size="sm"
          variant="outline"
          disabled={disabled}
          onClick={() => {
            toast(`原型：系統的檔案選擇器「選擇 Google Takeout 的 .zip」（只列 .zip），選了 ${baseName(TAKEOUT_PATH)}`)
            onPath(TAKEOUT_PATH)
          }}
        >
          選擇檔案
        </Button>
      ) : (
        <form
          className="flex w-full gap-2"
          onSubmit={(event) => {
            event.preventDefault()
            onPath(typed)
          }}
        >
          <Input
            value={typed}
            onChange={(event) => setTyped(event.target.value)}
            placeholder="/path/to/takeout.zip"
            aria-label="Takeout .zip 或資料夾的路徑"
            className="h-8 font-mono text-xs"
          />
          <Button size="sm" variant="outline" type="submit" disabled={disabled || !typed.trim()}>
            檢查
          </Button>
        </form>
      )}
    </div>
  )
}

function TakeoutSteps() {
  const steps = [
    '打開 takeout.google.com，全部取消選取。',
    '勾選 Chrome。在它的資料選項裡可以只留「歷史記錄」。',
    '以 .zip 格式建立匯出。準備好後 Google 會寄信通知你下載。',
    '把 .zip 拖到這裡或選擇它，不用解壓縮。',
  ]
  return (
    <Collapsible className="group/steps flex flex-col gap-2">
      <CollapsibleTrigger className="flex items-center gap-1.5 self-start text-[13px] text-muted-foreground hover:text-foreground">
        <ChevronRight className="size-3.5 transition-transform group-data-[state=open]/steps:rotate-90" aria-hidden />
        怎麼從 Google Takeout 取得
      </CollapsibleTrigger>
      <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
        <div className="flex flex-col gap-2 rounded-lg bg-muted px-3.5 py-3 text-[13px]">
          <ol className="flex list-decimal flex-col gap-1 pl-4">
            {steps.map((step) => (
              <li key={step}>{step}</li>
            ))}
          </ol>
          <p className="text-muted-foreground">Takeout 裡是 Chrome 同步到你 Google 帳號的歷史記錄，可能比這台電腦上的 Chrome 記得更久。</p>
        </div>
      </CollapsibleContent>
    </Collapsible>
  )
}

const isMyActivity = (inspection: Inspection) => inspection.quarantinedFiles.some((file) => file.reasonCode?.startsWith('chrome-my-activity'))

/** When nothing was found, the headline and its hint already say this. */
const SAID_WHEN_EMPTY = ['no-importable-files'] as const

function Found({ inspection, path }: { inspection: Inspection; path: string }) {
  const nothing = inspection.candidateItems === 0
  const myActivity = isMyActivity(inspection)
  return (
    <div className="flex flex-col gap-1 rounded-lg bg-muted p-3 text-[13px]">
      <span className="truncate font-mono text-xs text-muted-foreground" title={path}>
        {baseName(path)}
      </span>
      <span className="text-base font-semibold tabular">{nothing ? '找不到瀏覽記錄' : `找到 ${number(inspection.candidateItems)} 筆瀏覽記錄`}</span>
      {!nothing && inspection.previewRangeStart && inspection.previewRangeEnd && (
        <span className="text-muted-foreground">{`從 ${date(inspection.previewRangeStart)} 到 ${date(inspection.previewRangeEnd)}`}</span>
      )}
      {inspection.quarantinedFiles.length > 0 && (
        <span className="text-muted-foreground">{`有 ${number(inspection.quarantinedFiles.length)} 個檔案無法辨識，會另外存放。`}</span>
      )}
      {myActivity && <span>這份匯出裡「我的活動」中的 Chrome 記錄是 HTML，PathKeep 讀不了。在 Takeout 裡把「我的活動」的格式改為 JSON，或改為勾選 Chrome。</span>}
      {nothing && !myActivity && <span>這個檔案裡沒有 Chrome 歷史記錄。在 Takeout 裡勾選 Chrome，以 .zip 格式匯出。</span>}
      {!nothing && <span className="text-muted-foreground">已有的瀏覽記錄會被略過。按下匯入之前不會寫入任何內容。</span>}
    </div>
  )
}

/** The bar and count while an import runs; the browser-file card's says 「{done} / {total}」 the same way. */
export function ImportingBar({ progress, label, countWithoutTotal = false }: { progress: ImportProgress | null; label: string; countWithoutTotal?: boolean }) {
  const percent = progress?.progressPercent
  const processed = progress?.processedRecords
  const total = progress?.totalRecords
  return (
    <div className="flex flex-col gap-2.5 rounded-lg bg-muted p-3 text-[13px]">
      <div className="flex items-baseline justify-between gap-3">
        <span className="flex items-center gap-2">
          <Loader2 className="size-3.5 animate-spin" aria-hidden />
          {label}
        </span>
        {processed != null && (total || countWithoutTotal) ? (
          <span className="font-mono text-xs text-muted-foreground tabular">{total ? `${number(processed)} / ${number(total)}` : number(processed)}</span>
        ) : null}
      </div>
      <div role="progressbar" aria-label={label} aria-valuenow={percent == null ? undefined : Math.round(percent)} className="h-1 overflow-hidden rounded-full bg-background/60">
        <div
          className={cn('h-full rounded-full bg-brand', percent == null ? 'w-1/3 animate-indeterminate' : 'transition-[width]')}
          style={percent == null ? undefined : { width: `${Math.min(100, Math.max(2, percent))}%` }}
        />
      </div>
    </div>
  )
}

/** Stops the running import after the batch it is writing; shared with the browser-file card. */
export function StopButton({ stopping, onStop }: { stopping: boolean; onStop: () => void }) {
  return (
    <Button size="sm" variant="outline" disabled={stopping} onClick={onStop}>
      {stopping && <Loader2 className="size-3.5 animate-spin" aria-hidden />}
      {stopping ? '正在停止…' : '停止'}
    </Button>
  )
}

export function ImportCard({ takeout }: { takeout: ReturnType<typeof useTakeoutImport> }) {
  const scene = useScene()
  const { state, inspect, confirm, stop, reset } = takeout

  return (
    <SectionCard title="匯入" subtitle="從 Google Takeout 匯出檔匯入更早的歷史記錄。">
      {state.step === 'idle' && (
        <>
          <DropZone hovering={scene.hovering} disabled={scene.running} onPath={(path) => path.trim() && inspect(path.trim())} />
          <TakeoutSteps />
          {state.error && (
            <p role="alert" className="text-[13px] break-words text-destructive">
              <strong className="font-medium">無法讀取這個檔案。</strong> {state.error}
            </p>
          )}
        </>
      )}
      {state.step === 'inspecting' && (
        <p className="flex items-center gap-2 text-[13px] text-muted-foreground">
          <Loader2 className="size-3.5 animate-spin" aria-hidden />
          {`正在讀取 ${baseName(state.path)}…`}
        </p>
      )}
      {state.step === 'preview' && (
        <>
          <Found inspection={state.inspection} path={state.path} />
          <ImportEntries recorded={state.inspection.notes} skip={state.inspection.candidateItems === 0 ? SAID_WHEN_EMPTY : undefined} />
          <div className="flex gap-2">
            {state.inspection.candidateItems > 0 && (
              <Button size="sm" disabled={scene.running} onClick={confirm}>
                匯入
              </Button>
            )}
            <Button size="sm" variant="ghost" onClick={reset}>
              取消
            </Button>
          </div>
        </>
      )}
      {state.step === 'importing' && (
        <>
          <ImportingBar progress={state.progress} label="正在匯入…" />
          <div>
            <StopButton stopping={state.stopping} onStop={stop} />
          </div>
        </>
      )}
      {state.step === 'done' && (
        <>
          <div className="flex flex-col gap-1 rounded-lg bg-muted p-3 text-[13px]">
            <span className="text-base font-semibold tabular">
              {state.result.stopped
                ? `已停止。存下了 ${number(state.result.importedItems)} 筆新的瀏覽記錄。`
                : `已匯入 ${number(state.result.importedItems)} 筆新的瀏覽記錄`}
            </span>
            <span className="text-muted-foreground">{`另有 ${number(state.result.duplicateItems)} 筆已經在存檔裡。`}</span>
            {(state.result.sameVisitItems ?? 0) > 0 && (
              <span className="text-muted-foreground">{`${number(state.result.sameVisitItems ?? 0)} 筆已經從別的來源（瀏覽器本身，或另一份匯出）進了存檔：一併保存，只算一次。`}</span>
            )}
            {state.result.continuedFrom && !state.result.stopped && (
              <span className="text-muted-foreground">{`接著完成了 ${date(state.result.continuedFrom)} 中斷的這份匯出；上面的數字包括那次已經存下的。`}</span>
            )}
            {state.result.stopped && <span>再匯入同一份匯出就能完成：會從頭再讀一遍，已經存下的很快略過，接著存剩下的。</span>}
          </div>
          <div>
            <Button size="sm" variant="outline" onClick={reset}>
              再匯入一個
            </Button>
          </div>
        </>
      )}
    </SectionCard>
  )
}
