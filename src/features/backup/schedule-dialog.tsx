/**
 * Preview → confirm → apply for every change to the automatic backup:
 * install or change an interval (preset or custom), reinstall, turn off,
 * and remove a job left by an earlier version. The dialog shows the exact
 * file the scheduler will get, or exactly what will be removed, before the
 * user confirms; nothing is saved or installed while it is open.
 *
 * Not responsible for deciding which change applies (the card does) or for
 * the backend calls (`schedule-actions.ts`).
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { useDebouncedValue } from '@/lib/hooks/use-debounced-value'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { SchedulePlan, ScheduleStatus } from '@/lib/types'
import {
  formatInterval,
  hoursToMinutes,
  intervalHours,
  splitInterval,
  wakeHours,
  type IntervalUnit,
} from './interval'
import { PathLine } from './path-actions'
import { ManualSteps, PlanFiles } from './schedule-plan-view'
import {
  useApplySchedule,
  useCurrentSchedulePlan,
  useRemoveSchedule,
  useRepairLegacySchedule,
  useSchedulePlan,
} from './schedule-actions'

export type ScheduleIntent =
  | { kind: 'set'; hours: number; mode: 'install' | 'update' | 'reinstall' }
  | { kind: 'custom'; hours: number; mode: 'install' | 'update' }
  | { kind: 'off' }
  | { kind: 'legacy' }

const schedulerNames: Record<string, string> = {
  macos: 'launchd',
  windows: 'Task Scheduler',
  linux: 'systemd',
}

export function ScheduleDialog({
  intent,
  status,
  onClose,
}: {
  intent: ScheduleIntent | null
  status: ScheduleStatus | undefined
  onClose: () => void
}) {
  return (
    <Dialog open={intent !== null} onOpenChange={(open) => !open && onClose()}>
      {intent?.kind === 'set' || intent?.kind === 'custom' ? (
        <InstallForm intent={intent} onClose={onClose} />
      ) : intent ? (
        <RemoveForm kind={intent.kind} status={status} onClose={onClose} />
      ) : null}
    </Dialog>
  )
}

function ErrorLine({ children }: { children: string }) {
  return (
    <p
      role="alert"
      className="text-[13px] [overflow-wrap:anywhere] text-destructive"
    >
      {children}
    </p>
  )
}

function PlanSkeleton() {
  const { t } = useI18n()
  return (
    <div
      className="flex flex-col gap-2"
      aria-busy
      aria-label={t('backupSchedule.change.loading')}
    >
      <Skeleton className="h-4 w-2/3" />
      <Skeleton className="h-40 w-full" />
    </div>
  )
}

function IntervalInput({
  amount,
  unit,
  onAmount,
  onUnit,
  invalid,
}: {
  amount: string
  unit: IntervalUnit
  onAmount: (value: string) => void
  onUnit: (value: IntervalUnit) => void
  invalid: boolean
}) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2 text-[13px]">
        <span>{t('backupSchedule.change.every')}</span>
        <Input
          type="number"
          inputMode="numeric"
          min={1}
          step={1}
          value={amount}
          onChange={(event) => onAmount(event.target.value)}
          aria-label={t('backupSchedule.change.amount')}
          aria-invalid={invalid}
          className="h-8 w-24 tabular"
        />
        <Select
          value={unit}
          onValueChange={(value) => onUnit(value as IntervalUnit)}
        >
          <SelectTrigger
            size="sm"
            aria-label={t('backupSchedule.change.unit')}
            className="w-28"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {(['minutes', 'hours', 'days'] as const).map((value) => (
              <SelectItem key={value} value={value}>
                {t(`backupSchedule.change.units.${value}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {invalid && <ErrorLine>{t('backupSchedule.change.invalid')}</ErrorLine>}
    </div>
  )
}

function InstallForm({
  intent,
  onClose,
}: {
  intent: Extract<ScheduleIntent, { kind: 'set' | 'custom' }>
  onClose: () => void
}) {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const initial = splitInterval(hoursToMinutes(intent.hours))
  const [amount, setAmount] = useState(String(initial.amount))
  const [unit, setUnit] = useState<IntervalUnit>(initial.unit)
  const typedHours =
    intent.kind === 'custom'
      ? intervalHours(Number(amount), unit)
      : intent.hours
  // Typing "90" should not fetch a plan for "9" first.
  const hours = useDebouncedValue(typedHours, 250)
  const plan = useSchedulePlan(hours)
  const apply = useApplySchedule()
  const settled = hours === typedHours

  const title =
    intent.mode === 'install'
      ? t('backupSchedule.change.installTitle')
      : intent.mode === 'reinstall'
        ? t('backupSchedule.change.reinstallTitle')
        : t('backupSchedule.change.updateTitle')
  const supported = plan.data?.applySupported ?? true
  const confirmLabel = !supported
    ? t('backupSchedule.change.save')
    : intent.mode === 'install'
      ? t('backupSchedule.change.install')
      : intent.mode === 'reinstall'
        ? t('backupSchedule.change.reinstall')
        : t('backupSchedule.change.update')
  const ready = hours !== null && settled && plan.isSuccess && !plan.isFetching

  function confirm(target: SchedulePlan, value: number) {
    apply.mutate(
      { hours: value, plan: target },
      {
        onSuccess: () => {
          toast.success(
            target.applySupported
              ? t('backupSchedule.change.installed', {
                  interval: formatInterval(value, t),
                })
              : t('backupSchedule.change.saved'),
          )
          onClose()
        },
      },
    )
  }

  return (
    <DialogContent
      className="sm:max-w-xl"
      showCloseButton={!apply.isPending}
      onInteractOutside={(event) => apply.isPending && event.preventDefault()}
      onEscapeKeyDown={(event) => apply.isPending && event.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
        <DialogDescription>
          {supported
            ? t('backupSchedule.change.intro')
            : t('backupSchedule.change.manualIntro')}
        </DialogDescription>
      </DialogHeader>

      <div className="flex min-w-0 flex-col gap-4">
        {intent.kind === 'custom' && (
          <IntervalInput
            amount={amount}
            unit={unit}
            onAmount={setAmount}
            onUnit={setUnit}
            invalid={typedHours === null && amount !== ''}
          />
        )}

        {typedHours !== null && (
          <p className="text-[13px] text-muted-foreground">
            {t('backupSchedule.change.when', {
              interval: formatInterval(typedHours, t),
              check: formatInterval(
                wakeHours(
                  typedHours,
                  snapshot.config.scheduleCheckIntervalHours,
                ),
                t,
              ),
            })}
          </p>
        )}

        {hours === null ? null : plan.isPending || !settled ? (
          <PlanSkeleton />
        ) : plan.isError ? (
          <ErrorLine>
            {`${t('backupSchedule.change.previewFailed')} ${describeError(plan.error, 'preview_schedule')}`}
          </ErrorLine>
        ) : (
          <>
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-[13px]">
              <dt className="text-muted-foreground">
                {t('backupSchedule.change.scheduler')}
              </dt>
              <dd className="font-mono text-xs leading-5 break-all">
                {schedulerNames[plan.data.platform] ?? plan.data.platform} ·{' '}
                {plan.data.label}
              </dd>
            </dl>
            {plan.data.applySupported ? (
              <PlanFiles plan={plan.data} />
            ) : (
              <ManualSteps
                details={plan.data.manualStepDetails}
                steps={plan.data.manualSteps}
              />
            )}
          </>
        )}

        {apply.error && (
          <ErrorLine>
            {`${t('backupSchedule.change.failed')} ${describeError(apply.error, 'apply_schedule')}`}
          </ErrorLine>
        )}
      </div>

      <DialogFooter>
        <Button variant="ghost" disabled={apply.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          disabled={!ready || apply.isPending}
          onClick={() =>
            plan.data && hours !== null && confirm(plan.data, hours)
          }
        >
          {apply.isPending && <Spinner />}
          {apply.isPending ? t('backupSchedule.change.working') : confirmLabel}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}

/** Turn off, or remove an old job: lists what goes, then removes it. */
function RemoveForm({
  kind,
  status,
  onClose,
}: {
  kind: 'off' | 'legacy'
  status: ScheduleStatus | undefined
  onClose: () => void
}) {
  const { t } = useI18n()
  const plan = useCurrentSchedulePlan(true)
  const remove = useRemoveSchedule()
  const repair = useRepairLegacySchedule()
  const action = kind === 'off' ? remove : repair
  const files = status?.detectedFiles ?? []

  function confirm(target: SchedulePlan) {
    action.mutate(target, {
      onSuccess: () => {
        toast.success(
          kind === 'off'
            ? t('backup.schedule.turnedOff')
            : t('backup.schedule.repaired'),
        )
        onClose()
      },
    })
  }

  return (
    <DialogContent
      className="sm:max-w-lg"
      showCloseButton={!action.isPending}
      onInteractOutside={(event) => action.isPending && event.preventDefault()}
      onEscapeKeyDown={(event) => action.isPending && event.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>
          {kind === 'off'
            ? t('backupSchedule.off.title')
            : t('backupSchedule.legacy.title')}
        </DialogTitle>
        <DialogDescription>
          {kind === 'off'
            ? t('backupSchedule.off.body')
            : t('backupSchedule.legacy.body')}
        </DialogDescription>
      </DialogHeader>
      <div className="flex min-w-0 flex-col gap-2 text-[13px]">
        {files.length > 0 ? (
          <>
            <span className="text-muted-foreground">
              {t('backupSchedule.off.removes')}
            </span>
            <div className="flex flex-col gap-1 rounded-lg bg-muted px-3 py-2">
              {files.map((file) => (
                <PathLine key={file} path={file} />
              ))}
            </div>
          </>
        ) : (
          <span className="text-muted-foreground">
            {t('backupSchedule.off.noFiles')}
          </span>
        )}
        {plan.isError && (
          <ErrorLine>
            {`${t('backupSchedule.change.previewFailed')} ${describeError(plan.error, 'preview_schedule')}`}
          </ErrorLine>
        )}
        {action.error && (
          <ErrorLine>
            {`${t('backupSchedule.change.failed')} ${describeError(action.error, kind === 'off' ? 'remove_schedule' : 'repair_schedule')}`}
          </ErrorLine>
        )}
      </div>
      <DialogFooter>
        <Button variant="ghost" disabled={action.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          variant={kind === 'off' ? 'destructive' : 'default'}
          disabled={!plan.isSuccess || action.isPending}
          onClick={() => plan.data && confirm(plan.data)}
        >
          {action.isPending && <Spinner />}
          {action.isPending
            ? t('backupSchedule.change.working')
            : kind === 'off'
              ? t('backupSchedule.off.confirm')
              : t('backupSchedule.legacy.confirm')}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
