/**
 * 對話: the list on the left (or streamed full-text results while a search
 * is active), the reader on the right. The same workbench shows one project
 * when opened from 專案 (`projectId`), with that project's header on top.
 * Filters, search and the selected conversation live in the URL.
 */
import { MessagesSquare } from 'lucide-react'
import { useMemo, type ReactNode } from 'react'
import { Kbd } from '@/components/ui/kbd'
import type { RowMetric } from '~/components/chat-row'
import { archive } from '~/data/archive'
import { ChatDialogs, ChatRowContextMenu } from './chats/chat-actions'
import { useChatEdits } from './chats/chat-edits'
import { applyFilters, useFilters, type Filters, type Sort } from './chats/filters'
import { ChatList } from './chats/list'
import { SearchResults } from './chats/results'
import { Toolbar } from './chats/toolbar'
import { Reader } from './reader/reader'

const metricFor: Partial<Record<Sort, RowMetric>> = { agent: 'agent', tokens: 'tokens', turns: 'turns' }

function EmptyReader({ scoped }: { scoped: boolean }) {
  return (
    <div className="flex flex-1 items-center justify-center p-10">
      <div className="flex max-w-[360px] flex-col items-center gap-3 text-center">
        <MessagesSquare className="size-8 text-muted-foreground/70" strokeWidth={1.5} />
        <p className="text-[14px] font-medium">選一段對話來看</p>
        <div className="flex flex-col gap-1.5 text-[12.5px] text-muted-foreground">
          <span>
            <Kbd>↑</Kbd> <Kbd>↓</Kbd> 切換對話
          </span>
          <span>
            <Kbd>/</Kbd> {scoped ? '搜尋這個專案' : '搜尋全部對話'}
          </span>
          <span>
            <Kbd>⌘K</Kbd> 跳到任何地方
          </span>
        </div>
      </div>
    </div>
  )
}

export function ChatsWorkbench({ projectId, header }: { projectId?: string; header?: ReactNode }) {
  const { filters: raw, update } = useFilters()
  // Opened from a project: that project is fixed, not a filter you can clear.
  const filters = useMemo<Filters>(() => (projectId ? { ...raw, project: projectId, group: raw.group === 'project' ? 'day' : raw.group } : raw), [raw, projectId])
  const edits = useChatEdits()
  const chats = useMemo(
    () => applyFilters(filters, edits),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [filters.project, filters.harnesses.join(), filters.range, filters.flags.join(), filters.sort, edits.archived, edits.deleted],
  )
  const selected = filters.chat && !edits.deleted.has(filters.chat) ? archive.chatById.get(filters.chat) : undefined
  const open = (chat: { id: number }) => update({ chat: chat.id, find: undefined, at: undefined })
  const scoped = !!projectId

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      {header}
      <div className="flex min-h-0 min-w-0 flex-1">
        <section aria-label="對話列表" className="flex w-[400px] shrink-0 flex-col border-r max-[1240px]:w-[360px]">
          <Toolbar filters={filters} update={update} scoped={scoped} />
          {filters.q ? (
            <SearchResults filters={filters} update={update} onOpen={(chat, at) => update({ chat, find: filters.q, at })} />
          ) : (
            <ChatRowContextMenu onOpen={open}>
              <div className="flex min-h-0 flex-1 flex-col">
                <ChatList
                  chats={chats}
                  group={filters.group}
                  selected={filters.chat}
                  metric={metricFor[filters.sort]}
                  scoped={scoped}
                  onSelect={open}
                  onPickProject={(project) => update({ project, group: 'day' }, false)}
                />
              </div>
            </ChatRowContextMenu>
          )}
        </section>
        {selected ? (
          <Reader key={selected.id} chat={selected} find={filters.find} at={filters.at} onClose={() => update({ chat: undefined, find: undefined, at: undefined })} />
        ) : (
          <EmptyReader scoped={scoped} />
        )}
      </div>
      <ChatDialogs onDeleted={(chat) => chat.id === filters.chat && update({ chat: undefined, find: undefined, at: undefined })} />
    </div>
  )
}

export default function ChatsPage() {
  return <ChatsWorkbench />
}
