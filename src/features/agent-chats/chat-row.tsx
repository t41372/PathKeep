/**
 * One conversation in the list, kept to what helps pick it
 * (`agent-history.md` §3.3): its title and time, then its tool and project,
 * and on the right a star, "only in PathKeep", put away, and the number the
 * list is sorted by. Tokens, time, subagents and the rest are in the
 * reader's info panel.
 */
import { Archive, FolderClosed, ShieldCheck, Star } from 'lucide-react'
import { memo } from 'react'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import type { ChatSort, ChatSummary } from '@/lib/backend-client/agent-chats'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters } from '@/lib/i18n'
import {
  chatTitle,
  fromFirstPrompt,
  metricText,
  rowInstant,
  rowWhen,
} from './chat-words'

/** "Only in PathKeep": the tool deleted it, the archive keeps it. */
export function OnlyInPathKeepMark({ label }: { label?: string }) {
  const { t } = useI18n()
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          tabIndex={-1}
          role="img"
          aria-label={t('agentChats.row.onlyInPathKeep')}
          className="inline-flex shrink-0 items-center gap-1 rounded-md bg-muted px-1 py-px text-[11px] font-medium text-foreground/75"
        >
          <ShieldCheck className="size-3" aria-hidden />
          {label}
        </span>
      </TooltipTrigger>
      <TooltipContent className="max-w-[260px]">
        {t('agentChats.row.onlyInPathKeep')}
      </TooltipContent>
    </Tooltip>
  )
}

/** Put away by the person; with `withText` it says so in words. */
export function PutAwayMark({ withText }: { withText?: boolean }) {
  const { t } = useI18n()
  return (
    <span
      role="img"
      aria-label={t('agentChats.row.archived')}
      className="inline-flex shrink-0 items-center gap-1 rounded-md bg-muted px-1 py-px text-[11px] font-medium text-muted-foreground"
    >
      <Archive className="size-3" aria-hidden />
      {withText && <span aria-hidden>{t('agentChats.row.archived')}</span>}
    </span>
  )
}

export const ChatRowView = memo(function ChatRowView({
  chat,
  selected,
  onSelect,
  showProject,
  sort,
  clockOnly,
  zone,
  f,
}: {
  chat: ChatSummary
  selected: boolean
  onSelect: (chat: ChatSummary) => void
  showProject: boolean
  sort: ChatSort
  /** Under a day heading the row shows only the clock. */
  clockOnly: boolean
  zone: string
  f: Formatters
}) {
  const { t } = useI18n()
  const metric = metricText(t, f, sort, chat)
  return (
    <div className="px-2 py-[2px]">
      <button
        type="button"
        data-chat-id={chat.id}
        aria-current={selected ? 'true' : undefined}
        tabIndex={-1}
        onClick={() => onSelect(chat)}
        className={cn(
          'flex h-[56px] w-full min-w-0 flex-col justify-center gap-1 rounded-lg px-3 text-left transition-colors duration-150',
          selected ? 'bg-card shadow-card' : 'hover:bg-foreground/[0.035]',
        )}
      >
        <span className="flex w-full min-w-0 items-center gap-2">
          <span
            className={cn(
              'truncate text-[13.5px]',
              fromFirstPrompt(chat.titleSource) ? 'font-normal' : 'font-medium',
            )}
          >
            {chatTitle(t, chat)}
          </span>
          <span className="ml-auto shrink-0 text-xs text-muted-foreground tabular">
            {rowWhen(t, f, zone, rowInstant(chat, sort), clockOnly)}
          </span>
        </span>
        <span className="flex w-full min-w-0 items-center gap-1.5 text-[12px] text-muted-foreground">
          <ToolIcon id={chat.harness} />
          <span className="sr-only">{t(toolName(chat.harness))}</span>
          {showProject && chat.project && (
            <span className="flex min-w-0 items-center gap-1">
              <FolderClosed className="size-3 shrink-0" aria-hidden />
              <span className="truncate">{chat.project.label}</span>
            </span>
          )}
          <span className="ml-auto flex shrink-0 items-center gap-1.5">
            {chat.starred && (
              <Star
                className="size-3 fill-brand text-brand"
                role="img"
                aria-label={t('agentChats.row.starred')}
              />
            )}
            {chat.onlyInPathKeep && <OnlyInPathKeepMark />}
            {chat.archived && <PutAwayMark />}
            {metric && <span className="tabular">{metric}</span>}
          </span>
        </span>
      </button>
    </div>
  )
})
