/**
 * Draws a run's warnings (in their own tab) and notes (under the summary),
 * and what an import recorded (in its preview and its review), each with the
 * fix it points to, when there is one.
 *
 * Not responsible for what an entry says (see `run-entries.ts`).
 */
import { Info, TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import type { TakeoutNote } from '@/lib/types'
import { openFullDiskAccess } from './full-disk-access'
import type { RunAction } from './run-codes'
import { useRecordedEntries, type RunEntry } from './run-entries'

function ActionButton({ action }: { action: RunAction }) {
  const { t } = useI18n()
  if (action === 'full-disk-access')
    return (
      <Button
        size="sm"
        variant="outline"
        className="self-start"
        onClick={() => void openFullDiskAccess(t('backup.sources.grantFailed'))}
      >
        {t('backup.sources.grant')}
      </Button>
    )
  const to = action === 'ai-settings' ? '/settings/ai' : '/settings/background'
  return (
    <Button asChild size="sm" variant="outline" className="self-start">
      <Link to={to}>
        {t(
          action === 'ai-settings'
            ? 'backupRuns.actions.aiSettings'
            : 'backupRuns.actions.backgroundSettings',
        )}
      </Link>
    </Button>
  )
}

function Entry({ entry, icon }: { entry: RunEntry; icon: ReactNode }) {
  return (
    <li className="flex gap-2 rounded-lg bg-muted px-3 py-2">
      {icon}
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <p className="break-words">{entry.text}</p>
        {entry.evidence && (
          <p className="font-mono text-[11px] break-words whitespace-pre-wrap text-muted-foreground">
            {entry.evidence}
          </p>
        )}
        {entry.action && <ActionButton action={entry.action} />}
      </div>
    </li>
  )
}

const WARNING_ICON = (
  <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />
)
const NOTE_ICON = (
  <Info className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
)

function Entries({ entries, icon }: { entries: RunEntry[]; icon: ReactNode }) {
  return (
    <ul className="flex flex-col gap-1.5 text-[13px]">
      {entries.map((entry, index) => (
        <Entry key={`${index}-${entry.text}`} entry={entry} icon={icon} />
      ))}
    </ul>
  )
}

export function WarningList({ entries }: { entries: RunEntry[] }) {
  const { t } = useI18n()
  return entries.length === 0 ? (
    <p className="text-[13px] text-muted-foreground">
      {t('backup.runs.detail.noWarnings')}
    </p>
  ) : (
    <Entries entries={entries} icon={WARNING_ICON} />
  )
}

export function NoteList({ entries }: { entries: RunEntry[] }) {
  const { t } = useI18n()
  if (entries.length === 0) return null
  return (
    <section className="flex flex-col gap-1.5">
      <h3 className="text-[13px] font-medium">{t('backupRuns.notes.title')}</h3>
      <Entries entries={entries} icon={NOTE_ICON} />
    </section>
  )
}

/**
 * What an import recorded (records it skipped, files it could not read, each
 * undo and restore of it), where its preview and its review show it:
 * warnings, then notes. Nothing when it recorded nothing.
 */
export function ImportEntries({
  recorded,
  skip,
}: {
  recorded: { notes: string[]; noteDetails?: TakeoutNote[] }
  /** Codes the surrounding text already says. */
  skip?: readonly string[]
}) {
  const { warnings, notes } = useRecordedEntries(
    { warnings: recorded.notes, warningDetails: recorded.noteDetails },
    skip,
  )
  if (warnings.length === 0 && notes.length === 0) return null
  return (
    <div className="flex flex-col gap-1.5">
      {warnings.length > 0 && (
        <Entries entries={warnings} icon={WARNING_ICON} />
      )}
      {notes.length > 0 && <Entries entries={notes} icon={NOTE_ICON} />}
    </div>
  )
}
