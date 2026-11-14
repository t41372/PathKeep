/**
 * The chosen model's semantic index under the semantic search switch: its
 * build (progress, pace, pause, continue, cancel), what it still needs, how
 * complete it is, whether it was made from an older version of the page
 * text, and whether backups keep it up to date. Mirrors
 * src/features/settings/semantic-index-state.tsx on the fake state.
 */
import { useState, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Progress } from '@/components/ui/progress'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/cn'
import { num } from '~/data/format'
import { IN_APP, IN_APP_MODEL, TOTAL_VISITS, useAi } from './state'
import { RebuildDialog } from './rebuild'

/** Pages in the complete index that search keeps using while a rebuild runs. */
const CURRENT_VECTORS = 498_210

export function IndexState() {
  const ai = useAi()
  const { phase } = ai

  if (phase === 'running' || phase === 'queued' || phase === 'pausedUser' || phase === 'pausedService' || phase === 'pausedQueue') {
    const backgroundLink = (
      <Button size="xs" variant="ghost" asChild>
        <Link to="/settings/background">背景工作</Link>
      </Button>
    )
    const cancel = (
      <Button size="xs" variant="ghost" onClick={() => ai.set((s) => ({ phase: s.chosenComplete ? 'ready' : s.done > 0 ? 'unfinished' : 'needsBuild' }))}>
        取消
      </Button>
    )
    const meanwhile = ai.chosenComplete ? <p className="text-[13px] text-muted-foreground">完成之前，搜尋繼續用現有的索引：{num(CURRENT_VECTORS)} 個頁面。</p> : null

    if (phase === 'running') {
      return (
        <div className="flex flex-col gap-2">
          <ProgressLine
            label="正在建立索引…"
            detail={[`${num(ai.done)} / ${num(TOTAL_VISITS)} 次造訪`, pace(ai.done, ai.service ? 38 : 820)].join(' · ')}
            value={ai.done / TOTAL_VISITS}
            action={
              <>
                <Button size="xs" variant="outline" onClick={() => ai.set({ phase: 'pausedUser' })}>
                  暫停
                </Button>
                {cancel}
                {backgroundLink}
              </>
            }
          />
          {meanwhile}
        </div>
      )
    }
    if (phase === 'queued') {
      return (
        <div className="flex flex-col gap-2">
          <ProgressLine
            label="正在等其他背景工作完成…"
            action={
              <>
                {cancel}
                {backgroundLink}
              </>
            }
          />
          {meanwhile}
        </div>
      )
    }
    const byService = phase === 'pausedService'
    const byQueue = phase === 'pausedQueue'
    return (
      <div role="status" className="flex flex-col gap-2 text-[13px]">
        <p className={cn('[overflow-wrap:anywhere]', byService ? 'text-destructive' : 'text-muted-foreground')}>
          {byService
            ? `${ai.service?.name ?? ai.modelName} 沒能執行這個模型，建立索引已暫停。它的回覆：Failed to load model "${ai.modelName}". Error: Model not found`
            : byQueue
              ? '所有背景工作都已暫停，恢復後會繼續建立索引。'
              : `已暫停：${num(ai.done)} / ${num(TOTAL_VISITS)} 次造訪。`}
        </p>
        {meanwhile}
        <div className="flex justify-end gap-2">
          {byQueue ? (
            backgroundLink
          ) : (
            <>
              {cancel}
              <Button size="xs" variant="outline" onClick={() => ai.set({ phase: 'running' })}>
                繼續
              </Button>
            </>
          )}
        </div>
      </div>
    )
  }

  if (phase === 'setupRunning') return <ProgressLine label="正在等其他背景工作完成…" />
  if (phase === 'needsBuild' || phase === 'unfinished' || phase === 'unfinishedOlderText') return <NeedsBuild />
  if (phase === 'failed') return <p className="text-[13px] text-destructive">建立索引失敗。請重建搜尋索引再試一次。</p>
  if (phase === 'degraded') return <p className="text-[13px] text-destructive">索引不完整。請重建索引來修復。</p>

  const summary = <p className="text-[13px] text-muted-foreground">{phase === 'readyEmpty' ? '還沒有建立任何索引。' : `已建立 ${num(TOTAL_VISITS)} 次造訪的索引 · 上次建立索引：3 小時前`}</p>
  if (phase !== 'olderText') return summary
  return (
    <div className="flex flex-col gap-2 text-[13px]">
      {summary}
      <p className="text-foreground">這份索引是用舊版的頁面文字建立的，裡面還夾著造訪時間和網頁程式碼殘片。它照樣能用；重建之後，依意思搜尋會找得更準。</p>
      <div className="flex justify-end">
        <StartOver label="重建索引" />
      </div>
    </div>
  )
}

/** Rebuilds the chosen model's index from nothing, after the same confirmation as 重建搜尋索引. */
function StartOver({ label }: { label: string }) {
  const ai = useAi()
  const [open, setOpen] = useState(false)
  return (
    <>
      <Button size="sm" variant="outline" onClick={() => setOpen(true)}>
        {label}
      </Button>
      <RebuildDialog
        open={open}
        onOpenChange={setOpen}
        onConfirm={() => {
          ai.set({ phase: 'running', done: 0 })
          toast.success('正在重建搜尋索引。')
        }}
      />
    </>
  )
}

/** The chosen model has no complete index and nothing is building it. */
function NeedsBuild() {
  const ai = useAi()
  const started = ai.phase !== 'needsBuild'
  const olderText = ai.phase === 'unfinishedOlderText'
  const model = ai.modelName
  const serving = ai.servingOther ? (ai.servingOther === IN_APP ? IN_APP_MODEL : (ai.embeddingServices.find((p) => p.id === ai.servingOther)?.defaultModel ?? ai.servingOther)) : null
  return (
    <div className="flex flex-col gap-2 text-[13px]">
      <p className="text-foreground">{started ? `用 ${model} 建立的索引還沒完成：目前 ${num(ai.done)} 次造訪。` : `${model} 還沒有索引。建立索引會在背景讀取 ${num(TOTAL_VISITS)} 次造訪，隨時可以暫停。`}</p>
      {olderText && <p className="text-foreground">目前已經建立索引的造訪是用舊方式讀的。可以從頭重來，用現在的方式讀每一次造訪；也可以繼續，保留已經建好的部分。</p>}
      <p className="text-muted-foreground">
        {serving ? `完成之前，語意搜尋繼續用 ${serving}。 新索引完成後，用 ${serving} 建立的索引會被刪除。之後想換回 ${serving}，就得重新建它的索引。` : '完成之前，依意思搜尋只找得到已經建立索引的造訪。'}
      </p>
      <div className="flex justify-end gap-2">
        {olderText && <StartOver label="從頭重來" />}
        <Button size="sm" variant="outline" onClick={() => ai.set({ phase: 'running' })}>
          {started ? '繼續' : '建立索引'}
        </Button>
      </div>
    </div>
  )
}

/** Whether each backup's new visits are indexed; it starts once the index is complete. */
export function AutoUpdate() {
  const ai = useAi()
  return (
    <div className="flex items-center justify-between gap-3 border-t pt-3 text-[13px]">
      <div className="flex flex-col gap-0.5">
        <Label htmlFor="settings-semantic-auto" className="font-normal">
          每次備份後為新的造訪建立索引
        </Label>
        <span className="text-xs text-muted-foreground">索引完成後才會開始。</span>
      </div>
      <Switch id="settings-semantic-auto" checked={ai.autoUpdate} onCheckedChange={(autoUpdate) => ai.set({ autoUpdate })} />
    </div>
  )
}

/** Speed and time left, as the app words them. */
function pace(done: number, perSecond: number) {
  const secondsLeft = (TOTAL_VISITS - done) / perSecond
  const minutes = Math.round(secondsLeft / 60)
  const left = secondsLeft < 60 ? '剩不到一分鐘' : minutes < 90 ? `大約還要 ${num(minutes)} 分鐘` : `大約還要 ${num(Math.floor(minutes / 60))} 小時 ${num(minutes % 60)} 分鐘`
  return `每秒 ${num(perSecond)} 次 · ${left}`
}

export function ProgressLine({ label, detail, value, action }: { label: string; detail?: string; value?: number; action?: ReactNode }) {
  return (
    <div role="status" className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3 text-[13px]">
        <span>{label}</span>
        <span className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
          {detail}
          {action}
        </span>
      </div>
      <Progress value={value === undefined ? undefined : Math.round(value * 100)} aria-label={label} />
    </div>
  )
}
