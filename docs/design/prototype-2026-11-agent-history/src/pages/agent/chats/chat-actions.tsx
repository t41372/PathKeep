/**
 * Managing one conversation: put away (封存) and move to the trash. The
 * reader's ⋯ menu and a right-click on a list row offer the same actions.
 * Neither asks first: both only hide the conversation, and the toast can undo.
 * A trashed conversation offers only 放回.
 */
import { Archive, ArchiveRestore, Maximize2, Star, Trash2, Undo2 } from 'lucide-react'
import { ContextMenu } from 'radix-ui'
import { useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { DropdownMenuItem, DropdownMenuSeparator } from '@/components/ui/dropdown-menu'
import { archive } from '~/data/archive'
import type { Chat } from '~/data/types'
import { setArchived, setTrashed, useChatEdits } from './chat-edits'

export function putAway(chat: Chat) {
  setArchived(chat.id, true)
  toast('已封存，不再出現在清單裡', { description: '搜尋還找得到；要看全部，在「篩選」裡選「已封存」。', action: { label: '復原', onClick: () => setArchived(chat.id, false) } })
}

export function bringBack(chat: Chat) {
  setArchived(chat.id, false)
  toast('已取消封存')
}

/** `onTrashed` lets the page close the reader that shows this conversation. */
export function moveToTrash(chat: Chat, onTrashed?: (chat: Chat) => void) {
  setTrashed(chat.id, true)
  onTrashed?.(chat)
  toast('已移到垃圾桶', {
    description: '清單、搜尋和洞察都不再算它。要看垃圾桶，在「篩選」裡選「垃圾桶」。',
    action: { label: '復原', onClick: () => setTrashed(chat.id, false) },
  })
}

/** Out of the trash; its put-away state is whatever it was before. */
export function putBack(chat: Chat) {
  setTrashed(chat.id, false)
  toast('已放回')
}

/** The end of the reader's ⋯ menu: 封存 / 取消封存 and 移到垃圾桶, or 放回 for a trashed one. */
export function ChatManageItems({ chat, onTrashed }: { chat: Chat; onTrashed?: (chat: Chat) => void }) {
  const { archived, trashed } = useChatEdits()
  if (trashed.has(chat.id))
    return (
      <>
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={() => putBack(chat)}>
          <Undo2 />
          放回
        </DropdownMenuItem>
      </>
    )
  return (
    <>
      <DropdownMenuSeparator />
      {archived.has(chat.id) ? (
        <DropdownMenuItem onSelect={() => bringBack(chat)}>
          <ArchiveRestore />
          取消封存
        </DropdownMenuItem>
      ) : (
        <DropdownMenuItem onSelect={() => putAway(chat)}>
          <Archive />
          封存
        </DropdownMenuItem>
      )}
      <DropdownMenuItem onSelect={() => moveToTrash(chat, onTrashed)}>
        <Trash2 />
        移到垃圾桶
      </DropdownMenuItem>
    </>
  )
}

const menuItem =
  'relative flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground [&_svg]:size-4 [&_svg]:shrink-0 [&_svg]:text-muted-foreground'

/**
 * Right-click menu for the rows of a conversation list. One menu for the whole
 * list: rows carry `data-chat-id`, and the row under the pointer decides which
 * conversation the menu is about. A right-click elsewhere in the list does nothing.
 */
export function ChatRowContextMenu({ children, onOpen, onTrashed }: { children: ReactNode; onOpen: (chat: Chat) => void; onTrashed?: (chat: Chat) => void }) {
  const [chat, setChat] = useState<Chat | null>(null)
  const { archived, trashed } = useChatEdits()
  const navigate = useNavigate()
  const inTrash = !!chat && trashed.has(chat.id)
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger
        asChild
        onContextMenu={(event) => {
          const row = (event.target as HTMLElement).closest<HTMLElement>('[data-chat-id]')
          const found = row ? archive.chatById.get(Number(row.dataset.chatId)) : undefined
          if (found) setChat(found)
          else event.preventDefault()
        }}
      >
        {children}
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="z-50 w-[220px] overflow-hidden rounded-md border bg-popover p-1 text-popover-foreground shadow-md data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95">
          {chat && inTrash && (
            <>
              <ContextMenu.Item className={menuItem} onSelect={() => onOpen(chat)}>
                打開
              </ContextMenu.Item>
              <ContextMenu.Item className={menuItem} onSelect={() => putBack(chat)}>
                <Undo2 />
                放回
              </ContextMenu.Item>
            </>
          )}
          {chat && !inTrash && (
            <>
              <ContextMenu.Item className={menuItem} onSelect={() => onOpen(chat)}>
                打開
              </ContextMenu.Item>
              <ContextMenu.Item className={menuItem} onSelect={() => void navigate(`/agent/chat/${chat.id}`)}>
                <Maximize2 />
                全頁打開
              </ContextMenu.Item>
              <ContextMenu.Item className={menuItem} onSelect={() => toast(chat.starred ? '已取消星號' : '已加星號')}>
                <Star />
                {chat.starred ? '取消星號' : '加星號'}
              </ContextMenu.Item>
              <ContextMenu.Separator className="-mx-1 my-1 h-px bg-border" />
              {archived.has(chat.id) ? (
                <ContextMenu.Item className={menuItem} onSelect={() => bringBack(chat)}>
                  <ArchiveRestore />
                  取消封存
                </ContextMenu.Item>
              ) : (
                <ContextMenu.Item className={menuItem} onSelect={() => putAway(chat)}>
                  <Archive />
                  封存
                </ContextMenu.Item>
              )}
              <ContextMenu.Item className={menuItem} onSelect={() => moveToTrash(chat, onTrashed)}>
                <Trash2 />
                移到垃圾桶
              </ContextMenu.Item>
            </>
          )}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  )
}
