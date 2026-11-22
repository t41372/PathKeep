/**
 * 匯入瀏覽器歷史: another browser's history file or profile folder. Preview
 * first (new visits, ones already archived, Firefox Sync copies, the range
 * and the latest entries), then import exactly the new ones, with stop.
 * Mirrors `src/features/backup/browser-import-card.tsx` step by step.
 */
import { FolderOpen, History, Loader2 } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { cn } from '@/lib/cn'
import { baseName, date, dayAndTime, number } from './format'
import { EDGE_FOLDER, FIREFOX_PATH, type Inspection } from './import-data'
import { ImportingBar, StopButton } from './import-card'
import { browserSources, newVisits, syncCopies, toSave, type BrowserSource, type useBrowserImport } from './import-flows'
import { useScene } from './scene'

const sourceLabel: Record<BrowserSource, string> = {
  detect: '自動辨識',
  chrome: 'Google Chrome',
  edge: 'Microsoft Edge',
  firefox: 'Firefox',
  safari: 'Safari',
  atlas: 'ChatGPT Atlas',
  comet: 'Perplexity Comet',
}

const detectedFamilies: Record<string, 'chromium' | 'firefox' | 'safari'> = {
  'chromium-history-db': 'chromium',
  'firefox-places-db': 'firefox',
  'safari-history-db': 'safari',
}

const detectedLabel = { chromium: 'Chrome 格式的歷史', firefox: 'Firefox 歷史', safari: 'Safari 歷史' }

function Entry({ disabled, source, onSource, onPath }: { disabled: boolean; source: BrowserSource; onSource: (s: BrowserSource) => void; onPath: (path: string) => void }) {
  const { nativeFiles, hovering } = useScene()
  const [typed, setTyped] = useState('')

  function choose(directory: boolean) {
    const path = directory ? EDGE_FOLDER : FIREFOX_PATH
    toast(`原型：系統的選擇器「${directory ? '選擇瀏覽器使用者資料夾' : '選擇瀏覽器歷史檔案'}」，選了 ${baseName(path)}`)
    onPath(path)
  }

  return (
    <div className="flex flex-col gap-3">
      <div
        className={cn(
          'flex flex-col items-center gap-3 rounded-lg border border-dashed px-4 py-5 text-center transition-colors',
          hovering && 'border-brand bg-brand-soft',
        )}
      >
        <History className="size-5 text-muted-foreground" />
        <p className="text-[13px]">{hovering ? '放開即可檢查' : '把歷史檔案或使用者資料夾拖到這裡'}</p>
        {nativeFiles ? (
          <div className="flex gap-2">
            <Button size="sm" variant="outline" disabled={disabled} onClick={() => choose(false)}>
              選擇檔案
            </Button>
            <Button size="sm" variant="outline" disabled={disabled} onClick={() => choose(true)}>
              <FolderOpen />
              選擇資料夾
            </Button>
          </div>
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
              placeholder="/path/to/places.sqlite"
              aria-label="歷史檔案或使用者資料夾的路徑"
              className="h-8 font-mono text-xs"
            />
            <Button size="sm" variant="outline" type="submit" disabled={disabled || !typed.trim()}>
              檢查
            </Button>
          </form>
        )}
      </div>
      <div className="flex items-center justify-between gap-3 text-[13px]">
        <span className="text-muted-foreground">來自</span>
        <Select value={source} onValueChange={(value) => onSource(value as BrowserSource)}>
          <SelectTrigger size="sm" className="w-48" aria-label="來自">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {browserSources.map((value) => (
              <SelectItem key={value} value={value}>
                {sourceLabel[value]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    </div>
  )
}

function Preview({ path, source, inspection }: { path: string; source: BrowserSource; inspection: Inspection }) {
  const fresh = newVisits(inspection)
  const family = detectedFamilies[inspection.recognizedFiles[0]?.kind ?? '']
  const browser = source !== 'detect' ? sourceLabel[source] : family ? detectedLabel[family] : null
  const file = inspection.recognizedFiles[0]?.path ?? path
  const sample = inspection.previewEntries.slice(0, 5)

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1 rounded-lg bg-muted p-3 text-[13px]">
        <span className="truncate font-mono text-xs text-muted-foreground" title={file}>
          {baseName(file)}
          {browser && ` · ${browser}`}
        </span>
        <span className="text-base font-semibold tabular">{`${number(fresh)} 筆新的瀏覽記錄`}</span>
        <span className="text-muted-foreground tabular">{`${number(inspection.duplicateItems)} 筆已經在存檔裡`}</span>
        {syncCopies(inspection) > 0 && (
          <span className="text-muted-foreground tabular">{`${number(syncCopies(inspection))} 筆是存檔裡已有記錄的 Firefox 同步副本：一併保存，只算一次`}</span>
        )}
        <span className="text-muted-foreground tabular">
          {`檔案裡有 ${number(inspection.candidateItems)} 筆瀏覽記錄`}
          {inspection.previewRangeStart && inspection.previewRangeEnd && ` · ${`從 ${date(inspection.previewRangeStart)} 到 ${date(inspection.previewRangeEnd)}`}`}
        </span>
      </div>
      {sample.length > 0 && (
        <div className="flex flex-col gap-1">
          <span className="text-xs text-muted-foreground">檔案裡最近的記錄</span>
          <ul className="flex flex-col text-[13px]">
            {sample.map((entry) => (
              <li key={`${entry.sourceVisitId}-${entry.visitedAt}`} className="flex items-baseline gap-2 py-0.5">
                <span className="min-w-0 flex-1 truncate" title={entry.url}>
                  {entry.title || entry.url || '無標題'}
                </span>
                {entry.status === 'duplicate' && <span className="shrink-0 text-xs text-muted-foreground">已儲存</span>}
                {entry.status === 'same-visit' && <span className="shrink-0 text-xs text-muted-foreground">同步副本</span>}
                <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">{dayAndTime(entry.visitedAt)}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
      <p className="text-xs text-muted-foreground">
        {family !== 'chromium' && '只會匯入瀏覽記錄和頁面。圖示、下載和搜尋不在這個檔案裡。 '}
        PathKeep 讀取的是檔案的副本，不會改動瀏覽器自己的檔案。
      </p>
    </div>
  )
}

export function BrowserImportCard({ browser }: { browser: ReturnType<typeof useBrowserImport> }) {
  const scene = useScene()
  const [source, setSource] = useState<BrowserSource>('detect')
  const { state, inspect, confirm, stop, reset } = browser

  return (
    <SectionCard title="匯入瀏覽器歷史" subtitle="從別的瀏覽器或電腦匯入 History、places.sqlite、History.db 檔案，或整個使用者資料夾。">
      {state.step === 'idle' && (
        <>
          <Entry disabled={scene.running} source={source} onSource={setSource} onPath={(path) => path.trim() && inspect(path.trim(), source)} />
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
          <Preview path={state.path} source={state.source} inspection={state.inspection} />
          {toSave(state.inspection) === 0 && <p className="text-[13px] text-muted-foreground">這個檔案裡的內容都已經在存檔裡了。</p>}
          {scene.running && <p className="text-[13px] text-muted-foreground">請等備份完成。</p>}
          <div className="flex gap-2">
            {toSave(state.inspection) > 0 && (
              <Button size="sm" disabled={scene.running} onClick={confirm}>
                {`匯入 ${number(toSave(state.inspection))} 筆`}
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
          <ImportingBar progress={state.progress} label="正在匯入…" countWithoutTotal />
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
            {syncCopies(state.result) > 0 && (
              <span className="text-muted-foreground">{`${number(syncCopies(state.result))} 筆是存檔裡已有記錄的 Firefox 同步副本：一併保存，只算一次`}</span>
            )}
            {state.result.stopped && <span>再匯入這個檔案會補上其餘的；已經存下的會算作已在存檔裡。</span>}
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
