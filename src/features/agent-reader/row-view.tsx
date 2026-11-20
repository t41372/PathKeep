/**
 * One row of the conversation as the reader draws it (`agent-history.md`
 * §4.5): a divider where the context was compacted or edited, a prompt in
 * a bubble with its versions and attachments, a thought folded to one
 * line, the reply in Markdown, tool calls folded to one line with the
 * subagents they sent, what the tool noted (an interruption, a retry, an
 * error), and at a turn's end, while the pointer or focus is in the turn,
 * its time, tokens and model.
 */
import {
  AlertTriangle,
  Brain,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleSlash,
  Info,
  Minimize2,
  OctagonX,
} from 'lucide-react'
import { memo, useState, type ReactNode } from 'react'
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
import { Markdown } from '@/features/ask/markdown'
import type {
  ChatChildCard,
  ChatNotice,
  ChatRow,
  ChatTurnFooter,
} from '@/lib/backend-client/agent-chats'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters, type MessageKey } from '@/lib/i18n'
import { useChatVersions } from '@/lib/queries/agent-chats'
import { Attachments } from './attachments'
import { durationText, tokenTotal, tokensText } from './reader-words'
import {
  COLUMN,
  hasText,
  isPrompt,
  rowKey,
  textOf,
  thinkingMs,
  thinkingOf,
  toolCalls,
  turnKey,
  type ToolCall,
} from './row-model'
import { SubagentCards, ToolGroup } from './tool-group'

export interface RowContext {
  id: string
  publication: string
  f: Formatters
  /** Subagents by the row that sent them (`launchesOf`). */
  launches: Map<string, ChatChildCard[]>
  onOpenChild: (child: ChatChildCard) => void
  /** Picks another version of a prompt (`sourceId:ordinal`): the line that ends at `leaf`. */
  onPickVersion: (leaf: number, promptKey: string) => void
  /** The prompt whose earlier version is shown, and how long its branch is. */
  switched: { key: string; records: number } | null
  /** The turn under the pointer or focus: its footer shows. */
  activeTurn: string | null
}

function Boundary({ row }: { row: ChatRow }) {
  const { t, locale } = useI18n()
  const [open, setOpen] = useState(false)
  const boundary = row.boundary
  if (!boundary) return null
  if (boundary.kind !== 'compaction')
    return (
      <p className="flex items-center gap-3 py-1 text-[12px] text-muted-foreground">
        <span className="h-px flex-1 bg-border" aria-hidden />
        {boundary.kind === 'context_edit'
          ? t('agentReader.rows.contextEdit')
          : t('agentReader.rows.missingParent')}
        <span className="h-px flex-1 bg-border" aria-hidden />
      </p>
    )
  const summary = textOf(row)
  const tokens = boundary.tokens_before
  return (
    <div className="flex flex-col gap-1.5 py-1">
      <div className="flex items-center gap-3 text-[12px] text-muted-foreground">
        <span className="h-px flex-1 bg-border" aria-hidden />
        <Minimize2 className="size-3.5" aria-hidden />
        {tokens != null
          ? t('agentReader.rows.compaction', {
              tokens: new Intl.NumberFormat(locale, {
                notation: 'compact',
                maximumFractionDigits: 1,
              }).format(tokens),
            })
          : t('agentReader.rows.compactionPlain')}
        <span className="h-px flex-1 bg-border" aria-hidden />
      </div>
      <p className="text-center text-[12px] text-muted-foreground">
        {t('agentReader.rows.compactionBody')}
        {summary && (
          <button
            type="button"
            aria-expanded={open}
            className="ml-1 text-foreground underline underline-offset-2"
            onClick={() => setOpen((value) => !value)}
          >
            {t(
              open
                ? 'agentReader.rows.hideSummary'
                : 'agentReader.rows.showSummary',
            )}
          </button>
        )}
      </p>
      {open && summary && (
        <div className="animate-rise mx-auto max-w-[560px] rounded-lg border bg-muted/50 p-3 text-[12.5px] leading-relaxed whitespace-pre-wrap text-muted-foreground">
          {summary}
        </div>
      )}
    </div>
  )
}

function VersionSwitch({
  context,
  row,
}: {
  context: RowContext
  row: ChatRow
}) {
  const { t } = useI18n()
  const versions = useChatVersions(
    context.id,
    context.publication,
    row.sourceId,
    row.ordinal,
    (row.versionCount ?? 0) > 1,
  )
  const list = versions.data?.versions
  const items = list?.items ?? []
  const at = items.findIndex((item) => item.ordinal === row.ordinal)
  const count = list?.count ?? row.versionCount ?? 0
  const pick = (index: number) => {
    const item = items[index]
    if (item)
      context.onPickVersion(item.leaf, `${row.sourceId}:${item.ordinal}`)
  }
  return (
    <span className="flex items-center gap-0.5 rounded-md border px-1 py-px">
      <button
        type="button"
        aria-label={t('agentReader.rows.previousVersion')}
        disabled={at <= 0}
        onClick={() => pick(at - 1)}
        className="disabled:opacity-30"
      >
        <ChevronLeft className="size-3.5" aria-hidden />
      </button>
      <span className="tabular">
        {t('agentReader.rows.version', {
          current: at < 0 ? '–' : at + 1,
          count,
        })}
      </span>
      <button
        type="button"
        aria-label={t('agentReader.rows.nextVersion')}
        disabled={at < 0 || at >= items.length - 1}
        onClick={() => pick(at + 1)}
        className="disabled:opacity-30"
      >
        <ChevronRight className="size-3.5" aria-hidden />
      </button>
    </span>
  )
}

function Prompt({ context, row }: { context: RowContext; row: ChatRow }) {
  const { t } = useI18n()
  const switched = context.switched?.key === rowKey(row)
  return (
    <div className="flex flex-col items-end gap-1.5">
      <div className="flex items-center gap-2 text-[11.5px] text-muted-foreground">
        {(row.versionCount ?? 0) > 1 && (
          <VersionSwitch context={context} row={row} />
        )}
        <span>
          {row.at === null
            ? t('agentReader.rows.youUndated')
            : t('agentReader.rows.you', { time: context.f.time(row.at) })}
        </span>
      </div>
      <div className="max-w-[85%] rounded-[18px] bg-user-bubble px-4 py-2.5 text-[14px] leading-normal whitespace-pre-wrap text-user-bubble-foreground [overflow-wrap:anywhere]">
        {textOf(row)}
      </div>
      {switched && context.switched && (
        <p className="animate-rise max-w-[85%] text-right text-[12px] text-muted-foreground">
          {t('agentReader.rows.branch', { count: context.switched.records })}
        </p>
      )}
      <Attachments
        id={context.id}
        publication={context.publication}
        attachments={row.attachments}
      />
    </div>
  )
}

function Thinking({ row }: { row: ChatRow }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const ms = thinkingMs(row)
  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger className="group flex items-center gap-1.5 text-[12.5px] text-muted-foreground transition-colors duration-150 hover:text-foreground">
        <Brain className="size-3.5" aria-hidden />
        {ms === null
          ? t('agentReader.rows.thinking')
          : t('agentReader.rows.thought', { duration: durationText(t, ms) })}
        <ChevronDown
          className="size-3 transition-transform duration-200 group-data-[state=open]:rotate-180"
          aria-hidden
        />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <p className="mt-1.5 border-l-2 pl-3 text-[13px] leading-relaxed whitespace-pre-wrap text-muted-foreground italic">
          {thinkingOf(row)}
        </p>
      </CollapsibleContent>
    </Collapsible>
  )
}

const noticeLook = {
  info: { icon: Info, className: 'bg-muted text-muted-foreground' },
  warning: {
    icon: AlertTriangle,
    className: 'bg-muted text-foreground ring-1 ring-foreground/15',
  },
  error: {
    icon: OctagonX,
    className: 'bg-destructive/10 text-destructive',
  },
}

/**
 * The spec's three looks (`agent-history.md` §4.5): an interruption is
 * plain, a retry a warning, an API error an error; anything else as the
 * backend rates it.
 */
function noticeTone(notice: ChatNotice): keyof typeof noticeLook {
  if (notice.code === 'interrupted') return 'info'
  if (notice.code === 'retry') return 'warning'
  if (notice.code === 'apiError') return 'error'
  if (notice.tone === 'error') return 'error'
  return notice.tone === 'warning' ? 'warning' : 'info'
}

function Notice({ notice }: { notice: ChatNotice }) {
  const { t } = useI18n()
  const look = noticeLook[noticeTone(notice)]
  const Icon = notice.code === 'interrupted' ? CircleSlash : look.icon
  const known: Record<string, MessageKey> = {
    interrupted: 'agentReader.rows.notice.interrupted',
    retry: 'agentReader.rows.notice.retry',
    apiError: 'agentReader.rows.notice.apiError',
  }
  const text = known[notice.code]
    ? t(known[notice.code])
    : t('agentReader.rows.notice.other', { code: notice.code })
  return (
    <div
      className={cn(
        'flex items-center gap-2 rounded-md px-2.5 py-1.5 text-[12.5px]',
        look.className,
      )}
    >
      <Icon className="size-3.5 shrink-0" aria-hidden />
      {text}
    </div>
  )
}

function Footer({
  footer,
  f,
  shown,
}: {
  footer: ChatTurnFooter
  f: Formatters
  shown: boolean
}) {
  const { t } = useI18n()
  const facts = footer.facts
  const ms =
    facts.agentMs ??
    (footer.startedAt !== null && footer.endedAt !== null
      ? footer.endedAt - footer.startedAt
      : null)
  const duration = ms === null ? '–' : durationText(t, ms)
  const tokens = facts.tokens
  const usage = tokens ? tokensText(t, f, tokenTotal(tokens)) : null
  const text =
    footer.endReason === 'interrupted'
      ? [t('agentReader.rows.interrupted', { duration }), usage]
          .filter(Boolean)
          .join(' · ')
      : footer.model
        ? t('agentReader.rows.footer', {
            duration,
            usage: usage ?? t('agentReader.rows.noUsage'),
            model: footer.model,
          })
        : t('agentReader.rows.footerNoModel', {
            duration,
            usage: usage ?? t('agentReader.rows.noUsage'),
          })
  const line = (
    <span
      tabIndex={0}
      className="rounded px-1.5 py-0.5 outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50"
    >
      {text}
    </span>
  )
  return (
    <div
      data-turn-footer=""
      className={cn(
        'mt-1 flex justify-end text-[11.5px] text-muted-foreground tabular transition-opacity duration-150 focus-within:opacity-100',
        shown ? 'opacity-100' : 'opacity-0',
      )}
    >
      {tokens ? (
        <Tooltip>
          <TooltipTrigger asChild>{line}</TooltipTrigger>
          <TooltipContent className="tabular">
            {t('agentReader.rows.tokenKinds', {
              output: f.number(tokens.output),
              input: f.number(tokens.input),
              write: f.number(tokens.cacheWrite),
              read: f.number(tokens.cacheRead),
            })}
          </TooltipContent>
        </Tooltip>
      ) : (
        line
      )}
    </div>
  )
}

/** A row in a role the reader has no look for (system, developer, …). */
function OtherText({ row }: { row: ChatRow }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger className="group flex items-center gap-1.5 font-mono text-[11.5px] text-muted-foreground hover:text-foreground">
        {t('agentReader.rows.other', { role: row.role ?? row.kind })}
        <ChevronDown
          className="size-3 transition-transform duration-200 group-data-[state=open]:rotate-180"
          aria-hidden
        />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <p className="mt-1.5 rounded-md bg-muted/60 p-2.5 text-[12.5px] leading-relaxed whitespace-pre-wrap text-muted-foreground [overflow-wrap:anywhere]">
          {textOf(row)}
        </p>
      </CollapsibleContent>
    </Collapsible>
  )
}

/**
 * How a row stands in a run of tool-only rows (`row-model.ts`): it starts
 * one (`calls` of all its rows), it continues one (it draws only its
 * notices and its turn's footer), or it is on its own.
 */
export type RunPlace =
  | { kind: 'start'; calls: ToolCall[]; rows: ChatRow[] }
  | { kind: 'continued' }
  | { kind: 'alone' }

export const RowView = memo(function RowView({
  row,
  place,
  context,
  first,
}: {
  row: ChatRow
  place: RunPlace
  context: RowContext
  /** The conversation's first row: room above it. */
  first: boolean
}) {
  const parts: ReactNode[] = []
  if (place.kind === 'start') {
    parts.push(<ToolGroup key="tools" calls={place.calls} />)
  } else if (place.kind === 'alone') {
    if (isPrompt(row))
      parts.push(<Prompt key="prompt" context={context} row={row} />)
    else {
      if (row.parts.some((part) => part.kind === 'thinking'))
        parts.push(<Thinking key="thinking" row={row} />)
      if (hasText(row))
        parts.push(
          row.role === 'assistant' ? (
            <Markdown key="text" content={textOf(row)} streaming={false} />
          ) : (
            <OtherText key="text" row={row} />
          ),
        )
      const calls = toolCalls([row])
      if (calls.length > 0) parts.push(<ToolGroup key="tools" calls={calls} />)
    }
  }
  if (place.kind !== 'continued') {
    const members = place.kind === 'start' ? place.rows : [row]
    const cards = members.flatMap(
      (member) => context.launches.get(rowKey(member)) ?? [],
    )
    if (cards.length > 0)
      parts.push(
        <SubagentCards
          key="subagents"
          cards={cards}
          onOpen={context.onOpenChild}
        />,
      )
  }
  for (const notice of row.notices)
    parts.push(
      <Notice
        key={`notice-${notice.ordinal}-${notice.code}`}
        notice={notice}
      />,
    )
  const turn = turnKey(row)
  if (parts.length === 0 && !row.boundary && !row.footer) return null
  return (
    <div className={cn(COLUMN, 'flex flex-col gap-2', first && 'pt-5')}>
      <Boundary row={row} />
      {parts}
      {row.footer && (
        <Footer
          footer={row.footer}
          f={context.f}
          shown={turn !== null && context.activeTurn === turn}
        />
      )}
    </div>
  )
})
