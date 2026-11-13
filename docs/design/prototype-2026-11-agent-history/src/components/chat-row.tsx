/**
 * One conversation in a list, kept to what helps you pick it: title and
 * time, then the harness icon and the project. A star or "only here" adds one
 * small icon. Numbers (tokens, agent time, turns) appear only when the list
 * is sorted by them; everything else is in the reader.
 */
import { Archive, ShieldCheck, Star } from 'lucide-react'
import { memo, type CSSProperties } from 'react'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { projectOf } from '~/data/archive'
import { totalTokens } from '~/data/fixture'
import { big, duration, num, time, when } from '~/data/format'
import type { Chat } from '~/data/types'
import { useChatEdits } from '~/pages/agent/chats/chat-edits'
import { HarnessIcon, ProjectIcon } from './bits'

export const CHAT_ROW_HEIGHT = 60

export type RowMetric = 'agent' | 'tokens' | 'turns'

/** Put away by the user: shown where put-away conversations still appear (search, the 已封存 filter, the reader). */
export function ArchivedFlag({ withText }: { withText?: boolean }) {
  return (
    <span className="inline-flex shrink-0 items-center gap-1 rounded-md bg-muted px-1 py-px text-[11px] font-medium text-muted-foreground" aria-label="已封存">
      <Archive className="size-3" />
      {withText && '已封存'}
    </span>
  )
}

export function OnlyHereFlag({ compact }: { compact?: boolean }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="inline-flex shrink-0 items-center gap-1 rounded-md bg-amber-500/12 px-1 py-px text-[11px] font-medium text-amber-700 dark:text-amber-300">
          <ShieldCheck className="size-3" />
          {!compact && '只剩這裡有'}
        </span>
      </TooltipTrigger>
      <TooltipContent className="max-w-[260px]">工具自己的資料夾裡已經沒有這段對話，只有 PathKeep 的存檔還留著。</TooltipContent>
    </Tooltip>
  )
}

function metricText(chat: Chat, metric: RowMetric) {
  if (metric === 'agent') return duration(chat.agentMs, true)
  if (metric === 'turns') return `${num(chat.turns)} 輪`
  return chat.tokens ? `${big(totalTokens(chat.tokens))} token` : '沒有紀錄'
}

export const ChatRow = memo(function ChatRow({
  chat,
  selected,
  onSelect,
  showProject = true,
  metric,
  timeOnly,
  style,
}: {
  chat: Chat
  selected?: boolean
  onSelect?: (chat: Chat) => void
  showProject?: boolean
  /** Show this number on the right; set when the list is sorted by it. */
  metric?: RowMetric
  /** The list already groups by day, so the row shows only the time. */
  timeOnly?: boolean
  style?: CSSProperties
}) {
  const project = projectOf(chat)
  const archived = useChatEdits().archived.has(chat.id)
  return (
    <div style={style} className="px-2 py-[2px]">
      <button
        type="button"
        data-chat-id={chat.id}
        onClick={() => onSelect?.(chat)}
        aria-current={selected ? 'true' : undefined}
        className={cn(
          'flex h-[56px] w-full min-w-0 flex-col justify-center gap-1 rounded-lg px-3 text-left transition-colors',
          selected ? 'bg-card shadow-card' : 'hover:bg-foreground/[0.035]',
        )}
      >
        <span className="flex w-full min-w-0 items-center gap-2">
          <span className={cn('truncate text-[13.5px] font-medium', chat.titleSource === 'first-prompt' && 'font-normal')}>{chat.title}</span>
          <span className="ml-auto shrink-0 text-xs text-muted-foreground tabular">{timeOnly ? time(chat.ended) : when(chat.ended)}</span>
        </span>
        <span className="flex w-full min-w-0 items-center gap-1.5 text-[12px] text-muted-foreground">
          <HarnessIcon id={chat.harness} className="size-3.5 [&_img]:size-3.5" />
          {showProject && (
            <span className="flex min-w-0 items-center gap-1 truncate">
              <ProjectIcon project={project} className="size-3" />
              <span className="truncate">{project.name}</span>
            </span>
          )}
          <span className="ml-auto flex shrink-0 items-center gap-1.5">
            {chat.starred && <Star className="size-3 fill-brand text-brand" aria-label="已加星" />}
            {chat.source === 'gone' && <OnlyHereFlag compact />}
            {archived && <ArchivedFlag />}
            {metric && <span className="tabular">{metricText(chat, metric)}</span>}
          </span>
        </span>
      </button>
    </div>
  )
})
