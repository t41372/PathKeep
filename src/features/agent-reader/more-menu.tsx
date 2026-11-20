/**
 * The reader's ⋯ menu (`agent-history.md` §4.3), in the spec's order: find
 * in this conversation, the outline, the raw records, jump to the end;
 * export (Markdown, HTML, JSON Lines, then the tool's original files),
 * open as a full page (beside the list only), rename, show the tool's
 * file selected in the file manager (not when the tool's file is gone);
 * put away or bring back, move to the trash or put back.
 */
import {
  Archive,
  ArchiveRestore,
  ArrowDownToLine,
  Download,
  Ellipsis,
  FolderOpen,
  Maximize2,
  Pencil,
  TextSearch,
  Trash2,
  Undo2,
} from 'lucide-react'
import { Fragment } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Spinner } from '@/components/ui/spinner'
import type { ChatMarks } from '@/features/agent-chats/chat-marks'
import { supportClient } from '@/lib/backend-client/support'
import type { ChatHeader } from '@/lib/backend-client/agent-chats'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { shortcutLabel } from '@/lib/runtime'
import { agentChatPath } from '@/lib/spaces'
import { revealLabel, toolLabel } from './reader-words'
import { EXPORT_FORMATS, formatName, useExportChat } from './use-export'

const formatHint = {
  markdown: 'agentReader.export.markdownHint',
  html: 'agentReader.export.htmlHint',
  jsonl: 'agentReader.export.jsonlHint',
  originals: 'agentReader.export.originalsHint',
} as const

export function MoreMenu({
  header,
  beside,
  outline,
  raw,
  leaf,
  sourcePath,
  marks,
  onOpenChange,
  onOutline,
  onRaw,
  onEnd,
  onRename,
  onTrashed,
  onFind,
  onStale,
}: {
  header: ChatHeader
  beside: boolean
  outline: boolean
  raw: boolean
  /** The Pi line the reader shows; `null` for the tool's own. */
  leaf: number | null
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
  /** The conversation changed under an export: read the header again. */
  onStale: () => void
}) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const summary = header.summary
  const canReveal = !summary.onlyInPathKeep && Boolean(sourcePath)
  const exporter = useExportChat({
    id: summary.id,
    publication: header.publication,
    leaf,
    onStale,
  })
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
        <DropdownMenuSub>
          <DropdownMenuSubTrigger aria-busy={exporter.running !== null}>
            {exporter.running ? (
              <Spinner aria-hidden />
            ) : (
              <Download aria-hidden />
            )}
            {exporter.running
              ? t('agentReader.export.busy')
              : t('agentReader.menu.export')}
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent className="w-[280px]">
            {EXPORT_FORMATS.map((format) => (
              <Fragment key={format}>
                {format === 'originals' && <DropdownMenuSeparator />}
                <DropdownMenuItem
                  disabled={exporter.running !== null}
                  className="flex-col items-start gap-0.5"
                  onSelect={() => void exporter.start(format)}
                >
                  <span>{t(formatName[format])}</span>
                  <span className="text-xs text-muted-foreground">
                    {t(formatHint[format], {
                      tool: toolLabel(t, summary.harness),
                    })}
                  </span>
                </DropdownMenuItem>
              </Fragment>
            ))}
          </DropdownMenuSubContent>
        </DropdownMenuSub>
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
            void supportClient.revealFile(sourcePath!).catch((error: unknown) =>
              toast.error(t('agentReader.menu.revealFileFailed'), {
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
