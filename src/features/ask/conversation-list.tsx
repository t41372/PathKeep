/**
 * Left column of Ask: new chat button and saved conversations grouped by day,
 * with rename and delete (delete is confirmed).
 *
 * Not responsible for loading a conversation's messages or the live chat.
 */
import { useQueryClient } from '@tanstack/react-query'
import { MoreHorizontal, Pencil, Plus, Trash2 } from 'lucide-react'
import { useState } from 'react'
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
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { cn } from '@/lib/cn'
import { useFormat, useI18n, startOfDay } from '@/lib/i18n'
import type { AgentConversationSummary } from '@/lib/types'
import {
  conversationsKey,
  groupByDay,
  type useConversations,
} from './conversations'

interface Props {
  conversations: ReturnType<typeof useConversations>
  activeId: string
  /** Title of the active chat while it is not saved yet. */
  draftTitle: string | null
  onSelect: (id: string) => void
  onNew: () => void
  onDeleted: (id: string) => void
}

export function ConversationList({
  conversations,
  activeId,
  draftTitle,
  onSelect,
  onNew,
  onDeleted,
}: Props) {
  const { t } = useI18n()
  const format = useFormat()
  const client = useQueryClient()
  const [renaming, setRenaming] = useState<AgentConversationSummary | null>(
    null,
  )
  const [deleting, setDeleting] = useState<AgentConversationSummary | null>(
    null,
  )

  const items = conversations.data ?? []
  const showDraft =
    draftTitle !== null && !items.some((item) => item.id === activeId)
  const groups = groupByDay(items)
  const today = startOfDay(new Date()).getTime()

  const dayLabel = (day: Date) => {
    const diff = Math.round((today - day.getTime()) / 86_400_000)
    if (diff === 0) return t('common.today')
    if (diff === 1) return t('common.yesterday')
    return format.date(day, { dateStyle: 'medium' })
  }

  const refresh = () => client.invalidateQueries({ queryKey: conversationsKey })

  return (
    <aside
      aria-label={t('ask.list.label')}
      className="flex w-[240px] shrink-0 flex-col gap-1.5 overflow-y-auto border-r p-3.5"
    >
      <Button
        variant="secondary"
        onClick={onNew}
        className="h-10 shrink-0 gap-2 rounded-[10px] bg-muted text-sm font-medium text-foreground hover:bg-muted/70"
      >
        <Plus className="size-4" />
        {t('ask.newChat')}
        <kbd className="font-mono text-[11px] text-muted-foreground">⌘N</kbd>
      </Button>

      {showDraft && (
        <div className="mt-3">
          <GroupLabel>{t('common.today')}</GroupLabel>
          <Row
            active
            title={draftTitle || t('ask.untitled')}
            time={format.time(new Date())}
          />
        </div>
      )}

      {conversations.isPending && (
        <div className="mt-3 flex flex-col gap-2">
          {[0, 1, 2].map((index) => (
            <Skeleton key={index} className="h-[52px] rounded-[10px]" />
          ))}
        </div>
      )}

      {conversations.isError && (
        <p className="px-2 pt-4 text-[13px] text-muted-foreground">
          {t('ask.list.loadFailed')}
        </p>
      )}

      {conversations.isSuccess && items.length === 0 && !showDraft && (
        <p className="px-2 pt-4 text-[13px] text-muted-foreground">
          {t('ask.list.empty')}
        </p>
      )}

      {groups.map((group, index) => (
        <div
          key={group.day.getTime()}
          className={cn(index === 0 && !showDraft ? 'mt-3' : 'mt-1')}
        >
          <GroupLabel>{dayLabel(group.day)}</GroupLabel>
          {group.items.map((item) => (
            <Row
              key={item.id}
              active={item.id === activeId}
              title={item.title || t('ask.untitled')}
              time={format.time(item.updatedAt)}
              onSelect={() => onSelect(item.id)}
              onRename={() => setRenaming(item)}
              onDelete={() => setDeleting(item)}
            />
          ))}
        </div>
      ))}

      <RenameDialog
        conversation={renaming}
        onClose={() => setRenaming(null)}
        onRenamed={refresh}
      />
      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t('ask.list.deleteTitle')}</AlertDialogTitle>
            <AlertDialogDescription>
              {t('ask.list.deleteBody', {
                title: deleting?.title || t('ask.untitled'),
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={async () => {
                if (!deleting) return
                try {
                  await intelligenceClient.deleteConversation(deleting.id)
                  onDeleted(deleting.id)
                  await refresh()
                } catch {
                  toast.error(t('ask.list.deleteFailed'))
                }
              }}
            >
              {t('ask.list.delete')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </aside>
  )
}

function GroupLabel({ children }: { children: string }) {
  return (
    <div className="px-2 pt-2.5 pb-1 text-xs text-muted-foreground">
      {children}
    </div>
  )
}

function Row({
  active,
  title,
  time,
  onSelect,
  onRename,
  onDelete,
}: {
  active: boolean
  title: string
  time: string
  onSelect?: () => void
  onRename?: () => void
  onDelete?: () => void
}) {
  const { t } = useI18n()
  return (
    <div
      className={cn(
        'group relative rounded-[10px] transition-colors',
        active ? 'bg-card shadow-card' : 'hover:bg-muted/60',
      )}
    >
      <button
        type="button"
        onClick={onSelect}
        aria-current={active ? 'true' : undefined}
        className="flex w-full flex-col gap-0.5 rounded-[10px] px-3 py-2.5 pr-8 text-left"
      >
        <span className="truncate font-medium">{title}</span>
        <span className="text-xs text-muted-foreground">{time}</span>
      </button>
      {onRename && onDelete && (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={t('ask.list.options')}
              className="absolute top-2 right-1.5 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 data-[state=open]:opacity-100"
            >
              <MoreHorizontal />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={onRename}>
              <Pencil />
              {t('ask.list.rename')}
            </DropdownMenuItem>
            <DropdownMenuItem variant="destructive" onSelect={onDelete}>
              <Trash2 />
              {t('ask.list.delete')}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </div>
  )
}

function RenameDialog({
  conversation,
  onClose,
  onRenamed,
}: {
  conversation: AgentConversationSummary | null
  onClose: () => void
  onRenamed: () => void
}) {
  const { t } = useI18n()
  return (
    <Dialog
      open={conversation !== null}
      onOpenChange={(open) => !open && onClose()}
    >
      <DialogContent className="sm:max-w-sm">
        {conversation && (
          <RenameForm
            key={conversation.id}
            conversation={conversation}
            onDone={() => {
              onClose()
              onRenamed()
            }}
            onCancel={onClose}
            labels={{
              title: t('ask.list.renameTitle'),
              field: t('ask.list.renameLabel'),
              cancel: t('common.cancel'),
              save: t('common.save'),
              failed: t('ask.list.renameFailed'),
            }}
          />
        )}
      </DialogContent>
    </Dialog>
  )
}

function RenameForm({
  conversation,
  onDone,
  onCancel,
  labels,
}: {
  conversation: AgentConversationSummary
  onDone: () => void
  onCancel: () => void
  labels: {
    title: string
    field: string
    cancel: string
    save: string
    failed: string
  }
}) {
  const [title, setTitle] = useState(conversation.title)
  const [saving, setSaving] = useState(false)
  const valid = title.trim().length > 0

  const submit = async () => {
    if (!valid || saving) return
    setSaving(true)
    try {
      await intelligenceClient.renameConversation({
        id: conversation.id,
        title: title.trim(),
      })
      onDone()
    } catch {
      toast.error(labels.failed)
      setSaving(false)
    }
  }

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        void submit()
      }}
      className="flex flex-col gap-4"
    >
      <DialogHeader>
        <DialogTitle>{labels.title}</DialogTitle>
      </DialogHeader>
      <Input
        autoFocus
        aria-label={labels.field}
        value={title}
        maxLength={120}
        onChange={(event) => setTitle(event.target.value)}
      />
      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onCancel}>
          {labels.cancel}
        </Button>
        <Button type="submit" disabled={!valid || saving}>
          {labels.save}
        </Button>
      </DialogFooter>
    </form>
  )
}
