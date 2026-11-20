/**
 * The reader's ⋯ menu (`agent-history.md` §4.3), in the spec's order: find
 * in this conversation, the outline, the raw records, jump to the end;
 * open as a full page (beside the list only), rename, show the tool's file
 * in the file manager (not when the tool's file is gone); put away or
 * bring back, move to the trash or put back.
 *
 * Not here yet: "Export ▸", which needs a command the backend does not
 * have yet.
 */
import {
  Archive,
  ArchiveRestore,
  ArrowDownToLine,
  Ellipsis,
  FolderOpen,
  Maximize2,
  Pencil,
  TextSearch,
  Trash2,
  Undo2,
} from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import type { ChatMarks } from '@/features/agent-chats/chat-marks'
import { supportClient } from '@/lib/backend-client/support'
import type { ChatHeader } from '@/lib/backend-client/agent-chats'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { shortcutLabel } from '@/lib/runtime'
import { agentChatPath } from '@/lib/spaces'
import { revealLabel } from './reader-words'

export function MoreMenu({
  header,
  beside,
  outline,
  raw,
  sourcePath,
  marks,
  onOpenChange,
  onOutline,
  onRaw,
  onEnd,
  onRename,
  onTrashed,
  onFind,
}: {
  header: ChatHeader
  beside: boolean
  outline: boolean
  raw: boolean
  /** The tool's file, once the info is read; `undefined` while it is read. */
  sourcePath: string | null | undefined
  marks: ChatMarks
  onOpenChange: (open: boolean) => void
  onOutline: (on: boolean) => void
  onRaw: (on: boolean) => void
  onEnd: () => void
  onRename: () => void
  onTrashed: () => void
  onFind: () => void
}) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const summary = header.summary
  const canReveal = !summary.onlyInPathKeep && Boolean(sourcePath)
  return (
    <DropdownMenu onOpenChange={onOpenChange}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t('agentReader.header.more')}
          className="flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground"
        >
          <Ellipsis className="size-4" aria-hidden />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-[260px]">
        <DropdownMenuItem onSelect={onFind}>
          <TextSearch aria-hidden />
          {t('agentReader.menu.find')}
          <DropdownMenuShortcut>{shortcutLabel('F')}</DropdownMenuShortcut>
        </DropdownMenuItem>
        <DropdownMenuCheckboxItem checked={outline} onCheckedChange={onOutline}>
          {t('agentReader.menu.outline')}
        </DropdownMenuCheckboxItem>
        <DropdownMenuCheckboxItem checked={raw} onCheckedChange={onRaw}>
          {t('agentReader.menu.raw', { count: header.physicalRecords })}
        </DropdownMenuCheckboxItem>
        <DropdownMenuItem onSelect={onEnd}>
          <ArrowDownToLine aria-hidden />
          {t('agentReader.menu.end')}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {beside && (
          <DropdownMenuItem
            onSelect={() => void navigate(agentChatPath(summary.id))}
          >
            <Maximize2 aria-hidden />
            {t('agentReader.menu.openPage')}
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onSelect={onRename}>
          <Pencil aria-hidden />
          {t('agentReader.menu.rename')}
        </DropdownMenuItem>
        <DropdownMenuItem
          disabled={!canReveal}
          onSelect={() =>
            void supportClient
              .openPathInFileManager(sourcePath!)
              .catch((error: unknown) =>
                toast.error(t('agentReader.menu.revealFailed'), {
                  description: describeError(error),
                }),
              )
          }
        >
          <FolderOpen aria-hidden />
          {t(revealLabel())}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {summary.archived ? (
          <DropdownMenuItem onSelect={() => void marks.bringBack(summary.id)}>
            <ArchiveRestore aria-hidden />
            {t('agentChats.menu.bringBack')}
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem onSelect={() => void marks.putAway(summary.id)}>
            <Archive aria-hidden />
            {t('agentChats.menu.putAway')}
          </DropdownMenuItem>
        )}
        {summary.trashed ? (
          <DropdownMenuItem onSelect={() => void marks.putBack(summary.id)}>
            <Undo2 aria-hidden />
            {t('agentChats.menu.putBack')}
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem
            onSelect={() => void marks.trash(summary.id, onTrashed)}
          >
            <Trash2 aria-hidden />
            {t('agentChats.menu.trash')}
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
