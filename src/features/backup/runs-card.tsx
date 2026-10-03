/** Recent runs from the snapshot. Each row opens the full audit detail in a sheet. */
import { useState } from 'react'
import { useFormat, useI18n, type Translator } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { BackupRunOverview } from '@/lib/types'
import { RunIcon } from './run-icon'
import { runDuration } from './run-duration'
import { RunSheet } from './run-sheet'

const runTypes = [
  'backup',
  'import',
  'rekey',
  'doctor',
  'archive_restore',
  'snapshot_restore',
  'retention_prune',
] as const
type RunType = (typeof runTypes)[number]

function summary(run: BackupRunOverview, t: Translator) {
  if (run.status === 'failed')
    return run.errorMessage || t('backup.runs.failed')
  const type = runTypes.includes(run.runType as RunType)
    ? (run.runType as RunType)
    : 'backup'
  if (type === 'backup') {
    return t('backup.runs.summary', {
      sources: t('backup.runs.sources', { count: run.profilesProcessed }),
      visits: t('backup.runs.newVisits', { count: run.newVisits }),
    })
  }
  const label = t(`backup.runs.types.${type}`)
  return run.newVisits > 0
    ? t('backup.runs.summary', {
        sources: label,
        visits: t('backup.runs.newVisits', { count: run.newVisits }),
      })
    : label
}

export function RunsCard() {
  const { t } = useI18n()
  const format = useFormat()
  const runs = useSnapshot().recentRuns
  const [open, setOpen] = useState<number | null>(null)

  return (
    <section className="overflow-hidden rounded-xl border bg-card shadow-card">
      <h2 className="px-5 py-3.5 text-sm font-semibold">
        {t('backup.runs.title')}
      </h2>
      {runs.length === 0 ? (
        <p className="border-t px-5 py-4 text-[13px] text-muted-foreground">
          {t('backup.runs.empty')}
        </p>
      ) : (
        <ul className="border-t">
          {runs.map((run) => (
            <li key={run.id} className="border-t first:border-t-0">
              <button
                type="button"
                onClick={() => setOpen(run.id)}
                className="flex w-full items-center gap-3.5 px-5 py-2.5 text-left text-[13px] transition-colors hover:bg-accent/60"
              >
                <RunIcon status={run.status} />
                <span className="w-32 shrink-0 font-mono text-xs text-muted-foreground tabular">
                  {format.dayAndTime(run.startedAt)}
                </span>
                <span className="min-w-0 flex-1 truncate">
                  {summary(run, t)}
                </span>
                <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">
                  {runDuration(run, t) ?? '—'}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <RunSheet runId={open} onClose={() => setOpen(null)} />
    </section>
  )
}
