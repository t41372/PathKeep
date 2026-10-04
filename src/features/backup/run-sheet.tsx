/**
 * Side sheet with one run's audit detail (`load_audit_run_detail`) in three
 * tabs: Summary (times, counts, error, and a rekey's safety copy), Files
 * (manifest and kept copies, each with copy / show in folder) and Warnings.
 * The warnings tab carries its count so nothing is hidden behind it.
 */
import { useQuery } from '@tanstack/react-query'
import { ShieldCheck } from 'lucide-react'
import type { ReactNode } from 'react'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { auditClient } from '@/lib/backend-client/audit'
import { describeError } from '@/lib/errors'
import { messages, useFormat, useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import type { AuditArtifact, AuditRunDetail } from '@/lib/types'
import { PathLine } from './path-actions'
import { RunIcon } from './run-icon'
import { runDuration } from './run-duration'

const knownReasons = messages.en.backupRuns.reasons
type Reason = keyof typeof knownReasons

const SAFETY_REASON: Reason = 'before-rekey'

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

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex justify-between gap-4">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-right">{children}</dd>
    </div>
  )
}

function warningLines(detail: AuditRunDetail) {
  const lines =
    detail.warningDetails?.map((warning) => warning.message) ?? detail.warnings
  return lines.length > 0 ? lines : detail.warnings
}

function ArtifactBlock({ artifact }: { artifact: AuditArtifact }) {
  const { t } = useI18n()
  const format = useFormat()
  const reason =
    artifact.reason && artifact.reason in knownReasons
      ? t(`backupRuns.reasons.${artifact.reason as Reason}`)
      : artifact.reason
  return (
    <li className="flex flex-col gap-1 rounded-lg bg-muted px-3 py-2">
      <span className="text-[13px] font-medium">
        {reason ?? t('backupRuns.kinds.snapshot')}
      </span>
      <PathLine path={artifact.path} />
      <span className="text-xs text-muted-foreground tabular">
        {format.dayAndTime(artifact.createdAt)}
        {artifact.sizeBytes != null &&
          ` · ${t('backupRuns.size')} ${format.bytes(artifact.sizeBytes)}`}
      </span>
    </li>
  )
}

function Summary({ detail }: { detail: AuditRunDetail }) {
  const { t } = useI18n()
  const format = useFormat()
  const { run } = detail
  const trigger =
    detail.trigger === 'manual' || detail.trigger === 'schedule'
      ? detail.trigger
      : null
  const safety = detail.artifacts.find(
    (artifact) => artifact.reason === SAFETY_REASON,
  )
  return (
    <div className="flex flex-col gap-5">
      <dl className="flex flex-col gap-2 text-[13px]">
        <Fact label={t('backup.runs.detail.started')}>
          {format.dayAndTime(run.startedAt)}
        </Fact>
        <Fact label={t('backup.runs.detail.duration')}>
          {runDuration(run, t) ?? '—'}
        </Fact>
        <Fact label={t('backup.runs.detail.trigger')}>
          {trigger
            ? t(`backup.runs.detail.triggers.${trigger}`)
            : detail.trigger}
          {detail.dueOnly && (
            <span className="block text-xs text-muted-foreground">
              {t('backupRuns.dueOnly')}
            </span>
          )}
        </Fact>
        {detail.timezone && (
          <Fact label={t('backupRuns.timezone')}>{detail.timezone}</Fact>
        )}
      </dl>
      <div className="grid grid-cols-3 gap-2">
        <Stat label={t('backup.runs.detail.newVisits')} value={run.newVisits} />
        <Stat label={t('backup.runs.detail.newPages')} value={run.newUrls} />
        <Stat
          label={t('backup.runs.detail.sources')}
          value={run.profilesProcessed}
        />
      </div>
      {safety && (
        <section
          className="flex flex-col gap-1.5 rounded-lg bg-brand-soft p-3 text-[13px]"
          aria-label={t('backupRuns.safety.title')}
        >
          <h3 className="flex items-center gap-1.5 font-medium">
            <ShieldCheck className="size-4 text-brand" />
            {t('backupRuns.safety.title')}
          </h3>
          <p className="text-muted-foreground">{t('backupRuns.safety.body')}</p>
          <PathLine path={safety.path} />
        </section>
      )}
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
      {detail.manifestPath && (
        <section className="flex flex-col gap-1">
          <h3 className="text-[13px] font-medium">
            {t('backupRuns.manifest')}
          </h3>
          <PathLine path={detail.manifestPath} />
        </section>
      )}
    </div>
  )
}

function Files({ detail }: { detail: AuditRunDetail }) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-4 text-[13px]">
      <section className="flex flex-col gap-1">
        <h3 className="font-medium">{t('backupRuns.manifest')}</h3>
        {detail.manifestPath ? (
          <>
            <PathLine path={detail.manifestPath} />
            {detail.manifestHash && (
              <span className="font-mono text-[11px] break-all text-muted-foreground">
                {t('backupRuns.manifestHash')} {detail.manifestHash}
              </span>
            )}
          </>
        ) : (
          <p className="text-muted-foreground">{t('backupRuns.noManifest')}</p>
        )}
      </section>
      {detail.artifacts.length === 0 ? (
        <p className="text-muted-foreground">{t('backupRuns.noFiles')}</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {detail.artifacts.map((artifact) => (
            <ArtifactBlock key={artifact.path} artifact={artifact} />
          ))}
        </ul>
      )}
    </div>
  )
}

function Warnings({ lines }: { lines: string[] }) {
  const { t } = useI18n()
  return lines.length === 0 ? (
    <p className="text-[13px] text-muted-foreground">
      {t('backup.runs.detail.noWarnings')}
    </p>
  ) : (
    <ul className="flex flex-col gap-1.5 text-[13px]">
      {lines.map((line, index) => (
        <li
          key={`${index}-${line}`}
          className="rounded-lg bg-muted px-3 py-2 break-words"
        >
          {line}
        </li>
      ))}
    </ul>
  )
}

function Detail({ detail }: { detail: AuditRunDetail }) {
  const { t } = useI18n()
  const warnings = warningLines(detail)
  return (
    <Tabs defaultValue="summary" className="gap-4 px-4 pb-6">
      <TabsList className="w-full">
        <TabsTrigger value="summary">
          {t('backupRuns.tabs.summary')}
        </TabsTrigger>
        <TabsTrigger value="files">{t('backupRuns.tabs.files')}</TabsTrigger>
        <TabsTrigger value="warnings">
          {t('backupRuns.tabs.warnings', { count: warnings.length })}
        </TabsTrigger>
      </TabsList>
      <TabsContent value="summary">
        <Summary detail={detail} />
      </TabsContent>
      <TabsContent value="files">
        <Files detail={detail} />
      </TabsContent>
      <TabsContent value="warnings">
        <Warnings lines={warnings} />
      </TabsContent>
    </Tabs>
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
      <SheetContent className="w-[460px] overflow-y-auto sm:max-w-[460px]">
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
