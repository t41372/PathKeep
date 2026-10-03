/**
 * Step 5: how often to back up, and a preview of exactly what the system
 * scheduler will get. The plan is only applied on the last step.
 *
 * Not responsible for building the plan (see `use-schedule-preview.ts`) or
 * installing it (see `use-finish-setup.ts`).
 */
import { ChevronRight, TriangleAlert } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { Skeleton } from '@/components/ui/skeleton'
import type { Frequency } from '@/features/backup/schedule-actions'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import type { SchedulePlan } from '@/lib/types'
import { ChoiceCard, ChoiceGroup, RadioMark } from './choice-card'
import { InlineError } from './inline-error'
import { useSchedulePreview } from './use-schedule-preview'

const options: Frequency[] = ['hourly', 'sixHours', 'daily', 'off']

const schedulerNames: Record<string, string> = {
  macos: 'launchd',
  windows: 'Task Scheduler',
  linux: 'systemd',
}

export function ScheduleStep({
  frequency,
  onChange,
  backgroundCanUnlock,
}: {
  frequency: Frequency
  onChange: (frequency: Frequency) => void
  backgroundCanUnlock: boolean
}) {
  const { t } = useI18n()
  const preview = useSchedulePreview(frequency)

  return (
    <>
      <ChoiceGroup
        value={frequency}
        onValueChange={onChange}
        label={t('onboarding.schedule.label')}
      >
        {options.map((option) => (
          <ChoiceCard
            key={option}
            value={option}
            leading={<RadioMark />}
            title={t(`onboarding.schedule.${option}.title`)}
            body={t(`onboarding.schedule.${option}.body`)}
            trailing={
              option === 'hourly' && (
                <Badge className="self-center rounded-md border-0 bg-brand-soft text-[11px] text-brand">
                  {t('onboarding.schedule.recommended')}
                </Badge>
              )
            }
          />
        ))}
      </ChoiceGroup>

      {frequency !== 'off' && !backgroundCanUnlock && (
        <p className="flex animate-rise items-start gap-2.5 rounded-lg bg-brand-soft px-3.5 py-3 text-[13px]">
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />
          {t('onboarding.schedule.noKeychain')}
        </p>
      )}

      {frequency !== 'off' &&
        (preview.isPending ? (
          <div
            className="flex flex-col gap-2 rounded-xl border bg-card p-4"
            aria-busy
            aria-label={t('onboarding.schedule.preview.loading')}
          >
            <Skeleton className="h-4 w-40" />
            <Skeleton className="h-4 w-full" />
          </div>
        ) : preview.isError ? (
          <InlineError
            title={t('onboarding.schedule.preview.failed')}
            detail={describeError(preview.error, 'preview_schedule')}
            actions={
              <Button
                size="sm"
                variant="outline"
                onClick={() => void preview.refetch()}
              >
                {t('onboarding.schedule.preview.retry')}
              </Button>
            }
          />
        ) : preview.data.applySupported ? (
          <PlanPreview plan={preview.data} />
        ) : (
          <div className="flex flex-col gap-0.5 rounded-xl border bg-card px-4 py-3.5 text-[13px]">
            <span className="font-medium">
              {t('onboarding.schedule.unsupported.title')}
            </span>
            <span className="text-muted-foreground">
              {t('onboarding.schedule.unsupported.body')}
            </span>
          </div>
        ))}
    </>
  )
}

function PlanPreview({ plan }: { plan: SchedulePlan }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const file = plan.generatedFiles[0]
  const path =
    plan.manualStepDetails?.find((step) => step.filePath)?.filePath ??
    file?.relativePath

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="flex animate-rise flex-col gap-2.5 rounded-xl border bg-card px-4 py-3.5 text-[13px] shadow-card"
    >
      <div className="flex flex-col gap-0.5">
        <span className="font-medium">
          {t('onboarding.schedule.preview.title')}
        </span>
        <span className="text-muted-foreground">
          {t('onboarding.schedule.preview.when')}
        </span>
      </div>
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5">
        <dt className="text-muted-foreground">
          {t('onboarding.schedule.preview.scheduler')}
        </dt>
        <dd className="font-mono text-xs leading-5">
          {schedulerNames[plan.platform] ?? plan.platform} · {plan.label}
        </dd>
        {path && (
          <>
            <dt className="text-muted-foreground">
              {t('onboarding.schedule.preview.file')}
            </dt>
            <dd className="font-mono text-xs leading-5 break-all">{path}</dd>
          </>
        )}
      </dl>
      {file && (
        <>
          <CollapsibleTrigger asChild>
            <button
              type="button"
              className="flex w-fit items-center gap-1 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
            >
              <ChevronRight
                className={cn(
                  'size-3.5 transition-transform duration-150',
                  open && 'rotate-90',
                )}
              />
              {t(
                open
                  ? 'onboarding.schedule.preview.hideFile'
                  : 'onboarding.schedule.preview.showFile',
              )}
            </button>
          </CollapsibleTrigger>
          <CollapsibleContent>
            <pre className="max-h-56 overflow-auto rounded-lg bg-muted p-3 font-mono text-[11px] leading-relaxed whitespace-pre">
              {file.contents}
            </pre>
          </CollapsibleContent>
        </>
      )}
    </Collapsible>
  )
}
