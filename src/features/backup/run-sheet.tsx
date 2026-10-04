/** Side sheet with one run's audit detail: counts, errors and warnings. */
import { useQuery } from '@tanstack/react-query'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { Skeleton } from '@/components/ui/skeleton'
import { auditClient } from '@/lib/backend-client/audit'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import type { AuditRunDetail } from '@/lib/types'
import { RunIcon } from './run-icon'
import { runDuration } from './run-duration'

function Stat({ label, value }: { label: string; value: number }) {
  const format = useFormat()
  return (
    <div className="flex flex-col gap-0.5 rounded-lg bg-muted px-3 py-2">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-lg font-semibold tabular">
        {format.number(value)}
      </span>
    </div>
  )
}

function warningLines(detail: AuditRunDetail) {
  const lines =
    detail.warningDetails?.map((warning) => warning.message) ?? detail.warnings
  return lines.length > 0 ? lines : detail.warnings
}

function Detail({ detail }: { detail: AuditRunDetail }) {
  const { t } = useI18n()
  const format = useFormat()
  const { run } = detail
  const warnings = warningLines(detail)
  const trigger =
    detail.trigger === 'manual' || detail.trigger === 'schedule'
      ? detail.trigger
      : null
  return (
    <div className="flex flex-col gap-5 px-4 pb-6">
      <dl className="flex flex-col gap-2 text-[13px]">
        <div className="flex justify-between gap-4">
          <dt className="text-muted-foreground">
            {t('backup.runs.detail.started')}
          </dt>
          <dd>{format.dayAndTime(run.startedAt)}</dd>
        </div>
        <div className="flex justify-between gap-4">
          <dt className="text-muted-foreground">
            {t('backup.runs.detail.duration')}
          </dt>
          <dd>{runDuration(run, t) ?? '—'}</dd>
        </div>
        <div className="flex justify-between gap-4">
          <dt className="text-muted-foreground">
            {t('backup.runs.detail.trigger')}
          </dt>
          <dd>
            {trigger
              ? t(`backup.runs.detail.triggers.${trigger}`)
              : detail.trigger}
          </dd>
        </div>
      </dl>
      <div className="grid grid-cols-3 gap-2">
        <Stat label={t('backup.runs.detail.newVisits')} value={run.newVisits} />
        <Stat label={t('backup.runs.detail.newPages')} value={run.newUrls} />
        <Stat
          label={t('backup.runs.detail.sources')}
          value={run.profilesProcessed}
        />
      </div>
      {(detail.errorMessage ?? run.errorMessage) && (
        <section className="flex flex-col gap-1.5">
          <h3 className="text-[13px] font-medium text-destructive">
            {t('backup.runs.detail.error')}
          </h3>
          <p className="rounded-lg bg-muted p-3 font-mono text-xs break-words whitespace-pre-wrap">
            {detail.errorMessage ?? run.errorMessage}
          </p>
        </section>
      )}
      <section className="flex flex-col gap-1.5">
        <h3 className="text-[13px] font-medium">
          {t('backup.runs.detail.warnings')}
        </h3>
        {warnings.length === 0 ? (
          <p className="text-[13px] text-muted-foreground">
            {t('backup.runs.detail.noWarnings')}
          </p>
        ) : (
          <ul className="flex flex-col gap-1.5 text-[13px]">
            {warnings.map((line, index) => (
              <li
                key={`${index}-${line}`}
                className="rounded-lg bg-muted px-3 py-2 break-words"
              >
                {line}
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  )
}

export function RunSheet({
  runId,
  onClose,
}: {
  runId: number | null
  onClose: () => void
}) {
  const { t } = useI18n()
  const query = useQuery({
    queryKey: [...queryKeys.archiveData, 'run-detail', runId],
    queryFn: () => auditClient.getRunDetail(runId as number),
    enabled: runId !== null,
  })
  return (
    <Sheet open={runId !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-[420px] sm:max-w-[420px]">
        <SheetHeader>
          <SheetTitle className="flex items-center gap-2">
            {query.data && <RunIcon status={query.data.run.status} />}
            {t('backup.runs.detail.title')}
          </SheetTitle>
          <SheetDescription>
            {t('backup.runs.detail.description')}
          </SheetDescription>
        </SheetHeader>
        {query.isPending ? (
          <div className="flex flex-col gap-3 px-4">
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-24 w-full" />
          </div>
        ) : query.isError ? (
          <p
            role="alert"
            className="px-4 text-[13px] break-words text-destructive"
          >
            {describeError(query.error, 'load_audit_run_detail')}
          </p>
        ) : (
          <Detail detail={query.data} />
        )}
      </SheetContent>
    </Sheet>
  )
}
