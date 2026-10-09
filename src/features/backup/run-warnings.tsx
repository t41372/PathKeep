/**
 * Draws a run's warnings (in their own tab) and notes (under the summary),
 * each with the fix it points to, when there is one.
 *
 * Not responsible for what an entry says (see `run-entries.ts`).
 */
import { Info, TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import { openFullDiskAccess } from './full-disk-access'
import type { RunAction } from './run-codes'
import type { RunEntry } from './run-entries'

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

export function WarningList({ entries }: { entries: RunEntry[] }) {
  const { t } = useI18n()
  return entries.length === 0 ? (
    <p className="text-[13px] text-muted-foreground">
      {t('backup.runs.detail.noWarnings')}
    </p>
  ) : (
    <ul className="flex flex-col gap-1.5 text-[13px]">
      {entries.map((entry, index) => (
        <Entry
          key={`${index}-${entry.text}`}
          entry={entry}
          icon={<TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />}
        />
      ))}
    </ul>
  )
}

export function NoteList({ entries }: { entries: RunEntry[] }) {
  const { t } = useI18n()
  if (entries.length === 0) return null
  return (
    <section className="flex flex-col gap-1.5">
      <h3 className="text-[13px] font-medium">{t('backupRuns.notes.title')}</h3>
      <ul className="flex flex-col gap-1.5 text-[13px]">
        {entries.map((entry, index) => (
          <Entry
            key={`${index}-${entry.text}`}
            entry={entry}
            icon={
              <Info className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
            }
          />
        ))}
      </ul>
    </section>
  )
}
