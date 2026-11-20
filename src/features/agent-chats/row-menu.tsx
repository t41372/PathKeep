/**
 * The right-click menu of the list's rows (`agent-history.md` §3.3): open
 * beside the list or as a full page, star, put away or bring back, move to
 * the trash, or, on a row in the trash, put back. One menu for the whole
 * list: rows carry `data-chat-id`, and the row under the pointer decides
 * which conversation it is about; a right-click anywhere else does nothing.
 */
import {
  Archive,
  ArchiveRestore,
  Maximize2,
  PanelRight,
  Star,
  Trash2,
  Undo2,
} from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import type { ChatSummary } from '@/lib/backend-client/agent-chats'
import { useI18n } from '@/lib/i18n'
import { agentChatPath } from '@/lib/spaces'
import type { ChatMarks } from './chat-marks'

export function RowMenu({
  find,
  marks,
  onOpen,
  onTrashed,
  children,
}: {
  /** The conversation a row id names, among the rows loaded. */
  find: (id: string) => ChatSummary | undefined
  marks: ChatMarks
  onOpen: (chat: ChatSummary) => void
  /** It went to the trash: close it if it is open. */
  onTrashed: (chat: ChatSummary) => void
  children: ReactNode
}) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const [chat, setChat] = useState<ChatSummary | null>(null)
  return (
    <ContextMenu>
      <ContextMenuTrigger
        asChild
        onContextMenu={(event) => {
          const row = (event.target as HTMLElement).closest<HTMLElement>(
            '[data-chat-id]',
          )
          const found = row?.dataset.chatId
            ? find(row.dataset.chatId)
            : undefined
          if (found) setChat(found)
          else event.preventDefault()
        }}
      >
        <div className="flex min-h-0 flex-1 flex-col">{children}</div>
      </ContextMenuTrigger>
      <ContextMenuContent className="w-[220px]">
        {chat && (
          <>
            <ContextMenuItem onSelect={() => onOpen(chat)}>
              <PanelRight />
              {t('agentChats.menu.open')}
            </ContextMenuItem>
            <ContextMenuItem
              onSelect={() => void navigate(agentChatPath(chat.id))}
            >
              <Maximize2 />
              {t('agentChats.menu.openPage')}
            </ContextMenuItem>
            <ContextMenuItem
              onSelect={() => void marks.star(chat.id, !chat.starred)}
            >
              <Star />
              {t(
                chat.starred
                  ? 'agentChats.menu.unstar'
                  : 'agentChats.menu.star',
              )}
            </ContextMenuItem>
            <ContextMenuSeparator />
            {chat.archived ? (
              <ContextMenuItem onSelect={() => void marks.bringBack(chat.id)}>
                <ArchiveRestore />
                {t('agentChats.menu.bringBack')}
              </ContextMenuItem>
            ) : (
              <ContextMenuItem onSelect={() => void marks.putAway(chat.id)}>
                <Archive />
                {t('agentChats.menu.putAway')}
              </ContextMenuItem>
            )}
            {chat.trashed ? (
              <ContextMenuItem onSelect={() => void marks.putBack(chat.id)}>
                <Undo2 />
                {t('agentChats.menu.putBack')}
              </ContextMenuItem>
            ) : (
              <ContextMenuItem
                onSelect={() =>
                  void marks.trash(chat.id, () => onTrashed(chat))
                }
              >
                <Trash2 />
                {t('agentChats.menu.trash')}
              </ContextMenuItem>
            )}
          </>
        )}
      </ContextMenuContent>
    </ContextMenu>
  )
}
