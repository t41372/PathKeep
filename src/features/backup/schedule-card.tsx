/**
 * Automatic backup: choose how often the native scheduler runs, see when the
 * next run is due, and repair or set up the scheduler when it needs it.
 */
import { TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'
import { toast } from 'sonner'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import {
  needsSchedulerReview,
  normalizePlatform,
} from '@/lib/platform-guidance'
import { useSnapshot } from '@/lib/queries/app'
import {
  nextScheduledBackup,
  scheduleInstalled,
  useScheduleStatus,
} from '@/lib/queries/schedule'
import type { ScheduleStatus } from '@/lib/types'
import {
  frequencyForHours,
  useChangeFrequency,
  useRepairSchedule,
  type Frequency,
} from './schedule-actions'

type InstallStateKey =
  | 'installed'
  | 'notInstalled'
  | 'mismatch'
  | 'permissionWarning'
  | 'legacy'
  | 'manual'

const platformNames = {
  macos: 'launchd',
  windows: 'Task Scheduler',
  linux: 'Linux',
} as const

const installStates: Record<string, InstallStateKey> = {
  installed: 'installed',
  'not-installed': 'notInstalled',
  mismatch: 'mismatch',
  'permission-warning': 'permissionWarning',
  'legacy-install-detected': 'legacy',
  'manual-review': 'manual',
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4 text-[13px]">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-right font-mono text-[13px]">{children}</span>
    </div>
  )
}

function segmentClass() {
  return 'h-8 flex-1 rounded-md px-2 text-[13px] font-normal text-muted-foreground hover:bg-transparent data-[state=on]:bg-card data-[state=on]:text-foreground data-[state=on]:shadow-card'
}

export function ScheduleCard() {
  const { t } = useI18n()
  const format = useFormat()
  const snapshot = useSnapshot()
  const query = useScheduleStatus()
  const status = query.data
  const applySupported = status?.applySupported ?? true
  const change = useChangeFrequency(applySupported)
  const repair = useRepairSchedule()

  const active = status
    ? scheduleInstalled(status) || needsSchedulerReview(status)
    : false
  const preset = frequencyForHours(snapshot.config.dueAfterHours)
  const value: Frequency | null = !status
    ? null
    : applySupported && !active
      ? 'off'
      : preset
  const next = nextScheduledBackup(status)
  const platform = normalizePlatform(status?.platform)

  function choose(frequency: Frequency) {
    if (frequency === value) return
    change.mutate(frequency, {
      onSuccess: () =>
        toast.success(
          frequency === 'off'
            ? t('backup.schedule.turnedOff')
            : t('backup.schedule.changed', {
                frequency: t(`backup.schedule.${frequency}`),
              }),
        ),
      onError: (error) =>
        toast.error(t('backup.schedule.changeFailed'), {
          description: describeError(error, 'apply_schedule'),
        }),
    })
  }

  function fix() {
    repair.mutate(undefined, {
      onSuccess: () => toast.success(t('backup.schedule.repaired')),
      onError: (error) =>
        toast.error(t('backup.schedule.repairFailed'), {
          description: describeError(error, 'repair_schedule'),
        }),
    })
  }

  const frequencies: Frequency[] = applySupported
    ? ['hourly', 'sixHours', 'daily', 'off']
    : ['hourly', 'sixHours', 'daily']

  return (
    <SectionCard
      title={t('backup.schedule.title')}
      subtitle={t('backup.schedule.subtitle')}
    >
      {!status ? (
        <Skeleton className="h-9 w-full" />
      ) : (
        <ToggleGroup
          type="single"
          spacing={1}
          value={value ?? ''}
          disabled={change.isPending}
          onValueChange={(next) => next && choose(next as Frequency)}
          aria-label={t('backup.schedule.label')}
          className="w-full rounded-lg bg-muted p-[3px]"
        >
          {frequencies.map((frequency) => (
            <ToggleGroupItem
              key={frequency}
              value={frequency}
              className={segmentClass()}
            >
              {t(`backup.schedule.${frequency}`)}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      )}
      {status && !preset && active && (
        <p className="text-[13px] text-muted-foreground">
          {t('backup.schedule.custom', {
            hours: snapshot.config.dueAfterHours,
          })}
        </p>
      )}
      {status && (
        <div className="flex flex-col gap-2.5">
          <Row label={t('backup.schedule.next')}>
            {next ? format.dayAndTime(next) : '—'}
          </Row>
          <Row label={t('backup.schedule.scheduler')}>
            {platformNames[platform]} ·{' '}
            {t(
              `backup.schedule.state.${installStates[status.installState] ?? 'manual'}`,
            )}
          </Row>
        </div>
      )}
      {status && applySupported && needsSchedulerReview(status) && (
        <Attention status={status} busy={repair.isPending} onRepair={fix} />
      )}
      {status && !applySupported && <ManualSteps status={status} />}
    </SectionCard>
  )
}

function Attention({
  status,
  busy,
  onRepair,
}: {
  status: ScheduleStatus
  busy: boolean
  onRepair: () => void
}) {
  const { t } = useI18n()
  const kind =
    status.installState === 'permission-warning' ? 'permission' : 'mismatch'
  return (
    <div className="flex items-start gap-3 rounded-lg bg-brand-soft p-3">
      <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5 text-[13px]">
        <span className="font-medium">
          {t('backup.schedule.attention.title')}
        </span>
        <span className="text-muted-foreground">
          {t(`backup.schedule.attention.${kind}`)}
        </span>
      </div>
      <Button size="sm" variant="outline" disabled={busy} onClick={onRepair}>
        {t('backup.schedule.attention.repair')}
      </Button>
    </div>
  )
}

function ManualSteps({ status }: { status: ScheduleStatus }) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-2 rounded-lg bg-muted p-3 text-[13px]">
      <span className="font-medium">{t('backup.schedule.manual.title')}</span>
      <span className="text-muted-foreground">
        {t('backup.schedule.manual.body')}
      </span>
      {status.manualSteps.length > 0 && (
        <ol className="flex list-decimal flex-col gap-1 pl-4 font-mono text-xs break-all">
          {status.manualSteps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>
      )}
    </div>
  )
}
