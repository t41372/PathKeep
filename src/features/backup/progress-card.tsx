/** Live progress while a backup runs, fed by the shared backup runner. */
import { useBackupRunner } from '@/app/backup-runner'
import { useFormat, useI18n } from '@/lib/i18n'

export function ProgressCard() {
  const { t } = useI18n()
  const format = useFormat()
  const { running, progress } = useBackupRunner()
  if (!running) return null

  const total = progress?.totalProfiles ?? 0
  const percent =
    progress?.progressPercent ??
    (total > 0 ? (progress!.completedProfiles / total) * 100 : null)
  const processed = progress?.processedRecords
  const records = progress?.totalRecords

  return (
    <div className="animate-rise flex flex-col gap-2.5 rounded-xl border bg-card px-[18px] py-3.5 shadow-card">
      <div className="flex items-baseline justify-between gap-4 text-sm">
        <span className="truncate">
          {total > 0
            ? t('backup.progress.title', {
                done: Math.min(progress!.completedProfiles + 1, total),
                total,
              })
            : t('backup.progress.starting')}
          {progress?.sourceLabel && (
            <span className="text-muted-foreground">
              {' '}
              · {progress.sourceLabel}
            </span>
          )}
        </span>
        {processed != null && records != null && records > 0 && (
          <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">
            {format.number(processed)} / {format.number(records)}
          </span>
        )}
      </div>
      <div
        role="progressbar"
        aria-label={t('backup.progress.label')}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent == null ? undefined : Math.round(percent)}
        className="h-1 overflow-hidden rounded-full bg-muted"
      >
        <div
          className={
            percent == null
              ? 'h-full w-1/3 animate-indeterminate rounded-full bg-brand'
              : 'h-full rounded-full bg-brand transition-[width] duration-300'
          }
          style={
            percent == null
              ? undefined
              : { width: `${Math.min(100, Math.max(2, percent))}%` }
          }
        />
      </div>
    </div>
  )
}
