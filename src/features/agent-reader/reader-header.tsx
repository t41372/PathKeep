/**
 * The reader's header (`agent-history.md` §4.1): the title with star,
 * Continue, Info, ⋯ and (beside the list) close; a line of context (tool,
 * project, start, Agent time, put away); and the one banner it can have,
 * that only PathKeep still has the conversation. Under it, what is going
 * on: the conversation is in the trash, the raw records are on screen, or
 * some records could not be read into the conversation.
 */
import { FolderClosed, PanelRight, Star, Undo2, X } from 'lucide-react'
import type { ReactNode } from 'react'
import { dragRegion } from '@/components/app/drag-region'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { PutAwayMark } from '@/features/agent-chats/chat-row'
import { chatTitle, fromFirstPrompt } from '@/features/agent-chats/chat-words'
import type { ChatMarks } from '@/features/agent-chats/chat-marks'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import type { ChatHeader } from '@/lib/backend-client/agent-chats'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters } from '@/lib/i18n'
import { durationText, toolLabel } from './reader-words'

export function IconButton({
  label,
  pressed,
  onClick,
  children,
}: {
  label: string
  pressed?: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          aria-pressed={pressed}
          onClick={onClick}
          className={cn(
            'flex size-8 items-center justify-center rounded-md text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground',
            pressed && 'bg-muted text-foreground',
          )}
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}

function Line({ children }: { children: ReactNode }) {
  return (
    <p className="animate-rise flex flex-wrap items-center gap-2 text-[12.5px] text-muted-foreground">
      {children}
    </p>
  )
}

export function ReaderHeader({
  header,
  f,
  beside,
  raw,
  info,
  marks,
  resume,
  more,
  onInfo,
  onClose,
  onOpenProject,
  onRaw,
}: {
  header: ChatHeader
  f: Formatters
  beside: boolean
  raw: boolean
  info: boolean
  marks: ChatMarks
  /** Continue ▾ and ⋯, drawn by the reader. */
  resume: ReactNode
  more: ReactNode
  onInfo: () => void
  onClose?: () => void
  onOpenProject: (project: string) => void
  onRaw: (on: boolean) => void
}) {
  const { t } = useI18n()
  const summary = header.summary
  const title = chatTitle(t, summary)
  const Heading = beside ? 'h2' : 'h1'
  return (
    <header
      {...dragRegion()}
      className="flex flex-col gap-1.5 border-b px-5 pt-5 pb-3"
    >
      <div className="flex items-center gap-2">
        <Heading
          title={title}
          className={cn(
            'min-w-0 flex-1 truncate text-[16px] leading-snug tracking-[-0.01em]',
            fromFirstPrompt(summary.titleSource)
              ? 'font-medium'
              : 'font-semibold',
          )}
        >
          {title}
        </Heading>
        <div className="flex shrink-0 items-center gap-1">
          <IconButton
            label={t(
              summary.starred
                ? 'agentReader.header.unstar'
                : 'agentReader.header.star',
            )}
            pressed={summary.starred}
            onClick={() => void marks.star(summary.id, !summary.starred)}
          >
            <Star
              className={cn(
                'size-4',
                summary.starred && 'fill-brand text-brand',
              )}
              aria-hidden
            />
          </IconButton>
          {resume}
          <IconButton
            label={t('agentReader.header.info')}
            pressed={info}
            onClick={onInfo}
          >
            <PanelRight className="size-4" aria-hidden />
          </IconButton>
          {more}
          {onClose && beside && (
            <IconButton label={t('agentReader.header.close')} onClick={onClose}>
              <X className="size-4" aria-hidden />
            </IconButton>
          )}
        </div>
      </div>
      <div className="flex min-w-0 flex-wrap items-center gap-x-2.5 gap-y-1 text-[12.5px] text-muted-foreground">
        <ToolBadge id={summary.harness} className="text-[12.5px]" />
        <span aria-hidden>·</span>
        {summary.project ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={() => onOpenProject(summary.project!.id)}
                className="-mx-1 flex min-w-0 items-center gap-1 rounded px-1 transition-colors duration-150 hover:bg-muted hover:text-foreground"
              >
                <FolderClosed className="size-3.5 shrink-0" aria-hidden />
                <span className="truncate">{summary.project.label}</span>
              </button>
            </TooltipTrigger>
            <TooltipContent>
              {t('agentReader.header.openProject')}
            </TooltipContent>
          </Tooltip>
        ) : (
          <span>{t('agentReader.header.noFolder')}</span>
        )}
        {summary.startedAt !== null && (
          <>
            <span aria-hidden>·</span>
            <span className="tabular">
              {f.date(summary.startedAt, {
                dateStyle: 'long',
                timeStyle: 'short',
              })}
            </span>
          </>
        )}
        <span aria-hidden>·</span>
        <span className="tabular">
          {summary.agentMs === null
            ? t('agentReader.header.agentUnknown')
            : t('agentReader.header.agent', {
                duration: durationText(t, summary.agentMs),
              })}
        </span>
        {summary.archived && <PutAwayMark withText />}
      </div>
      {summary.onlyInPathKeep && (
        <p className="rounded-md bg-muted px-2.5 py-1.5 text-[12.5px] text-foreground">
          {t('agentReader.header.onlyInPathKeep', {
            tool: toolLabel(t, summary.harness),
            time:
              summary.lastSavedAt === null
                ? '–'
                : f.date(summary.lastSavedAt, {
                    dateStyle: 'long',
                    timeStyle: 'short',
                  }),
          })}
        </p>
      )}
      {summary.trashed && (
        <Line>
          {t('agentReader.header.inTrash')}
          <Button
            variant="outline"
            size="xs"
            onClick={() => void marks.putBack(summary.id)}
          >
            <Undo2 aria-hidden />
            {t('agentReader.header.putBack')}
          </Button>
        </Line>
      )}
      {raw ? (
        <Line>
          {t('agentReader.header.raw')}
          <button
            type="button"
            className="text-foreground underline underline-offset-2"
            onClick={() => onRaw(false)}
          >
            {t('agentReader.header.backToChat')}
          </button>
        </Line>
      ) : (
        header.unparsedRecords > 0 && (
          <Line>
            {t('agentReader.header.unparsed', {
              count: header.unparsedRecords,
            })}
            <button
              type="button"
              className="text-foreground underline underline-offset-2"
              onClick={() => onRaw(true)}
            >
              {t('agentReader.header.seeRaw')}
            </button>
          </Line>
        )
      )}
    </header>
  )
}
