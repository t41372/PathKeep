/**
 * Agent conversations' results in ⌘K, until conversations can be found:
 * the tools and folders it backs up, and its recent backups, whose words
 * match what was typed. A source opens the backup page; a backup opens its
 * details there (`?run=`).
 *
 * Responsibilities: matching what the backup page already reads (its
 * sources and the first page of its runs) and drawing the group, which
 * is left out when nothing matched. Not responsible for asking the backend
 * anything the backup page does not.
 */
import {
  CommandGroup,
  CommandItem,
  CommandShortcut,
} from '@/components/ui/command'
import { useFormat, useI18n } from '@/lib/i18n'
import { useAgentRuns, useAgentSources } from '@/lib/queries/agent-backup'
import { spaceBackupPath, type PaletteResultsProps } from '@/lib/spaces'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import { folderLabel, toolName } from '@/features/data-kinds/tools'
import { RUN_PARAM } from './run-param'
import { runLine } from './words'

/** Enough to find one, few enough to leave room for the other groups. */
const SHOWN = 4

export default function PaletteResults({
  term,
  open,
  heading,
  searchRow,
}: PaletteResultsProps) {
  const { t } = useI18n()
  const format = useFormat()
  const sources = useAgentSources()
  const runs = useAgentRuns()
  const needle = term.toLocaleLowerCase()
  const has = (...words: (string | null | undefined)[]) =>
    words.some((word) => word?.toLocaleLowerCase().includes(needle))

  const folders = (sources.data?.pages.flatMap((page) => page.items) ?? [])
    .filter((source) => source.path !== null)
    .map((source) => {
      const label = folderLabel(source.tool, source.path)
      return {
        source,
        name: t(toolName(source.tool)),
        label: label ? t(label) : undefined,
      }
    })
    .filter(({ source, name, label }) => has(name, label, source.path))
    .slice(0, SHOWN)

  const backups = (runs.data?.pages[0]?.items ?? [])
    .map((run) => ({
      run,
      label: t('agentBackup.palette.run', {
        time: format.dayAndTime(run.startedAt ?? 0),
      }),
      line: runLine(t, run),
    }))
    .filter(({ label, line }) => has(label, line))
    .slice(0, SHOWN)

  // Nothing matched and no search row: no heading left alone either.
  if (folders.length === 0 && backups.length === 0 && !searchRow) return null
  return (
    <CommandGroup heading={heading}>
      {folders.map(({ source, label }) => (
        <CommandItem
          key={`source:${source.rootId ?? source.tool}:${source.path}`}
          value={`agent-source:${source.rootId ?? source.path}`}
          onSelect={() => open(spaceBackupPath('agent'))}
        >
          <ToolBadge id={source.tool} />
          {label && (
            <span className="truncate text-muted-foreground">{label}</span>
          )}
          <CommandShortcut className="max-w-[45%] truncate font-mono tracking-normal">
            {source.path}
          </CommandShortcut>
        </CommandItem>
      ))}
      {backups.map(({ run, label, line }) => (
        <CommandItem
          key={run.runRef}
          value={`agent-run:${run.runRef}`}
          onSelect={() =>
            open(
              spaceBackupPath(
                'agent',
                `?${RUN_PARAM}=${encodeURIComponent(run.runRef)}`,
              ),
            )
          }
        >
          <span className="truncate">{label}</span>
          {line && (
            <CommandShortcut className="max-w-[50%] truncate tracking-normal">
              {line}
            </CommandShortcut>
          )}
        </CommandItem>
      ))}
      {searchRow}
    </CommandGroup>
  )
}
