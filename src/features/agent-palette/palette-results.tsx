/**
 * Agent conversations' groups in ⌘K (`data-spaces.md` §6), shown from the
 * moment the palette opens:
 * - "Agent conversations": up to six. Nothing typed: the most recent, not
 *   put away. Words typed: titles that match, put-away ones marked. Each
 *   row: icon, title, project, tool and time; it opens the conversation on
 *   the History page. Last, with words typed, the row that searches the
 *   conversations for them.
 * - "Projects": up to five, the most recently active or those whose name
 *   matches; each with its main folder, opening the project's page.
 *
 * Responsibilities: drawing both groups, a quiet line while the first
 * answer is on its way, and a plain sentence when it cannot be read. Older
 * rows stay, dimmed, until the new words' answer arrives.
 * Not responsible for: the reads (`use-agent-palette.ts`).
 */
import { FolderGit2, MessagesSquare } from 'lucide-react'
import {
  CommandGroup,
  CommandItem,
  CommandShortcut,
} from '@/components/ui/command'
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import {
  agentProjectPath,
  spacePath,
  type PaletteResultsProps,
} from '@/lib/spaces'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import { chatTitle, rowWhen } from '@/features/agent-chats/chat-words'
import { useInsightsFormat, useInsightsZone } from '@/features/insights/zone'
import { useAgentPalette } from './use-agent-palette'

export default function AgentPaletteResults({
  term,
  open,
  heading,
  searchRow,
}: PaletteResultsProps) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const zone = useInsightsZone().shown
  const palette = useAgentPalette(term)
  const conversations = palette.data?.conversations ?? []
  const projects = palette.data?.projects ?? []
  const dim = palette.stale && 'opacity-60 transition-opacity'

  return (
    <>
      {(conversations.length > 0 ||
        searchRow ||
        palette.loading ||
        palette.failed) && (
        <CommandGroup heading={heading} className={cn(dim)}>
          {palette.loading && (
            <div role="status" className="flex flex-col gap-2 px-2 py-2">
              <span className="sr-only">{t('shell.palette.agentLoading')}</span>
              <Skeleton aria-hidden className="h-4 w-3/4" />
              <Skeleton aria-hidden className="h-4 w-1/2" />
            </div>
          )}
          {palette.failed && conversations.length === 0 && (
            <p
              role="status"
              className="px-2 py-1.5 text-[13px] text-muted-foreground"
            >
              {t('shell.palette.agentFailed')}
            </p>
          )}
          {conversations.map((chat) => (
            <CommandItem
              key={chat.id}
              value={`agent-chat:${chat.id}`}
              onSelect={() =>
                open(
                  spacePath(
                    'agent',
                    'chats',
                    `?chat=${encodeURIComponent(chat.id)}`,
                  ),
                )
              }
            >
              <MessagesSquare />
              <span className="min-w-0 truncate">{chatTitle(t, chat)}</span>
              {chat.archived && (
                <span className="shrink-0 text-xs text-muted-foreground">
                  {t('agentChats.row.archived')}
                </span>
              )}
              {chat.project && (
                <span className="min-w-0 truncate text-xs text-muted-foreground">
                  {chat.project.label}
                </span>
              )}
              <CommandShortcut className="flex shrink-0 items-center gap-2 tracking-normal">
                <ToolBadge id={chat.harness} className="text-xs" />
                <span className="tabular">
                  {rowWhen(t, format, zone, chat.lastActivityAt, false)}
                </span>
              </CommandShortcut>
            </CommandItem>
          ))}
          {searchRow}
        </CommandGroup>
      )}
      {projects.length > 0 && (
        <CommandGroup heading={t('shell.nav.projects')} className={cn(dim)}>
          {projects.map((project) => (
            <CommandItem
              key={project.id}
              value={`agent-project:${project.id}`}
              onSelect={() => open(agentProjectPath(project.id))}
            >
              <FolderGit2 />
              <span className="min-w-0 truncate">{project.name}</span>
              {project.mainFolder && (
                <CommandShortcut className="max-w-[50%] truncate font-mono tracking-normal">
                  {project.mainFolder}
                </CommandShortcut>
              )}
            </CommandItem>
          ))}
        </CommandGroup>
      )}
    </>
  )
}
