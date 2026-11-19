/**
 * The entries of one conversation backup's details: what it could not save
 * (warnings) and the notes it left on files it did save. Each entry is in
 * plain words, with its tool, its file and its code for anyone who needs
 * to look further. Format notices are `format-notice-card.tsx`.
 *
 * Not responsible for: which entries a run has or paging them
 * (`agent-run-sheet.tsx`).
 */
import type { RunNote, RunWarning } from '@/lib/backend-client/agent-backup'
import { useI18n } from '@/lib/i18n'
import { toolName } from '@/features/data-kinds/tools'
import { noteText, warningText } from './words'

const OMITTED = new Set(['diagnostics_omitted', 'capture_warnings_omitted'])

function Entry({
  text,
  tool,
  path,
  code,
  isNew = false,
}: {
  text: string
  tool: RunWarning['tool']
  path: string | null
  code: string
  isNew?: boolean
}) {
  const { t } = useI18n()
  return (
    <li
      data-code={code}
      className="flex flex-col gap-0.5 rounded-lg bg-muted px-3 py-2 text-[13px]"
    >
      <span>
        {tool
          ? t('agentTasks.warnings.tool', { tool: t(toolName(tool)), text })
          : text}
        {isNew && (
          <span className="ml-1.5 rounded bg-background px-1 text-[10.5px] text-muted-foreground">
            {t('agentTasks.notes.isNew')}
          </span>
        )}
      </span>
      {path && (
        <span className="font-mono text-[11px] [overflow-wrap:anywhere] text-muted-foreground">
          {path}
        </span>
      )}
      <span className="font-mono text-[10.5px] text-muted-foreground/80">
        {code}
      </span>
    </li>
  )
}

/**
 * Something the run did not save. `counted`: the run is recent enough to
 * keep notes apart, so an unknown code here means data was not saved.
 */
export function WarningEntry({
  warning,
  counted,
}: {
  warning: RunWarning
  counted: boolean
}) {
  const { t } = useI18n()
  const text = OMITTED.has(warning.code)
    ? t('agentTasks.warnings.omitted', { count: Number(warning.detail) || 1 })
    : warningText(t, warning, counted)
  return (
    <Entry
      text={text}
      tool={warning.tool}
      path={warning.path}
      code={warning.code}
    />
  )
}

/** A condition of a file the run saved. */
export function NoteEntry({ note }: { note: RunNote }) {
  const { t } = useI18n()
  return (
    <Entry
      text={noteText(t, note)}
      tool={note.tool}
      path={note.path}
      code={note.code}
      isNew={note.isNew}
    />
  )
}
