/**
 * Tool calls and subagents in the reader (`agent-history.md` §4.5). A
 * step's calls fold into one line, "Used 3 tools · changed +12 −3 lines",
 * with a dot when some did not succeed; open, it lists each call, and a
 * call opens its diff or its output. A subagent is a card that opens its
 * own transcript; several sent off together fold into one line.
 */
import {
  Bot,
  ChevronDown,
  ChevronRight,
  FilePen,
  FileSearch,
  SquareTerminal,
  Wrench,
} from 'lucide-react'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import type { ChatChildCard } from '@/lib/backend-client/agent-chats'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useReveal } from './find-target'
import { durationText } from './reader-words'
import { callArgument, type ToolCall } from './row-model'

function CallIcon({ call }: { call: ToolCall }) {
  const name = (call.name ?? '').toLowerCase()
  const className = 'size-3.5 shrink-0 text-muted-foreground'
  if (call.edit) return <FilePen className={className} aria-hidden />
  if (/bash|shell|exec|command|terminal/.test(name))
    return <SquareTerminal className={className} aria-hidden />
  if (/read|grep|glob|search|find|ls|list/.test(name))
    return <FileSearch className={className} aria-hidden />
  return <Wrench className={className} aria-hidden />
}

function failed(call: ToolCall) {
  return call.status === 'error' || call.status === 'denied'
}

function Diff({ text }: { text: string }) {
  return (
    <pre className="max-h-[320px] overflow-auto rounded-md bg-muted p-2.5 font-mono text-[11.5px] leading-[1.55]">
      {text.split('\n').map((line, index) => (
        <div
          key={index}
          className={cn(
            line.startsWith('+') && 'bg-green/12 text-green',
            line.startsWith('-') && 'bg-red/10 text-red',
          )}
        >
          {line || ' '}
        </div>
      ))}
    </pre>
  )
}

function CallDetail({ call }: { call: ToolCall }) {
  const { t } = useI18n()
  if (call.edit?.diff)
    return (
      <>
        <Diff text={call.edit.diff} />
        {call.edit.truncated && (
          <p className="mt-1 text-[12px] text-muted-foreground">
            {t('agentReader.rows.shortened')}
          </p>
        )}
      </>
    )
  if (call.output?.trim())
    return (
      <>
        <pre
          data-part={call.outputToken ?? undefined}
          className="max-h-[260px] overflow-auto rounded-md bg-muted p-2.5 font-mono text-[11.5px] leading-[1.55] whitespace-pre-wrap [overflow-wrap:anywhere]"
        >
          {call.output}
        </pre>
        {call.truncated && (
          <p className="mt-1 text-[12px] text-muted-foreground">
            {t('agentReader.rows.shortened')}
          </p>
        )}
      </>
    )
  return (
    <p className="text-[12px] text-muted-foreground">
      {t('agentReader.rows.noOutput')}
    </p>
  )
}

function CallRow({ call }: { call: ToolCall }) {
  const { t } = useI18n()
  const [open, setOpen] = useReveal(callTokens([call]))
  const added = call.edit?.addedLines
  const removed = call.edit?.removedLines
  return (
    <li>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="flex w-full min-w-0 items-center gap-2 rounded-md px-2 py-1 text-left text-[12.5px] transition-colors duration-150 hover:bg-muted"
      >
        <CallIcon call={call} />
        {call.name && (
          <span className="shrink-0 font-mono text-[11.5px] font-medium">
            {call.name}
          </span>
        )}
        <span
          data-part={call.inputToken ?? undefined}
          className="truncate font-mono text-[11.5px] text-muted-foreground"
        >
          {callArgument(call.input)}
        </span>
        <span className="ml-auto flex shrink-0 items-center gap-2 text-[11.5px] text-muted-foreground tabular">
          {added != null && removed != null && (
            <span>
              <span className="text-green">+{added}</span>{' '}
              <span className="text-red">−{removed}</span>
            </span>
          )}
          {call.status === 'error' && (
            <span className="text-destructive">
              {t('agentReader.rows.failed')}
            </span>
          )}
          {call.status === 'denied' && (
            <span className="text-foreground">
              {t('agentReader.rows.denied')}
            </span>
          )}
          {call.durationMs !== null && durationText(t, call.durationMs)}
        </span>
      </button>
      {open && (
        <div className="animate-rise mt-1 mb-2 ml-7">
          <CallDetail call={call} />
        </div>
      )}
    </li>
  )
}

function callTokens(calls: ToolCall[]) {
  return calls.flatMap((call) =>
    [call.inputToken, call.outputToken].filter((token) => token !== null),
  )
}

export function ToolGroup({ calls }: { calls: ToolCall[] }) {
  const { t } = useI18n()
  const [open, setOpen] = useReveal(callTokens(calls))
  if (calls.length === 0) return null
  const counts = new Map<string, number>()
  for (const call of calls) {
    if (!call.name) continue
    counts.set(call.name, (counts.get(call.name) ?? 0) + 1)
  }
  const summary = [...counts]
    .map(([name, count]) => (count > 1 ? `${name} ×${count}` : name))
    .join(' · ')
  let added = 0
  let removed = 0
  for (const call of calls) {
    added += call.edit?.addedLines ?? 0
    removed += call.edit?.removedLines ?? 0
  }
  const notOk = calls.filter(failed).length
  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="rounded-lg border bg-card/50"
    >
      <CollapsibleTrigger className="group flex w-full min-w-0 items-center gap-2 px-3 py-2 text-left text-[12.5px]">
        <Wrench
          className="size-3.5 shrink-0 text-muted-foreground"
          aria-hidden
        />
        <span className="shrink-0 text-muted-foreground">
          {t('agentReader.rows.tools', { count: calls.length })}
          {added + removed > 0 &&
            ` · ${t('agentReader.rows.changed', { added, removed })}`}
        </span>
        <span className="ml-auto flex shrink-0 items-center gap-2.5 text-[11.5px] text-muted-foreground">
          {notOk > 0 && (
            <Tooltip>
              <TooltipTrigger asChild>
                <span
                  role="img"
                  aria-label={t('agentReader.rows.notSucceeded', {
                    count: notOk,
                  })}
                  className="size-1.5 rounded-full bg-destructive"
                />
              </TooltipTrigger>
              <TooltipContent>
                {t('agentReader.rows.notSucceeded', { count: notOk })}
              </TooltipContent>
            </Tooltip>
          )}
          <ChevronDown
            className="size-3.5 transition-transform duration-200 group-data-[state=open]:rotate-180"
            aria-hidden
          />
        </span>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <p className="border-t px-3 pt-2 font-mono text-[11.5px] text-muted-foreground">
          {summary}
          {notOk > 0 && (
            <span className="ml-2 font-sans text-destructive">
              {t('agentReader.rows.notSucceeded', { count: notOk })}
            </span>
          )}
        </p>
        <ul className="px-1.5 py-1.5">
          {calls.map((call) => (
            <CallRow key={call.key} call={call} />
          ))}
        </ul>
      </CollapsibleContent>
    </Collapsible>
  )
}

function SubagentCard({
  child,
  onOpen,
}: {
  child: ChatChildCard
  onOpen: (child: ChatChildCard) => void
}) {
  const { t } = useI18n()
  return (
    <button
      type="button"
      onClick={() => onOpen(child)}
      className="flex w-full min-w-0 items-center gap-2.5 rounded-lg border bg-card/60 px-3 py-2 text-left transition-colors duration-150 hover:bg-card"
    >
      <Bot className="size-4 shrink-0 text-muted-foreground" aria-hidden />
      <span className="shrink-0 text-[12px] text-muted-foreground">
        {t('agentReader.rows.subagent')}
      </span>
      <span
        data-part={`title:${child.id}`}
        className="truncate text-[13px] font-medium"
      >
        {child.title?.trim() || t('agentReader.untitled')}
      </span>
      {child.agentMs !== null && (
        <span className="ml-auto shrink-0 text-[12px] text-muted-foreground tabular">
          {durationText(t, child.agentMs)}
        </span>
      )}
      <ChevronRight
        className={cn(
          'size-4 shrink-0 text-muted-foreground',
          child.agentMs === null && 'ml-auto',
        )}
        aria-hidden
      />
    </button>
  )
}

/** One card, or several sent off together folded into one line. */
export function SubagentCards({
  cards,
  onOpen,
}: {
  cards: ChatChildCard[]
  onOpen: (child: ChatChildCard) => void
}) {
  const { t } = useI18n()
  const [open, setOpen] = useReveal(cards.map((card) => `title:${card.id}`))
  if (cards.length === 0) return null
  if (cards.length === 1)
    return <SubagentCard child={cards[0]} onOpen={onOpen} />
  const types = [
    ...new Set(cards.map((card) => card.agentType).filter(Boolean)),
  ]
  const total = cards.reduce((sum, card) => sum + (card.agentMs ?? 0), 0)
  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="rounded-lg border bg-card/60"
    >
      <CollapsibleTrigger className="group flex w-full min-w-0 items-center gap-2.5 px-3 py-2 text-left text-[13px]">
        <Bot className="size-4 shrink-0 text-muted-foreground" aria-hidden />
        <span className="shrink-0 font-medium">
          {t('agentReader.rows.subagents', { count: cards.length })}
        </span>
        {types.length > 0 && (
          <span className="truncate text-[12px] text-muted-foreground">
            · {types.join(t('agentReader.rows.listSeparator'))}
          </span>
        )}
        <span className="ml-auto shrink-0 text-[12px] text-muted-foreground tabular">
          {t('agentReader.rows.subagentsTotal', {
            duration: durationText(t, total),
          })}
        </span>
        <ChevronDown
          className="size-3.5 shrink-0 text-muted-foreground transition-transform duration-200 group-data-[state=open]:rotate-180"
          aria-hidden
        />
      </CollapsibleTrigger>
      <CollapsibleContent className="flex flex-col gap-1.5 border-t p-1.5">
        {cards.map((card) => (
          <SubagentCard key={card.id} child={card} onOpen={onOpen} />
        ))}
      </CollapsibleContent>
    </Collapsible>
  )
}
