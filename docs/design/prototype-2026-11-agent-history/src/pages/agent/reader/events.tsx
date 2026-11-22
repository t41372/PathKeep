/**
 * How each projected event looks in the reader. Tool calls fold into one
 * line per group, subagents into a card, compaction and branches are said
 * out loud, and every turn ends with its time and tokens.
 */
import {
  AlertTriangle,
  Bot,
  Brain,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleSlash,
  FileCode2,
  FilePen,
  FileSearch,
  Globe,
  ImageOff,
  Info,
  ListTodo,
  Minimize2,
  SquareTerminal,
  Wrench,
} from 'lucide-react'
import { memo, useState, type ReactNode } from 'react'
import { toast } from 'sonner'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { tokenParts } from '~/components/bits'
import { totalTokens } from '~/data/fixture'
import { big, duration, time } from '~/data/format'
import type { TEvent, ToolCall, ToolName } from '~/data/transcript'
import type { Subagent } from '~/data/types'
import { Markdown } from './markdown'

const toolIcon: Record<ToolName, typeof Wrench> = {
  Read: FileSearch,
  read_file: FileSearch,
  Grep: FileSearch,
  Glob: FileSearch,
  Bash: SquareTerminal,
  shell: SquareTerminal,
  Edit: FilePen,
  apply_patch: FilePen,
  Write: FileCode2,
  TodoWrite: ListTodo,
  WebSearch: Globe,
}

function UserEvent({ e }: { e: Extract<TEvent, { k: 'user' }> }) {
  const [version, setVersion] = useState(e.branches?.current ?? 1)
  const older = e.branches && version < e.branches.count
  return (
    <div className="flex flex-col items-end gap-1.5">
      <div className="flex items-center gap-2 text-[11.5px] text-muted-foreground">
        {e.branches && (
          <span className="flex items-center gap-0.5 rounded-md border px-1 py-px">
            <button type="button" aria-label="上一個版本" disabled={version <= 1} onClick={() => setVersion((v) => v - 1)} className="disabled:opacity-30">
              <ChevronLeft className="size-3.5" />
            </button>
            <span className="tabular">
              第 {version} / {e.branches.count} 版
            </span>
            <button type="button" aria-label="下一個版本" disabled={version >= e.branches.count} onClick={() => setVersion((v) => v + 1)} className="disabled:opacity-30">
              <ChevronRight className="size-3.5" />
            </button>
          </span>
        )}
        <span>你 · {time(e.ts)}</span>
      </div>
      <div className="max-w-[85%] rounded-[18px] bg-user-bubble px-4 py-2.5 text-[14px] leading-normal whitespace-pre-wrap text-user-bubble-foreground">
        {older ? '先別改程式，列出會受影響的檔案就好。' : e.text}
      </div>
      {older && (
        <p className="max-w-[85%] text-right text-[12px] text-muted-foreground">
          這是你改寫前的版本，後面接的是另一條分支（9 則事件）。PathKeep 兩條都存了；預設顯示工具最後採用的那一條。
        </p>
      )}
      {e.attachments?.map((a) => (
        <span key={a.name} className="flex items-center gap-1.5 rounded-lg border border-dashed px-2.5 py-1.5 text-[12px] text-muted-foreground">
          <ImageOff className="size-3.5" />
          {a.name}
          <span className="text-foreground/70">· 工具沒有把這張圖存進對話紀錄，所以 PathKeep 也拿不到</span>
        </span>
      ))}
    </div>
  )
}

function ThinkingEvent({ e, forceOpen }: { e: Extract<TEvent, { k: 'thinking' }>; forceOpen?: boolean }) {
  const [open, setOpen] = useState(false)
  return (
    <Collapsible open={open || forceOpen} onOpenChange={setOpen}>
      <CollapsibleTrigger className="group flex items-center gap-1.5 text-[12.5px] text-muted-foreground hover:text-foreground">
        <Brain className="size-3.5" />
        思考了 {duration(e.ms)}
        <ChevronDown className="size-3 transition-transform group-data-[state=open]:rotate-180" />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <p className="mt-1.5 border-l-2 pl-3 text-[13px] leading-relaxed text-muted-foreground italic">{e.text}</p>
      </CollapsibleContent>
    </Collapsible>
  )
}

function CallDetail({ call }: { call: ToolCall }) {
  if (call.diff)
    return (
      <pre className="overflow-x-auto rounded-md bg-muted p-2.5 font-mono text-[11.5px] leading-[1.55]">
        {call.diff.text.split('\n').map((line, i) => (
          <div key={i} className={cn(line.startsWith('+') && !line.startsWith('+++') && 'bg-green/12 text-green', line.startsWith('-') && !line.startsWith('---') && 'bg-red/10 text-red', line.startsWith('@@') && 'text-muted-foreground')}>
            {line || ' '}
          </div>
        ))}
      </pre>
    )
  if (call.output)
    return <pre className="max-h-[220px] overflow-auto rounded-md bg-muted p-2.5 font-mono text-[11.5px] leading-[1.55] whitespace-pre-wrap">{call.output}</pre>
  return <p className="text-[12px] text-muted-foreground">這個工具沒有輸出。</p>
}

function CallRow({ call, forceOpen }: { call: ToolCall; forceOpen?: boolean }) {
  const [state, setOpen] = useState(false)
  const open = state || !!forceOpen
  const Icon = toolIcon[call.tool]
  return (
    <li>
      <button type="button" onClick={() => setOpen((o) => !o)} className="flex w-full min-w-0 items-center gap-2 rounded-md px-2 py-1 text-left text-[12.5px] hover:bg-muted">
        <Icon className="size-3.5 shrink-0 text-muted-foreground" />
        <span className="shrink-0 font-mono text-[11.5px] font-medium">{call.tool}</span>
        <span className="truncate font-mono text-[11.5px] text-muted-foreground">{call.arg}</span>
        <span className="ml-auto flex shrink-0 items-center gap-2 text-[11.5px] text-muted-foreground tabular">
          {call.diff && (
            <span>
              <span className="text-green">+{call.diff.add}</span> <span className="text-red">−{call.diff.del}</span>
            </span>
          )}
          {call.status === 'error' && <span className="text-destructive">失敗</span>}
          {call.status === 'denied' && <span className="text-amber-600 dark:text-amber-400">你拒絕了</span>}
          {duration(call.ms)}
        </span>
      </button>
      {open && (
        <div className="mt-1 mb-2 ml-7">
          <CallDetail call={call} />
        </div>
      )}
    </li>
  )
}

function ToolsEvent({ e, forceOpen }: { e: Extract<TEvent, { k: 'tools' }>; forceOpen?: boolean }) {
  const [open, setOpen] = useState(false)
  const counts = new Map<string, number>()
  for (const c of e.calls) counts.set(c.tool, (counts.get(c.tool) ?? 0) + 1)
  const summary = [...counts].map(([t, n]) => (n > 1 ? `${t} ×${n}` : t)).join(' · ')
  const add = e.calls.reduce((s, c) => s + (c.diff?.add ?? 0), 0)
  const del = e.calls.reduce((s, c) => s + (c.diff?.del ?? 0), 0)
  const failed = e.calls.filter((c) => c.status !== 'ok').length
  return (
    <Collapsible open={open || forceOpen} onOpenChange={setOpen} className="rounded-lg border bg-card/50">
      <CollapsibleTrigger className="group flex w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-[12.5px]">
        <Wrench className="size-3.5 shrink-0 text-muted-foreground" />
        <span className="shrink-0 text-muted-foreground">用了 {e.calls.length} 次工具</span>
        {add + del > 0 && (
          <span className="shrink-0 text-[12px] text-muted-foreground tabular">
            · 改了 +{add} −{del} 行
          </span>
        )}
        <span className="ml-auto flex shrink-0 items-center gap-2.5 text-[11.5px] text-muted-foreground tabular">
          {failed > 0 && <span className="size-1.5 rounded-full bg-amber-500" title={`${failed} 次沒成功`} />}
          <ChevronDown className="size-3.5 transition-transform group-data-[state=open]:rotate-180" />
        </span>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <p className="border-t px-3 pt-2 font-mono text-[11.5px] text-muted-foreground">
          {summary}
          {failed > 0 && <span className="ml-2 font-sans text-amber-700 dark:text-amber-300">{failed} 次沒成功</span>}
        </p>
        <ul className="px-1.5 py-1.5">
          {e.calls.map((c) => (
            <CallRow key={c.id} call={c} forceOpen={forceOpen && !!c.output} />
          ))}
        </ul>
      </CollapsibleContent>
    </Collapsible>
  )
}

function SubagentEvent({ e, onOpen }: { e: Extract<TEvent, { k: 'subagent' }>; onOpen: (sub: Subagent) => void }) {
  const s = e.sub
  return (
    <button type="button" onClick={() => onOpen(s)} className="flex w-full min-w-0 items-center gap-2.5 rounded-lg border bg-card/60 px-3 py-2 text-left transition-colors hover:bg-card">
      <Bot className="size-4 shrink-0 text-muted-foreground" />
      <span className="shrink-0 text-[12px] text-muted-foreground">subagent</span>
      <span className="truncate text-[13px] font-medium">{s.title}</span>
      <span className="ml-auto shrink-0 text-[12px] text-muted-foreground tabular">{duration(s.agentMs)}</span>
      <ChevronRight className="size-4 shrink-0 text-muted-foreground" />
    </button>
  )
}

/** Several subagents sent off together read as one step; open it to see each. */
export function SubagentGroup({ subs, onOpen }: { subs: Extract<TEvent, { k: 'subagent' }>[]; onOpen: (sub: Subagent) => void }) {
  const [open, setOpen] = useState(false)
  const total = subs.reduce((s, e) => s + e.sub.agentMs, 0)
  return (
    <Collapsible open={open} onOpenChange={setOpen} className="rounded-lg border bg-card/60">
      <CollapsibleTrigger className="group flex w-full items-center gap-2.5 px-3 py-2 text-left text-[13px]">
        <Bot className="size-4 shrink-0 text-muted-foreground" />
        <span className="font-medium">派出 {subs.length} 個 subagent</span>
        <span className="truncate text-[12px] text-muted-foreground">{subs.map((e) => e.sub.agentType).filter((t, i, a) => a.indexOf(t) === i).join('、')}</span>
        <span className="ml-auto shrink-0 text-[12px] text-muted-foreground tabular">合計 {duration(total)}</span>
        <ChevronDown className="size-3.5 shrink-0 text-muted-foreground transition-transform group-data-[state=open]:rotate-180" />
      </CollapsibleTrigger>
      <CollapsibleContent className="flex flex-col gap-1.5 border-t p-1.5">
        {subs.map((e) => (
          <SubagentEvent key={e.id} e={e} onOpen={onOpen} />
        ))}
      </CollapsibleContent>
    </Collapsible>
  )
}

function CompactionEvent({ e }: { e: Extract<TEvent, { k: 'compaction' }> }) {
  const [open, setOpen] = useState(false)
  return (
    <div className="flex flex-col gap-1.5 py-1">
      <div className="flex items-center gap-3 text-[12px] text-muted-foreground">
        <span className="h-px flex-1 bg-border" />
        <Minimize2 className="size-3.5" />
        上下文在這裡壓縮了（壓縮前約 {big(e.tokensBefore)} token）
        <span className="h-px flex-1 bg-border" />
      </div>
      <p className="text-center text-[12px] text-muted-foreground">
        從這裡開始，Agent 只看得到前面的摘要；上面的完整內容 PathKeep 都留著。
        <button type="button" className="ml-1 text-brand hover:underline" onClick={() => setOpen((o) => !o)}>
          {open ? '收起摘要' : '看 Agent 拿到的摘要'}
        </button>
      </p>
      {open && (
        <div className="mx-auto max-w-[560px] rounded-lg border bg-muted/50 p-3 text-[12.5px] leading-relaxed text-muted-foreground">
          使用者在修 Safari 匯入的時區錯位。已確認問題在 safari.rs 把本地時間當 UTC；已改成用匯入時的時區轉換，測試通過。剩下：README 的說明、E2E 補一個跨日的案例。
        </div>
      )}
    </div>
  )
}

export function TurnEnd({ e }: { e: Extract<TEvent, { k: 'turn-end' }> }) {
  const t = e.tokens
  return (
    <div className="flex justify-end text-[11.5px] text-muted-foreground tabular">
      <Tooltip>
        <TooltipTrigger asChild>
          <span tabIndex={0} className="rounded px-1.5 py-0.5 hover:bg-muted">
            這一輪 {duration(e.ms)} · {t ? `${big(totalTokens(t))} token` : '沒有用量紀錄'} · {e.model}
          </span>
        </TooltipTrigger>
        {t && (
          <TooltipContent className="flex flex-col gap-0.5 tabular">
            {tokenParts.map((p) => (
              <span key={p.key}>
                {p.label} {big(t[p.key])}
              </span>
            ))}
          </TooltipContent>
        )}
      </Tooltip>
    </div>
  )
}

function Notice({ e }: { e: Extract<TEvent, { k: 'notice' }> }) {
  const Icon = e.tone === 'info' ? (e.text.includes('中斷') ? CircleSlash : Info) : AlertTriangle
  return (
    <div className={cn('flex items-center gap-2 rounded-md px-2.5 py-1.5 text-[12.5px]', e.tone === 'warn' ? 'bg-amber-500/10 text-amber-800 dark:text-amber-300' : 'bg-muted text-muted-foreground')}>
      <Icon className="size-3.5 shrink-0" />
      {e.text}
    </div>
  )
}

export const EventView = memo(function EventView({ e, onOpenSub, forceOpen }: { e: TEvent; onOpenSub: (sub: Subagent) => void; forceOpen?: boolean }) {
  let body: ReactNode
  switch (e.k) {
    case 'user':
      body = <UserEvent e={e} />
      break
    case 'thinking':
      body = <ThinkingEvent e={e} forceOpen={forceOpen} />
      break
    case 'tools':
      body = <ToolsEvent e={e} forceOpen={forceOpen} />
      break
    case 'assistant':
      body = <Markdown content={e.md} />
      break
    case 'subagent':
      body = <SubagentEvent e={e} onOpen={onOpenSub} />
      break
    case 'compaction':
      body = <CompactionEvent e={e} />
      break
    case 'turn-end':
      body = <TurnEnd e={e} />
      break
    case 'notice':
      body = <Notice e={e} />
      break
  }
  return body
})

export function copyText(text: string, message: string) {
  void navigator.clipboard?.writeText(text).catch(() => undefined)
  toast.success(message)
}
