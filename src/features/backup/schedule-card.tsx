/**
 * Automatic backup: how often the native scheduler runs PathKeep, when it
 * runs next, and whether the scheduler is healthy. Choosing an interval,
 * turning it off or fixing a problem opens a preview first
 * (`schedule-dialog.tsx`); "Details" shows what PathKeep verified
 * (`schedule-details-sheet.tsx`).
 */
import { ChevronRight, TriangleAlert } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
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
import { formatInterval } from './interval'
import {
  frequencyForHours,
  frequencyHours,
  type Frequency,
} from './schedule-actions'
import { ScheduleDetailsSheet } from './schedule-details-sheet'
import { ScheduleDialog, type ScheduleIntent } from './schedule-dialog'
import { ManualSteps } from './schedule-plan-view'

type Choice = Frequency | 'custom'

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
  linux: 'systemd',
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

const segmentClass =
  'h-8 flex-1 rounded-md px-2 text-[13px] font-normal text-muted-foreground hover:bg-transparent data-[state=on]:bg-card data-[state=on]:text-foreground data-[state=on]:shadow-card'

export function ScheduleCard() {
  const { t } = useI18n()
  const format = useFormat()
  const snapshot = useSnapshot()
  const query = useScheduleStatus()
  const status = query.data
  const [intent, setIntent] = useState<ScheduleIntent | null>(null)
  const [details, setDetails] = useState(false)

  const applySupported = status?.applySupported ?? true
  const active = status
    ? scheduleInstalled(status) || needsSchedulerReview(status)
    : false
  const savedHours = snapshot.config.dueAfterHours
  const preset = frequencyForHours(savedHours)
  const value: Choice | null = !status
    ? null
    : applySupported && !active
      ? 'off'
      : (preset ?? 'custom')
  const next = nextScheduledBackup(status)
  const platform = normalizePlatform(status?.platform)
  const changeMode = applySupported && active ? 'update' : 'install'

  function choose(choice: Choice) {
    if (choice === value) return
    if (choice === 'off') setIntent({ kind: 'off' })
    else if (choice === 'custom')
      setIntent({ kind: 'custom', hours: savedHours, mode: changeMode })
    else
      setIntent({
        kind: 'set',
        hours: frequencyHours[choice],
        mode: changeMode,
      })
  }

  const choices: Choice[] = applySupported
    ? ['hourly', 'sixHours', 'daily', 'custom', 'off']
    : ['hourly', 'sixHours', 'daily', 'custom']

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
          onValueChange={(next) => next && choose(next as Choice)}
          aria-label={t('backup.schedule.label')}
          className="w-full rounded-lg bg-muted p-[3px]"
        >
          {choices.map((choice) => (
            <ToggleGroupItem
              key={choice}
              value={choice}
              className={segmentClass}
            >
              {choice === 'custom'
                ? t('backupSchedule.custom')
                : t(`backup.schedule.${choice}`)}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      )}
      {status && value === 'custom' && (
        <p className="flex items-center justify-between gap-3 text-[13px] text-muted-foreground">
          {t('backupSchedule.customActive', {
            interval: formatInterval(savedHours, t),
          })}
          <Button
            size="xs"
            variant="ghost"
            onClick={() =>
              setIntent({ kind: 'custom', hours: savedHours, mode: changeMode })
            }
          >
            {t('backupSchedule.changeCustom')}
          </Button>
        </p>
      )}
      {status && (
        <div className="flex flex-col gap-2.5">
          <Row label={t('backup.schedule.next')}>
            {next ? format.dayAndTime(next) : '—'}
          </Row>
          {active && (
            <Row label={t('backupSchedule.lastRun')}>
              {status.lastScheduledSuccessAt
                ? format.dayAndTime(status.lastScheduledSuccessAt)
                : t('backupSchedule.lastRunNever')}
            </Row>
          )}
          <Row label={t('backup.schedule.scheduler')}>
            {platformNames[platform]} ·{' '}
            {t(
              `backup.schedule.state.${installStates[status.installState] ?? 'manual'}`,
            )}
          </Row>
        </div>
      )}
      {status && applySupported && needsSchedulerReview(status) && (
        <Attention
          status={status}
          onFix={() =>
            setIntent(
              status.installState === 'legacy-install-detected'
                ? { kind: 'legacy' }
                : { kind: 'set', hours: savedHours, mode: 'reinstall' },
            )
          }
        />
      )}
      {status && !applySupported && (
        <div className="flex flex-col gap-2 rounded-lg bg-muted p-3 text-[13px]">
          <span className="font-medium">
            {t('backup.schedule.manual.title')}
          </span>
          <span className="text-muted-foreground">
            {t('backup.schedule.manual.body')}
          </span>
          <ManualSteps
            details={status.manualStepDetails}
            steps={status.manualSteps}
          />
        </div>
      )}
      {status && (
        <Button
          size="sm"
          variant="ghost"
          className="-ml-2 w-fit text-muted-foreground"
          onClick={() => setDetails(true)}
        >
          {t('backupSchedule.details')}
          <ChevronRight />
        </Button>
      )}
      <ScheduleDialog
        intent={intent}
        status={status}
        onClose={() => setIntent(null)}
      />
      {status && (
        <ScheduleDetailsSheet
          open={details}
          status={status}
          checking={query.isFetching}
          onCheck={() => void query.refetch()}
          onClose={() => setDetails(false)}
        />
      )}
    </SectionCard>
  )
}

function Attention({
  status,
  onFix,
}: {
  status: ScheduleStatus
  onFix: () => void
}) {
  const { t } = useI18n()
  const body =
    status.installState === 'legacy-install-detected'
      ? t('backupSchedule.legacy.attention')
      : status.installState === 'permission-warning'
        ? t('backup.schedule.attention.permission')
        : t('backup.schedule.attention.mismatch')
  return (
    <div className="flex items-start gap-3 rounded-lg bg-brand-soft p-3">
      <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5 text-[13px]">
        <span className="font-medium">
          {t('backup.schedule.attention.title')}
        </span>
        <span className="text-muted-foreground">{body}</span>
      </div>
      <Button size="sm" variant="outline" onClick={onFix}>
        {t('backup.schedule.attention.repair')}
      </Button>
    </div>
  )
}
