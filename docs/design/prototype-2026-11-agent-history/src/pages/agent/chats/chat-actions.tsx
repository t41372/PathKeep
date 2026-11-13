/**
 * Managing one conversation: put away (封存) and delete from PathKeep.
 * The reader's ⋯ menu and a right-click on a list row offer the same two
 * actions and open the same delete dialog, which says exactly what goes,
 * what stays in the tool's own folder, and that it can't be undone.
 */
import { Archive, ArchiveRestore, Maximize2, Star, Trash2 } from 'lucide-react'
import { ContextMenu } from 'radix-ui'
import { useState, type ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { DropdownMenuItem, DropdownMenuSeparator } from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/cn'
import { HarnessIcon } from '~/components/bits'
import { archive, harnesses, projectOf } from '~/data/archive'
import { bytes, date, num } from '~/data/format'
import type { Chat } from '~/data/types'
import { askDelete, deleteChat, setArchived, useChatEdits } from './chat-edits'

/** How many versions of this conversation PathKeep has kept (invented here). */
const versionsOf = (chat: Chat) => 1 + (chat.id % 4)

export function putAway(chat: Chat) {
  setArchived(chat.id, true)
  toast('已封存，不再出現在清單裡', { description: '搜尋還找得到；要看全部，在「篩選」裡選「已封存」。', action: { label: '復原', onClick: () => setArchived(chat.id, false) } })
}

export function bringBack(chat: Chat) {
  setArchived(chat.id, false)
  toast('已取消封存')
}

/** 封存 / 取消封存 and 從 PathKeep 刪除…, for the reader's ⋯ menu. */
export function ChatManageItems({ chat }: { chat: Chat }) {
  const { archived } = useChatEdits()
  const isArchived = archived.has(chat.id)
  return (
    <>
      <DropdownMenuSeparator />
      {isArchived ? (
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
      <DropdownMenuItem variant="destructive" onSelect={() => askDelete(chat.id)}>
        <Trash2 />
        從 PathKeep 刪除…
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
export function ChatRowContextMenu({ children, onOpen }: { children: ReactNode; onOpen: (chat: Chat) => void }) {
  const [chat, setChat] = useState<Chat | null>(null)
  const { archived } = useChatEdits()
  const navigate = useNavigate()
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
          {chat && (
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
              <ContextMenu.Item
                className={cn(menuItem, 'text-destructive data-[highlighted]:bg-destructive/10 data-[highlighted]:text-destructive [&_svg]:text-destructive')}
                onSelect={() => askDelete(chat.id)}
              >
                <Trash2 />
                從 PathKeep 刪除…
              </ContextMenu.Item>
            </>
          )}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  )
}

/** The delete dialog. Mount once per page; `onDeleted` lets the page close the reader. */
export function ChatDialogs({ onDeleted }: { onDeleted?: (chat: Chat) => void }) {
  const { deleting, archived } = useChatEdits()
  const chat = deleting === null ? undefined : archive.chatById.get(deleting)
  if (!chat) return null
  const tool = harnesses[chat.harness].name
  const project = projectOf(chat)
  const onlyCopy = chat.source === 'gone'
  const parts = ['對話內容', chat.subagentCount > 0 ? `${num(chat.subagentCount)} 個 subagent` : null, `PathKeep 存過的 ${versionsOf(chat)} 個版本`, '搜尋索引裡的內容'].filter(Boolean)
  return (
    <AlertDialog open onOpenChange={(open) => !open && askDelete(null)}>
      <AlertDialogContent className="sm:max-w-[480px]">
        <AlertDialogHeader>
          <AlertDialogTitle>從 PathKeep 刪除這段對話？</AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div className="flex flex-col gap-3 text-left text-[13px] leading-relaxed text-muted-foreground">
              <div className="flex min-w-0 items-center gap-2 rounded-lg bg-muted px-3 py-2 text-foreground">
                <HarnessIcon id={chat.harness} className="size-4 [&_img]:size-4" />
                <span className="min-w-0 flex-1 truncate font-medium">{chat.title}</span>
                <span className="shrink-0 text-[12px] text-muted-foreground tabular">
                  {project.name} · {date(chat.started, true)}
                </span>
              </div>
              <p>
                會刪掉：{parts.join('、')}，<span className="whitespace-nowrap">約 {bytes(chat.sizeBytes)}</span>。
              </p>
              {onlyCopy ? (
                <p className="rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-foreground">
                  這是唯一的一份。{tool} 已經刪掉這段對話，從 PathKeep 刪除後就找不回來了。
                </p>
              ) : (
                <p>
                  {tool} 自己的資料夾裡還有這段對話，PathKeep 不會動它，之後備份也不會再把它存回來。要連那邊一起刪，請到 {tool} 裡刪。
                </p>
              )}
              <p>
                刪除後無法復原。
                {!archived.has(chat.id) && (
                  <>
                    只是不想在清單裡看到它，可以
                    <button
                      type="button"
                      className="text-foreground underline underline-offset-2 hover:text-brand"
                      onClick={() => {
                        askDelete(null)
                        putAway(chat)
                      }}
                    >
                      改為封存
                    </button>
                    。
                  </>
                )}
              </p>
            </div>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>取消</AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            onClick={() => {
              deleteChat(chat.id)
              onDeleted?.(chat)
              toast('已從 PathKeep 刪除這段對話')
            }}
          >
            刪除對話
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
