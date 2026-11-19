/**
 * One tool on the backup page's sources card: its switch (the one control a
 * source has), its icon and name, the auto-delete badge, how many
 * conversations of it the archive holds, its last backup, its state and
 * how many notes its last backup left; expanded, the folders it reads, what
 * is kept of it, why it was not read in full (a folder made again where
 * the approved one was offers to look at it and approve it again), and its
 * notes by kind (the files are saved; a note is about reading them). Its ⋯
 * menu shows a folder in the file manager and rescans.
 *
 * Not responsible for: what a switch does (`use-source-changes.ts`) or the
 * rescan itself (`source-preview-dialog.tsx`).
 */
import {
  Check,
  ChevronDown,
  EllipsisVertical,
  FolderOpen,
  RefreshCw,
} from 'lucide-react'
import { useId, useState } from 'react'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Switch } from '@/components/ui/switch'
import type { AgentSource, SourcePart } from '@/lib/backend-client/agent-backup'
import { supportClient } from '@/lib/backend-client/support'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { useArchiveRun, wireNumber } from '@/lib/queries/agent-backup'
import { isMacOsHost } from '@/lib/runtime'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import type { ToolRow } from './source-rows'
import { availabilityText, isProblem, noteText } from './words'

const PARTS: { part: SourcePart; label: MessageKey }[] = [
  { part: 'conversations', label: 'agentBackup.sources.parts.conversations' },
  { part: 'subagents', label: 'agentBackup.sources.parts.subagents' },
  { part: 'attachments', label: 'agentBackup.sources.parts.attachments' },
  {
    part: 'retained-after-removal',
    label: 'agentBackup.sources.parts.retained',
  },
]

function revealLabel(): MessageKey {
  if (isMacOsHost()) return 'agentBackup.sources.reveal.mac'
  const platform = typeof navigator === 'undefined' ? '' : navigator.platform
  return /Win/i.test(platform)
    ? 'agentBackup.sources.reveal.windows'
    : 'agentBackup.sources.reveal.linux'
}

type State = { label: MessageKey; dot: string }

function stateOf(row: ToolRow): State {
  if (row.roots.length === 0 && !row.found)
    return {
      label: 'agentBackup.sources.state.notHere',
      dot: 'bg-muted-foreground/50',
    }
  if (row.roots.length === 0 || !row.enabled)
    return {
      label: 'agentBackup.sources.state.off',
      dot: 'bg-muted-foreground/50',
    }
  // Files being written are read next time; what was there is saved.
  if (row.availability === 'available' || row.availability === 'changing')
    return { label: 'agentBackup.sources.state.ok', dot: 'bg-green' }
  if (row.availability === 'stale')
    return {
      label: 'agentBackup.sources.state.notRead',
      dot: 'bg-muted-foreground/50',
    }
  // Saved as they are: a note, not a problem.
  if (row.availability === 'unknown-format')
    return { label: 'agentBackup.sources.state.format', dot: 'bg-green' }
  return { label: 'agentBackup.sources.state.problem', dot: 'bg-brand' }
}

/** The notes a tool's last backup left, by kind; its files are saved. */
function SourceNotes({ row }: { row: ToolRow }) {
  const { t } = useI18n()
  const titleId = useId()
  return (
    <section aria-labelledby={titleId} className="flex flex-col gap-1">
      <span id={titleId} className="font-medium text-foreground">
        {t('agentBackup.sources.notesTitle')}
      </span>
      <span>{t('agentBackup.sources.notesBody')}</span>
      <ul className="flex flex-col gap-0.5">
        {row.notes.map((note) => (
          <li key={note.code} data-note-code={note.code} className="tabular">
            {t('agentBackup.sources.noteGroup', {
              count: note.count,
              text: noteText(t, { code: note.code }),
            })}
            {note.newCount > 0 && (
              <span className="ml-1.5 rounded bg-muted px-1 text-[10.5px]">
                {t('agentBackup.sources.noteNew', { count: note.newCount })}
              </span>
            )}
          </li>
        ))}
      </ul>
    </section>
  )
}

function LastBackup({ runRef }: { runRef: string | undefined }) {
  const { t } = useI18n()
  const format = useFormat()
  const run = useArchiveRun(runRef ?? null)
  const first = run.data?.pages[0]
  if (!runRef) return <>{t('agentBackup.sources.never')}</>
  if (!first) return <>—</>
  return <>{format.dayAndTime(first.finishedAt ?? first.startedAt ?? 0)}</>
}

export function SourceRow({
  row,
  locked,
  pending,
  lastSourceNote,
  onSwitch,
  onRescan,
  onRenew,
}: {
  row: ToolRow
  /** The switch cannot be used now (a backup runs, or nothing is set up here). */
  locked: boolean
  pending: boolean
  /**
   * The id of the note saying it is the last source of the only kind on:
   * it cannot be switched off, and the switch says so.
   */
  lastSourceNote?: string
  onSwitch: (on: boolean) => void
  onRescan: () => void
  /** Look again at a folder made again where an approved one was. */
  onRenew: (root: AgentSource) => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const [open, setOpen] = useState(false)
  const name = t(toolName(row.tool))
  const state = stateOf(row)
  const totals = row.totals
  const conversations = wireNumber(totals?.roots)
  const subagents = wireNumber(totals?.children)
  const gone = wireNumber(totals?.onlyInPathkeep)
  const parts = new Set(row.roots.flatMap((root) => root.parts))
  const folders = row.roots.filter((root) => root.path)

  return (
    <Collapsible open={open} onOpenChange={setOpen} asChild>
      <li data-tool={row.tool} className="border-t first:border-t-0">
        <div className="flex items-center gap-3 py-2.5 pr-3 pl-4">
          <Switch
            aria-label={t('agentBackup.sources.switchLabel', { name })}
            checked={row.enabled}
            disabled={
              locked ||
              pending ||
              (row.roots.length === 0 && !row.found) ||
              (lastSourceNote !== undefined && row.enabled)
            }
            aria-describedby={lastSourceNote}
            onCheckedChange={onSwitch}
          />
          <CollapsibleTrigger
            aria-label={t('agentBackup.sources.expandLabel', { name })}
            className="group flex min-w-0 flex-1 items-center gap-3 text-left"
          >
            <span
              className={cn(
                'flex min-w-0 flex-1 flex-wrap items-center gap-2 transition-opacity',
                !row.enabled && 'opacity-60',
              )}
            >
              <ToolBadge id={row.tool} className="text-sm font-medium" />
              {row.autoDeletes && (
                <Badge
                  variant="outline"
                  className="border-brand/40 text-[11px] font-normal text-brand"
                >
                  {t('agentBackup.sources.autoDeletes')}
                </Badge>
              )}
              {row.enabled && row.noteCount > 0 && (
                <span className="text-xs text-muted-foreground tabular">
                  {t('agentBackup.sources.notes', { count: row.noteCount })}
                </span>
              )}
            </span>
            <span className="hidden w-28 shrink-0 text-right text-[13px] text-muted-foreground tabular sm:block">
              {totals
                ? t('agentBackup.sources.count', { count: conversations })
                : t('agentBackup.sources.noCount')}
            </span>
            <span className="hidden w-36 shrink-0 text-right text-[13px] text-muted-foreground tabular md:block">
              <LastBackup runRef={row.lastRun} />
            </span>
            <span className="flex w-[112px] shrink-0 items-center justify-end gap-1.5 text-xs text-muted-foreground">
              <span
                aria-hidden
                className={cn('size-1.5 rounded-full', state.dot)}
              />
              {t(state.label)}
            </span>
            <ChevronDown
              aria-hidden
              className="size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-data-[state=open]:rotate-180"
            />
          </CollapsibleTrigger>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="size-8 text-muted-foreground"
                aria-label={t('agentBackup.sources.moreLabel', { name })}
              >
                <EllipsisVertical />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {folders.map((root) => (
                <DropdownMenuItem
                  key={root.rootId}
                  onSelect={() =>
                    void supportClient
                      .openPathInFileManager(root.path!)
                      .catch((error) =>
                        toast.error(t('agentBackup.sources.reveal.failed'), {
                          description: describeError(
                            error,
                            'open_path_in_file_manager',
                          ),
                        }),
                      )
                  }
                >
                  <FolderOpen />
                  <span className="flex flex-col">
                    {t(revealLabel())}
                    {folders.length > 1 && (
                      <span className="font-mono text-[11px] text-muted-foreground">
                        {root.path}
                      </span>
                    )}
                  </span>
                </DropdownMenuItem>
              ))}
              <DropdownMenuItem onSelect={onRescan}>
                <RefreshCw />
                {t('agentBackup.sources.rescan')}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
          <div className="flex flex-col gap-2 pr-5 pb-3.5 pl-[68px] text-[12.5px] text-muted-foreground">
            {row.roots.length === 0 ? (
              <span>
                {t(
                  row.found
                    ? 'agentBackup.sources.notSetUpNote'
                    : 'agentBackup.sources.notHereNote',
                )}
              </span>
            ) : (
              !row.enabled && <span>{t('agentBackup.sources.offNote')}</span>
            )}
            {folders.map((root) => (
              <span key={root.rootId} className="flex flex-col gap-0.5">
                <span className="font-mono text-[11.5px] [overflow-wrap:anywhere]">
                  {t('agentBackup.sources.reads', { path: root.path! })}
                </span>
                {row.enabled && isProblem(root.availability) && (
                  <span className="text-foreground">
                    {availabilityText(t, root.availability)}
                  </span>
                )}
                {row.enabled &&
                  root.availability === 'root-changed' &&
                  !locked && (
                    <Button
                      size="sm"
                      variant="outline"
                      className="mt-1 h-7 w-fit"
                      onClick={() => onRenew(root)}
                    >
                      {t('agentBackup.sources.lookAgain')}
                    </Button>
                  )}
                {row.enabled &&
                  (root.availability === 'unknown-format' ||
                    root.availability === 'changing') && (
                    <span>{availabilityText(t, root.availability)}</span>
                  )}
              </span>
            ))}
            {row.notes.length > 0 && <SourceNotes row={row} />}
            {parts.size > 0 && (
              <span
                aria-label={t('agentBackup.sources.parts.label')}
                className="flex flex-wrap gap-x-3 gap-y-0.5"
              >
                {PARTS.filter(({ part }) => parts.has(part)).map(
                  ({ part, label }) => (
                    <span key={part} className="flex items-center gap-1">
                      <Check
                        aria-hidden
                        className="size-3 text-green"
                        strokeWidth={2.5}
                      />
                      {t(label)}
                    </span>
                  ),
                )}
              </span>
            )}
            {totals && (
              <span className="tabular">
                {[
                  subagents > 0 &&
                    t('agentBackup.sources.subagents', { count: subagents }),
                  t('agentBackup.sources.sourceSize', {
                    size: format.bytes(wireNumber(totals.sourceBytes)),
                  }),
                  t('agentBackup.sources.onlyInPathkeep', { count: gone }),
                ]
                  .filter(Boolean)
                  .join(' · ')}
              </span>
            )}
          </div>
        </CollapsibleContent>
      </li>
    </Collapsible>
  )
}
