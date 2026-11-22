/**
 * The conversation reader, used beside the list and as a full page.
 * Header: title, harness, project, branch, time, tokens, saved state, and
 * the actions (continue, export). Body: the transcript, virtualized for
 * conversations with thousands of events, with an outline of what you asked,
 * find (⌘F), an info panel, and the raw records.
 */
import { useVirtualizer } from '@tanstack/react-virtual'
import {
  ArrowDownToLine,
  ChevronDown,
  ChevronUp,
  Copy,
  Download,
  Ellipsis,
  Maximize2,
  PanelRight,
  Search,
  Star,
  TerminalSquare,
  Undo2,
  X,
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { HarnessBadge, ProjectChip } from '~/components/bits'
import { ArchivedFlag, OnlyHereFlag } from '~/components/chat-row'
import { harnesses, projectOf, resumeCommand } from '~/data/archive'
import { totalTokens } from '~/data/fixture'
import { big, date, dateTime, duration, num } from '~/data/format'
import { transcriptOf, type TEvent } from '~/data/transcript'
import type { Chat, Subagent } from '~/data/types'
import { ChatManageItems, putBack } from '../chats/chat-actions'
import { useChatEdits } from '../chats/chat-edits'
import { copyText, EventView, SubagentGroup, TurnEnd } from './events'
import { paintMatches, useClearHighlights, useMatches } from './find'
import { InfoPanel } from './info'
import { RawEvents } from './raw'

const estimate: Record<TEvent['k'], number> = { user: 84, assistant: 120, tools: 44, thinking: 30, subagent: 66, compaction: 56, 'turn-end': 40, notice: 40 }

function IconButton({ label, onClick, active, children }: { label: string; onClick?: () => void; active?: boolean; children: React.ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          aria-pressed={active}
          onClick={onClick}
          className={cn('flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground', active && 'bg-muted text-foreground')}
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}

function ResumeMenu({ chat, cwdGone }: { chat: Chat; cwdGone: boolean }) {
  const cmd = resumeCommand(chat)
  const h = harnesses[chat.harness]
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline" size="sm">
          <TerminalSquare />
          繼續對話
          <ChevronDown className="opacity-60" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-[380px] p-2">
        <DropdownMenuLabel className="px-1 text-[12px] font-normal text-muted-foreground">在終端機用 {h.name} 接著這段對話：</DropdownMenuLabel>
        <pre className="my-1 overflow-x-auto rounded-md bg-muted p-2 font-mono text-[11.5px] whitespace-pre-wrap [overflow-wrap:anywhere]">{cmd}</pre>
        {chat.source === 'gone' && (
          <p className="px-1 py-1 text-[12px] leading-relaxed text-amber-700 dark:text-amber-300">
            {h.name} 的資料夾裡已經沒有這段對話，這個指令會找不到它。把對話放回去的功能還沒做。
          </p>
        )}
        {cwdGone && chat.source !== 'gone' && (
          <p className="px-1 py-1 text-[12px] leading-relaxed text-amber-700 dark:text-amber-300">工作資料夾已經刪了。要在原處接著做，得先在同一個路徑把資料夾建回來（例如重新建立 worktree）。</p>
        )}
        <DropdownMenuItem onSelect={() => copyText(cmd, '指令已複製，貼到終端機就能繼續')}>
          <Copy />
          複製指令
        </DropdownMenuItem>
        <p className="px-1 pt-1 text-[11.5px] text-muted-foreground">PathKeep 不會替你開終端機或啟動 {h.name}。</p>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

function ExportSubmenu({ chat }: { chat: Chat }) {
  const done = (what: string) => toast.success(`已匯出到「下載」：${chat.title.slice(0, 18)}.${what}`)
  const item = (label: string, hint: string, ext: string) => (
    <DropdownMenuItem onSelect={() => done(ext)}>
      <span className="flex flex-col">
        {label}
        <span className="text-[12px] text-muted-foreground">{hint}</span>
      </span>
    </DropdownMenuItem>
  )
  return (
    <DropdownMenuSub>
      <DropdownMenuSubTrigger>
        <Download />
        匯出
      </DropdownMenuSubTrigger>
      <DropdownMenuSubContent className="w-[280px]">
        {item('Markdown', '好讀、好分享；工具呼叫摺成一行', 'md')}
        {item('HTML', '單一檔案，跟這裡看到的一樣', 'html')}
        {item('JSON Lines', '整理過的事件，給程式讀', 'jsonl')}
        <DropdownMenuSeparator />
        {item('原始檔案', `${harnesses[chat.harness].name} 寫的原樣，含 subagent 檔`, chat.harness === 'codex' ? 'jsonl.zst' : 'jsonl')}
      </DropdownMenuSubContent>
    </DropdownMenuSub>
  )
}

function SubagentSheet({ sub, onClose }: { sub: Subagent | null; onClose: () => void }) {
  const events = useMemo<TEvent[]>(() => {
    if (!sub) return []
    const ts = Date.now()
    return [
      { k: 'user', id: 's-u', ts, turn: 0, text: sub.title },
      { k: 'tools', id: 's-t', ts, turn: 0, calls: [
        { id: 'a', tool: 'Grep', arg: '"open_archive"', ms: 220, status: 'ok', output: 'src-tauri/crates/vault-core/src/archive.rs:41\nsrc-tauri/src/commands/backup.rs:88' },
        { id: 'b', tool: 'Read', arg: 'src-tauri/crates/vault-core/src/archive.rs', ms: 140, status: 'ok', output: '（212 行）' },
        { id: 'c', tool: 'Read', arg: 'src-tauri/src/commands/backup.rs', ms: 120, status: 'ok', output: '（96 行）' },
      ] },
      { k: 'assistant', id: 's-a', ts, turn: 0, md: '找到 2 個呼叫點：\n\n1. `archive.rs:41` 建立存檔時\n2. `backup.rs:88` 每次備份開始時\n\n兩處都沒有處理密碼錯誤以外的失敗。' },
      { k: 'turn-end', id: 's-e', ts, turn: 0, ms: sub.agentMs, tokens: sub.tokens, model: sub.model },
    ]
  }, [sub])
  return (
    <Sheet open={!!sub} onOpenChange={(o) => !o && onClose()}>
      <SheetContent className="w-[560px] sm:max-w-[560px]">
        {sub && (
          <>
            <SheetHeader>
              <SheetTitle>{sub.title}</SheetTitle>
              <SheetDescription>
                subagent · {sub.agentType} · {sub.model} · {duration(sub.agentMs)} · {big(totalTokens(sub.tokens))} token
              </SheetDescription>
            </SheetHeader>
            <div className="flex flex-col gap-3 overflow-y-auto px-4 pb-6">
              <p className="text-[12px] text-muted-foreground">subagent 有自己的紀錄檔；PathKeep 把它掛在呼叫它的那一步底下，不在列表裡單獨列出。</p>
              {events.map((e) => (
                <EventView key={e.id} e={e} onOpenSub={() => undefined} />
              ))}
            </div>
          </>
        )}
      </SheetContent>
    </Sheet>
  )
}

export function Reader({
  chat,
  page,
  find,
  at,
  onClose,
  onTrashed,
}: {
  chat: Chat
  page?: boolean
  find?: string
  at?: number
  onClose?: () => void
  /** The conversation went to the trash from this reader's ⋯ menu. */
  onTrashed?: () => void
}) {
  const navigate = useNavigate()
  const project = projectOf(chat)
  const transcript = useMemo(() => transcriptOf(chat), [chat])
  const { events, prompts } = transcript
  const [view, setView] = useState<'chat' | 'raw'>('chat')
  const [info, setInfo] = useState(false)
  const [outline, setOutline] = useState(!!page)
  const [findOpen, setFindOpen] = useState(!!find)
  const [query, setQuery] = useState(find ?? '')
  const [current, setCurrent] = useState(0)
  const [sub, setSub] = useState<Subagent | null>(null)
  const [starred, setStarred] = useState(chat.starred)
  const scrollRef = useRef<HTMLDivElement>(null)
  const findRef = useRef<HTMLInputElement>(null)
  const location = project.locations.find((l) => l.path === chat.cwd)
  const cwdGone = !!location && !location.exists
  const edits = useChatEdits()
  const archived = edits.archived.has(chat.id)
  const trashed = edits.trashed.has(chat.id)
  useClearHighlights()

  const matches = useMatches(events, findOpen ? query : '')
  const openEvents = useMemo(() => new Set(matches.map((m) => m.event)), [matches])

  // Rows on screen: consecutive subagents fold into one row ("派出 N 個 subagent").
  const rows = useMemo(() => {
    const list: { first: number; last: number }[] = []
    for (let i = 0; i < events.length; i++) {
      let j = i
      if (events[i].k === 'subagent') while (j + 1 < events.length && events[j + 1].k === 'subagent') j++
      list.push({ first: i, last: j })
      i = j
    }
    const rowOf = new Int32Array(events.length)
    list.forEach((r, ri) => {
      for (let k = r.first; k <= r.last; k++) rowOf[k] = ri
    })
    return { list, rowOf }
  }, [events])

  const virtualizer = useVirtualizer({
    count: rows.list.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => estimate[events[rows.list[i].first].k],
    overscan: 6,
  })
  const items = virtualizer.getVirtualItems()
  const scrollToEvent = (event: number, align: 'start' | 'center' | 'end') => virtualizer.scrollToIndex(rows.rowOf[event] ?? 0, { align })

  // Jump to a search hit when opened from results.
  useEffect(() => {
    setView('chat')
    setQuery(find ?? '')
    setFindOpen(!!find)
    setStarred(chat.starred)
    if (at !== undefined) requestAnimationFrame(() => scrollToEvent(at, 'center'))
    else virtualizer.scrollToOffset(0)
  }, [chat.id, find, at, virtualizer, chat.starred])

  useEffect(() => {
    if (!find || at === undefined) return setCurrent(0)
    const i = matches.findIndex((m) => m.event >= at)
    setCurrent(Math.max(0, i))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chat.id, find, at])

  const go = (delta: number) => {
    if (!matches.length) return
    const next = (current + delta + matches.length) % matches.length
    setCurrent(next)
    scrollToEvent(matches[next].event, 'center')
  }

  useEffect(() => {
    const id = requestAnimationFrame(() => paintMatches(scrollRef.current, findOpen ? query : '', matches[current]))
    return () => cancelAnimationFrame(id)
  })

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'f') {
        e.preventDefault()
        setFindOpen(true)
        requestAnimationFrame(() => findRef.current?.select())
      } else if (e.key === 'Escape' && !e.defaultPrevented) {
        if (findOpen) setFindOpen(false)
        else if (!document.querySelector('[role="dialog"],[role="menu"]')) onClose?.()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [findOpen, onClose])

  const topTurn = events[rows.list[virtualizer.range?.startIndex ?? 0]?.first ?? 0]?.turn ?? 0

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col" data-reader="">
      {/* Header: one line of title, one line of context; the rest is in ⋯ and 資訊. */}
      <header className="flex flex-col gap-1.5 border-b px-5 pt-3 pb-3">
        <div className="flex items-center gap-2">
          <h1 title={chat.title} className={cn('min-w-0 flex-1 truncate text-[16px] leading-snug tracking-[-0.01em]', chat.titleSource === 'first-prompt' ? 'font-medium' : 'font-semibold')}>
            {chat.title}
          </h1>
          <div className="flex shrink-0 items-center gap-1">
            <IconButton label={starred ? '取消星號' : '加星號'} onClick={() => setStarred((s) => !s)}>
              <Star className={cn('size-4', starred && 'fill-brand text-brand')} />
            </IconButton>
            <ResumeMenu chat={chat} cwdGone={cwdGone} />
            <IconButton label="資訊" active={info} onClick={() => setInfo((o) => !o)}>
              <PanelRight className="size-4" />
            </IconButton>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <button type="button" aria-label="更多" className="flex size-8 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground">
                  <Ellipsis className="size-4" />
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-[240px]">
                <DropdownMenuItem
                  onSelect={() => {
                    setView('chat')
                    setFindOpen(true)
                    requestAnimationFrame(() => findRef.current?.focus())
                  }}
                >
                  <Search />
                  在這段對話裡找
                  <DropdownMenuShortcut>⌘F</DropdownMenuShortcut>
                </DropdownMenuItem>
                <DropdownMenuCheckboxItem checked={outline} onCheckedChange={(v) => setOutline(!!v)}>
                  你說過的話（目錄）
                </DropdownMenuCheckboxItem>
                <DropdownMenuCheckboxItem checked={view === 'raw'} onCheckedChange={(v) => setView(v ? 'raw' : 'chat')}>
                  原始紀錄（{num(transcript.raw.length)} 筆）
                </DropdownMenuCheckboxItem>
                <DropdownMenuItem onSelect={() => scrollToEvent(events.length - 1, 'end')}>
                  <ArrowDownToLine />
                  跳到最後
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <ExportSubmenu chat={chat} />
                {!page && (
                  <DropdownMenuItem onSelect={() => void navigate(`/agent/chat/${chat.id}`)}>
                    <Maximize2 />
                    全頁打開
                  </DropdownMenuItem>
                )}
                <DropdownMenuItem onSelect={() => toast('改名只改 PathKeep 裡的標題，不動工具的檔案')}>改名…</DropdownMenuItem>
                <DropdownMenuItem onSelect={() => toast('已在 Finder 顯示原始檔')}>在 Finder 顯示原始檔</DropdownMenuItem>
                <DropdownMenuItem onSelect={() => copyText(`pathkeep://agent/chat/${chat.id}`, '連結已複製')}>複製連結</DropdownMenuItem>
                <ChatManageItems chat={chat} onTrashed={onTrashed} />
              </DropdownMenuContent>
            </DropdownMenu>
            {onClose && (
              <IconButton label="關閉（Esc）" onClick={onClose}>
                <X className="size-4" />
              </IconButton>
            )}
          </div>
        </div>
        <div className="flex min-w-0 items-center gap-x-2.5 text-[12.5px] text-muted-foreground">
          <HarnessBadge id={chat.harness} withName className="shrink-0 text-[12.5px]" />
          <span aria-hidden>·</span>
          <ProjectChip project={project} className="-mx-1.5 shrink-0" />
          <span aria-hidden>·</span>
          <span className="shrink-0 tabular">{dateTime(chat.started)}</span>
          <span aria-hidden>·</span>
          <span className="shrink-0 tabular">Agent {duration(chat.agentMs)}</span>
          {chat.source === 'gone' && <OnlyHereFlag />}
          {archived && <ArchivedFlag withText />}
        </div>
        {trashed && (
          <p className="flex items-center gap-2 text-[12.5px] text-muted-foreground">
            這段對話在垃圾桶裡，清單、搜尋和洞察都不算它。
            <Button variant="outline" size="xs" onClick={() => putBack(chat)}>
              <Undo2 />
              放回
            </Button>
          </p>
        )}
        {chat.source === 'gone' && (
          <p className="text-[12.5px] text-amber-800 dark:text-amber-300">
            {harnesses[chat.harness].name} 已經刪掉這段對話；這是 PathKeep 在 {date(chat.ended + 3600_000, true)} 存下的完整版本。
          </p>
        )}
        {view === 'raw' && (
          <p className="flex items-center gap-2 text-[12.5px] text-muted-foreground">
            正在看原始紀錄。
            <button type="button" className="text-brand hover:underline" onClick={() => setView('chat')}>
              回到對話
            </button>
          </p>
        )}
        {findOpen && view === 'chat' && (
          <div className="flex items-center gap-2 rounded-lg border bg-background/70 px-2.5 py-1">
            <Search className="size-3.5 text-muted-foreground" />
            <input
              ref={findRef}
              value={query}
              autoFocus
              placeholder="在這段對話裡找…"
              aria-label="在這段對話裡找"
              onChange={(e) => {
                setQuery(e.target.value)
                setCurrent(0)
              }}
              onKeyDown={(e) => {
                if (e.nativeEvent.isComposing) return
                if (e.key === 'Enter') go(e.shiftKey ? -1 : 1)
                if (e.key === 'Escape') {
                  e.preventDefault()
                  setFindOpen(false)
                }
              }}
              className="min-w-0 flex-1 bg-transparent text-[13px] outline-none"
            />
            <span className="text-[12px] text-muted-foreground tabular">{query ? (matches.length ? `${current + 1} / ${num(matches.length)}` : '沒有') : ''}</span>
            <button type="button" aria-label="上一個" onClick={() => go(-1)} className="text-muted-foreground hover:text-foreground">
              <ChevronUp className="size-4" />
            </button>
            <button type="button" aria-label="下一個" onClick={() => go(1)} className="text-muted-foreground hover:text-foreground">
              <ChevronDown className="size-4" />
            </button>
            <button type="button" aria-label="關閉找字" onClick={() => setFindOpen(false)} className="text-muted-foreground hover:text-foreground">
              <X className="size-3.5" />
            </button>
          </div>
        )}
      </header>

      {/* Body */}
      <div className="flex min-h-0 flex-1">
        {view === 'raw' ? (
          <RawEvents
            transcript={transcript}
            onJump={(event) => {
              setView('chat')
              requestAnimationFrame(() => scrollToEvent(event, 'center'))
            }}
          />
        ) : (
          <>
            <div ref={scrollRef} className="thin-scroll min-w-0 flex-1 overflow-y-auto">
              <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
                {items.map((v) => {
                  const row = rows.list[v.index]
                  const e = events[row.first]
                  return (
                    <div
                      key={e.id}
                      data-index={v.index}
                      data-event={row.first}
                      ref={virtualizer.measureElement}
                      style={{ position: 'absolute', top: 0, left: 0, right: 0, transform: `translateY(${v.start}px)` }}
                    >
                      {e.k === 'turn-end' ? (
                        // A turn's time and tokens show when you hover its reply, not on every turn.
                        <div aria-hidden className="h-6" />
                      ) : (
                        <div className={cn('group/turn mx-auto max-w-[760px] px-6 py-1.5', row.first === 0 && 'pt-5')}>
                          {row.last > row.first ? (
                            <SubagentGroup subs={events.slice(row.first, row.last + 1) as Extract<TEvent, { k: 'subagent' }>[]} onOpen={setSub} />
                          ) : (
                            <EventView e={e} onOpenSub={setSub} forceOpen={openEvents.has(row.first) && (e.k === 'tools' || e.k === 'thinking')} />
                          )}
                          {e.k === 'assistant' && events[row.first + 1]?.k === 'turn-end' && (
                            <div className="mt-1 opacity-0 transition-opacity duration-150 group-hover/turn:opacity-100 focus-within:opacity-100">
                              <TurnEnd e={events[row.first + 1] as Extract<TEvent, { k: 'turn-end' }>} />
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
            </div>
            {outline && (
              <nav aria-label="你說過的話" className="thin-scroll hidden w-[220px] shrink-0 overflow-y-auto border-l py-3 min-[1180px]:block">
                <h2 className="px-4 pb-2 text-[12px] font-semibold text-muted-foreground">你說過的話 · {num(prompts.length)}</h2>
                {prompts.map((idx) => {
                  const e = events[idx] as Extract<TEvent, { k: 'user' }>
                  return (
                    <button
                      key={idx}
                      type="button"
                      onClick={() => scrollToEvent(idx, 'start')}
                      className={cn(
                        'flex w-full gap-2 px-4 py-1 text-left text-[12px] leading-snug hover:bg-muted',
                        e.turn === topTurn ? 'text-foreground' : 'text-muted-foreground',
                      )}
                    >
                      <span className="w-6 shrink-0 text-right tabular opacity-60">{e.turn + 1}</span>
                      <span className="line-clamp-2">{e.text}</span>
                    </button>
                  )
                })}
              </nav>
            )}
          </>
        )}
        {info && (
          <aside aria-label="這段對話的資訊" className="thin-scroll w-[300px] shrink-0 overflow-y-auto border-l bg-background/30">
            <InfoPanel chat={chat} />
          </aside>
        )}
      </div>
      <SubagentSheet sub={sub} onClose={() => setSub(null)} />
    </div>
  )
}
