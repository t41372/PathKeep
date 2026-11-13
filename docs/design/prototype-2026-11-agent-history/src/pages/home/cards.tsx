/** Home cards. Each one is drawn from the data-type registry, so a new type shows up without touching this file. */
import { ArrowRight, Bot, Copy, HardDriveDownload, X } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Link, useNavigate, useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import { SectionCard } from '@/components/app/section-card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { HarnessBadge } from '~/components/bits'
import { ChatRow } from '~/components/chat-row'
import { archive, harnesses, harnessOrder, projectOf, resumeCommand } from '~/data/archive'
import { big, date, hours, num, pct } from '~/data/format'
import type { Chat } from '~/data/types'
import { kindById, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'
import { YearGrid, YearStepper } from '../agent/insights/parts'
import { kindStatus, onThisDay, week, yearCounts } from './data'

export function BackupPill() {
  return (
    <Link to="/backup" className="flex h-9 items-center gap-2 rounded-lg border bg-card px-3 text-[13px] shadow-card hover:bg-card/90">
      <span className="size-2 rounded-full bg-green" />
      上次備份 今天 15:02
    </Link>
  )
}

export function BackupNow() {
  const { enabledKinds } = useProto()
  return (
    <Button
      onClick={() =>
        toast.success('開始備份', { description: enabledKinds.map((k) => kindById[k].label).join('、') || '沒有開啟的資料類型' })
      }
    >
      <HardDriveDownload />
      立即備份
    </Button>
  )
}

/** Upgrade case: shown until the person opens the preview or dismisses it. Nothing has been read yet. */
export function AgentIntroCard() {
  const [, setParams] = useSearchParams()
  const [hidden, setHidden] = useState(false)
  if (hidden) return null
  return (
    <section className="relative flex gap-4 rounded-xl border border-brand/25 bg-brand-soft p-5">
      <span className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-card shadow-card">
        <Bot className="size-5" strokeWidth={1.75} />
      </span>
      <div className="flex min-w-0 flex-col gap-2">
        <h2 className="text-[15px] font-semibold">PathKeep 現在也能保存 Agent 對話</h2>
        <p className="max-w-[640px] text-[13px] leading-relaxed text-muted-foreground">
          Claude Code 預設 30 天後刪掉舊對話，Codex 的紀錄也常被清掉。開啟後，它們會存進同一個存檔，照專案、時間和全文都找得回來。
        </p>
        <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px]">
          這台電腦上有這些工具的資料夾：
          {harnessOrder.slice(0, 3).map((h) => (
            <HarnessBadge key={h} id={h} withName />
          ))}
        </p>
        <p className="text-xs text-muted-foreground">只看了資料夾在不在，還沒有讀任何內容。</p>
        <div className="mt-1 flex gap-2">
          <Button onClick={() => setParams({ enable: 'agent' })}>預覽會保存什麼</Button>
          <Button variant="ghost" onClick={() => setHidden(true)}>
            以後再說
          </Button>
        </div>
      </div>
      <button
        type="button"
        aria-label="關閉"
        onClick={() => setHidden(true)}
        className="absolute top-3 right-3 rounded-md p-1 text-muted-foreground hover:bg-card hover:text-foreground"
      >
        <X className="size-4" />
      </button>
    </section>
  )
}

export function StatusCards() {
  const { enabledKinds } = useProto()
  if (enabledKinds.length === 0) return null
  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-[15px] font-semibold">保存了什麼</h2>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
        {enabledKinds.map((id) => {
          const kind = kindById[id]
          const s = kindStatus[id]
          return (
            <Link
              key={id}
              to={kind.pages[0].path}
              className="flex min-w-0 flex-col gap-1.5 rounded-xl border bg-card px-[18px] py-4 shadow-card transition-colors hover:bg-card/90"
            >
              <span className="flex items-center gap-1.5 text-[13px] text-muted-foreground">
                <kind.icon className="size-3.5" strokeWidth={1.75} />
                {kind.label}
                {s.sample && (
                  <Badge variant="outline" className="ml-auto px-1.5 py-0 text-[10px] font-normal">
                    示意
                  </Badge>
                )}
              </span>
              <span className="flex items-baseline gap-1.5">
                <span className="text-2xl font-semibold tracking-[-0.02em] tabular">{s.value}</span>
                <span className="text-[13px] text-muted-foreground">{s.unit}</span>
              </span>
              <span className="text-xs text-muted-foreground">{s.sub}</span>
              {s.onlyHere && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span className="w-fit rounded-md bg-amber-500/12 px-1.5 py-px text-[11px] font-medium text-amber-700 dark:text-amber-300">
                      {s.onlyHere}
                    </span>
                  </TooltipTrigger>
                  <TooltipContent className="max-w-[260px]">{kind.why}；這些在原本的地方已經沒有了，只有 PathKeep 的存檔還留著。</TooltipContent>
                </Tooltip>
              )}
            </Link>
          )
        })}
      </div>
    </section>
  )
}

function copyResume(chat: Chat) {
  const cmd = resumeCommand(chat)
  void navigator.clipboard?.writeText(cmd).catch(() => undefined)
  toast.success('已複製接著做的指令', { description: <code className="font-mono text-xs break-all">{cmd}</code> })
}

export function ContinueCard() {
  const navigate = useNavigate()
  const latest = archive.chats.slice(0, 4)
  return (
    <SectionCard
      title="接著做"
      subtitle="最近的對話。複製指令貼到終端機，就能在原本的工具裡接著聊"
      action={
        <Link to="/agent/chats" className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground">
          所有對話 <ArrowRight className="size-3" />
        </Link>
      }
      className="gap-2"
    >
      <ul className="-mx-3 flex flex-col">
        {latest.map((chat) => (
          <li key={chat.id} className="flex items-center gap-1 pr-2">
            <div className="min-w-0 flex-1">
              <ChatRow chat={chat} onSelect={(c) => void navigate(`/agent/chats?chat=${c.id}`)} />
            </div>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon" className="size-8 shrink-0 text-muted-foreground" aria-label="複製接著做的指令" onClick={() => copyResume(chat)}>
                  <Copy className="size-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>複製 {harnesses[chat.harness].name} 接著做的指令</TooltipContent>
            </Tooltip>
          </li>
        ))}
      </ul>
    </SectionCard>
  )
}

export function WeekCard() {
  const max = Math.max(0.1, ...week.days.map((d) => d.hours))
  const change = week.lastBusy ? week.busy / week.lastBusy - 1 : 0
  return (
    <SectionCard title="這週的 Agent" subtitle="週一到現在" action={
      <Link to="/agent/insights" className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground">
        洞察 <ArrowRight className="size-3" />
      </Link>
    }>
      <div className="flex flex-col gap-1">
        <span className="text-[13px] text-muted-foreground">Agent 工作時間</span>
        <span className="flex items-baseline gap-2">
          <span className="text-2xl font-semibold tracking-[-0.02em] tabular">{hours(week.busy)}</span>
          {week.lastBusy > 0 && (
            <span
              title="跟上週一到上週同一時刻比"
              className={cn('rounded-md px-1.5 py-0.5 text-xs font-medium tabular', change >= 0 ? 'bg-brand-soft text-brand' : 'bg-muted text-muted-foreground')}
            >
              {change >= 0 ? '+' : ''}
              {pct(change)} 比上週同期
            </span>
          )}
        </span>
        <span className="text-xs text-muted-foreground">
          分開加總 {hours(week.summed)} · 平均同時開 {week.concurrency.toFixed(1)} 個
        </span>
      </div>
      <div className="flex h-[72px] items-end gap-2" aria-label="這週每天的 Agent 工作時間">
        {week.days.map((d) => (
          <div key={d.label} className="flex flex-1 flex-col items-center gap-1" title={d.future ? '' : `週${d.label}：${hours(d.hours * 3_600_000)}`}>
            <span className="flex h-[52px] w-full items-end">
              <span
                className={cn('w-full rounded-[3px]', d.future ? 'bg-transparent' : d.hours ? 'bg-brand/80' : 'bg-muted')}
                style={{ height: d.future ? 0 : `${Math.max(6, (d.hours / max) * 100)}%` }}
              />
            </span>
            <span className={cn('text-[11px]', d.future ? 'text-muted-foreground/50' : 'text-muted-foreground')}>{d.label}</span>
          </div>
        ))}
      </div>
      <dl className="grid grid-cols-2 gap-3 border-t pt-3 text-[13px]">
        <div>
          <dt className="text-xs text-muted-foreground">Token 總量</dt>
          <dd className="font-medium tabular">{big(week.tokens)}</dd>
          {week.known < week.chats && <dd className="text-xs text-muted-foreground">{week.known} / {week.chats} 段有紀錄</dd>}
        </div>
        <div>
          <dt className="text-xs text-muted-foreground">對話</dt>
          <dd className="font-medium tabular">{num(week.chats)} 段</dd>
          <dd className="text-xs text-muted-foreground">另有 {num(week.subagents)} 個 subagent</dd>
        </div>
      </dl>
    </SectionCard>
  )
}

const inventedOnThisDay: Partial<Record<KindId, string>> = {
  browsing: '瀏覽了 214 頁，最多在 github.com、docs.rs',
  terminal: '執行了 386 條指令，最多是 git 和 cargo',
  messages: '收到 92 則訊息，多在 #release 頻道',
  webchat: '2 段 ChatGPT 對話',
  notifications: '61 則通知',
}

export function OnThisDayCard() {
  const { enabledKinds } = useProto()
  const navigate = useNavigate()
  return (
    <SectionCard title="往年今日" subtitle="以前的這一天，你在做什麼">
      {onThisDay.length === 0 && <p className="text-[13px] text-muted-foreground">存檔還不到一年。</p>}
      {onThisDay.map(({ key, chats }) => (
        <div key={key} className="flex flex-col gap-2">
          <span className="text-xs font-medium text-muted-foreground">{date(Date.parse(key) + 4 * 3600_000, true)}</span>
          <ul className="flex flex-col gap-1.5 text-[13px]">
            {enabledKinds.map((id) => {
              const kind = kindById[id]
              if (id === 'agent')
                return (
                  <li key={id} className="flex flex-col gap-1">
                    <span className="flex items-center gap-1.5 text-muted-foreground">
                      <kind.icon className="size-3.5" />
                      {chats.length ? `${chats.length} 段 Agent 對話` : '那天沒有 Agent 對話'}
                    </span>
                    {chats.slice(0, 3).map((chat) => (
                      <button
                        key={chat.id}
                        type="button"
                        onClick={() => void navigate(`/agent/chats?chat=${chat.id}`)}
                        className="ml-5 flex min-w-0 items-center gap-2 rounded-md px-1.5 py-1 text-left hover:bg-foreground/[0.035]"
                      >
                        <HarnessBadge id={chat.harness} />
                        <span className="truncate">{chat.title}</span>
                        <span className="ml-auto shrink-0 text-xs text-muted-foreground">{projectOf(chat).name}</span>
                      </button>
                    ))}
                  </li>
                )
              return (
                <li key={id} className="flex items-center gap-1.5 text-muted-foreground">
                  <kind.icon className="size-3.5" />
                  {inventedOnThisDay[id]}
                </li>
              )
            })}
          </ul>
        </div>
      ))}
    </SectionCard>
  )
}

const yearMeasure: Record<KindId, { title: string; cell: (v: number) => string; total: (sum: number, year: number, days: number) => string }> = {
  agent: {
    title: 'Agent 工作時間',
    cell: (v) => (v >= 1 ? `${v.toFixed(1)} 小時` : `${Math.round(v * 60)} 分`),
    total: (sum, year, days) => `${year} 年共 ${hours(sum * 3_600_000)}，${days} 天有對話`,
  },
  browsing: { title: '每天瀏覽的頁數', cell: (v) => `${num(v)} 頁`, total: (sum, year) => `${year} 年共 ${big(sum)} 頁` },
  terminal: { title: '每天執行的指令', cell: (v) => `${num(v)} 條`, total: (sum, year) => `${year} 年共 ${big(sum)} 條` },
  messages: { title: '每天收發的訊息', cell: (v) => `${num(v)} 則`, total: (sum, year) => `${year} 年共 ${big(sum)} 則` },
  webchat: { title: '每天的 AI 網頁對話', cell: (v) => `${num(v)} 段`, total: (sum, year) => `${year} 年共 ${big(sum)} 段` },
  notifications: { title: '每天的通知', cell: (v) => `${num(v)} 則`, total: (sum, year) => `${year} 年共 ${big(sum)} 則` },
}

export function YearCard() {
  const { enabledKinds } = useProto()
  const navigate = useNavigate()
  const [picked, setPicked] = useState<KindId>(enabledKinds.includes('agent') ? 'agent' : enabledKinds[0])
  const kind = enabledKinds.includes(picked) ? picked : enabledKinds[0]
  const [year, setYear] = useState(2026)
  const counts = useMemo(() => yearCounts(kind, year), [kind, year])
  const measure = yearMeasure[kind]
  const label = useMemo(() => (key: string, v: number) => (v > 0 ? `${key} · ${measure.cell(v)}` : `${key} · 沒有`), [measure])
  const sum = [...counts.values()].reduce((s, v) => s + v, 0)
  return (
    <SectionCard
      title="這一年"
      subtitle={`${measure.title}${kindById[kind].real ? '' : '（示意）'}`}
      action={
        <div className="flex items-center gap-2">
          {enabledKinds.length > 1 && (
            <ToggleGroup type="single" size="sm" variant="outline" value={kind} onValueChange={(v) => v && setPicked(v as KindId)}>
              {enabledKinds.map((id) => (
                <ToggleGroupItem key={id} value={id} className="px-2 text-xs" aria-label={kindById[id].label}>
                  {kindById[id].short}
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
          )}
          <YearStepper year={year} min={2025} max={2026} onChange={setYear} />
        </div>
      }
    >
      <YearGrid
        year={year}
        counts={counts}
        label={label}
        summary={measure.total(sum, year, counts.size)}
        onDay={kind === 'agent' ? (key) => void navigate(`/agent/chats?day=${key}`) : undefined}
      />
    </SectionCard>
  )
}

const inventedSources: Partial<Record<KindId, { name: string; detail: string }[]>> = {
  browsing: [
    { name: 'Chrome', detail: '3 個設定檔 · 121 萬筆' },
    { name: 'Safari', detail: '48 萬筆' },
    { name: 'Arc', detail: '13 萬筆' },
  ],
  terminal: [{ name: 'zsh', detail: '~/.zsh_history' }, { name: 'Atuin', detail: '~/.local/share/atuin' }],
  messages: [{ name: 'Slack', detail: '2 個工作區' }],
  webchat: [{ name: 'ChatGPT 匯出檔', detail: '上次匯入 10 月 2 日' }],
  notifications: [{ name: 'macOS 通知中心', detail: '每小時讀一次' }],
}

export function SourcesCard() {
  const { enabledKinds } = useProto()
  const agentCounts = useMemo(() => {
    const m = new Map<string, number>()
    for (const c of archive.chats) m.set(c.harness, (m.get(c.harness) ?? 0) + 1)
    return m
  }, [])
  return (
    <SectionCard
      title="來源"
      subtitle="每個資料類型從哪裡讀"
      action={
        <Link to="/backup" className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground">
          管理 <ArrowRight className="size-3" />
        </Link>
      }
    >
      <div className="flex flex-col gap-3">
        {enabledKinds.map((id) => {
          const kind = kindById[id]
          return (
            <div key={id} className="flex flex-col gap-1.5">
              <span className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                <kind.icon className="size-3.5" />
                {kind.label}
              </span>
              <ul className="flex flex-col gap-1 text-[13px]">
                {id === 'agent'
                  ? harnessOrder.map((h) => (
                      <li key={h} className="flex items-center gap-2">
                        <HarnessBadge id={h} withName />
                        <span className="truncate font-mono text-[11px] text-muted-foreground">{harnesses[h].root}</span>
                        <span className="ml-auto shrink-0 text-xs text-muted-foreground tabular">{num(agentCounts.get(h) ?? 0)} 段</span>
                      </li>
                    ))
                  : (inventedSources[id] ?? []).map((s) => (
                      <li key={s.name} className="flex items-center gap-2">
                        <span>{s.name}</span>
                        <span className="ml-auto shrink-0 text-xs text-muted-foreground">{s.detail}</span>
                      </li>
                    ))}
              </ul>
            </div>
          )
        })}
      </div>
    </SectionCard>
  )
}

export function NothingOn() {
  return (
    <SectionCard title="還沒有開啟任何資料類型">
      <p className="text-[13px] text-muted-foreground">
        在「設定 › 資料類型」選要保存的東西。可以只開一種，例如只保存 Agent 對話。
      </p>
      <div>
        <Button asChild variant="outline">
          <Link to="/settings/kinds">選擇資料類型</Link>
        </Button>
      </div>
    </SectionCard>
  )
}

