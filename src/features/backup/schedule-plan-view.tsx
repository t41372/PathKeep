/**
 * The files of a schedule plan exactly as the scheduler will get them, with
 * where they go and copy buttons; and the manual steps that install the same
 * thing by hand. Used by the install dialog and the details sheet.
 */
import type { ScheduleManualStep, SchedulePlan } from '@/lib/types'
import { useI18n } from '@/lib/i18n'
import { CopyButton, PathLine } from './path-actions'
import { scheduleCopy } from './schedule-copy'

/** Where a generated file goes, from the manual step that saves it. */
function destination(plan: SchedulePlan, contents: string, fallback: string) {
  return (
    plan.manualStepDetails?.find((step) => step.fileContents === contents)
      ?.filePath ?? fallback
  )
}

export function FileBlock({
  path,
  contents,
}: {
  path: string
  contents: string
}) {
  const { t } = useI18n()
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <PathLine path={path} />
      <div className="relative">
        <pre
          aria-label={t('backupSchedule.change.contents')}
          className="max-h-56 overflow-auto rounded-lg bg-muted p-3 pr-10 font-mono text-[11px] leading-relaxed whitespace-pre"
        >
          {contents}
        </pre>
        <CopyButton
          text={contents}
          label={t('backupSchedule.change.copyFile')}
          className="absolute top-1.5 right-1.5 bg-muted"
        />
      </div>
    </div>
  )
}

export function PlanFiles({ plan }: { plan: SchedulePlan }) {
  return (
    <div className="flex flex-col gap-3">
      {plan.generatedFiles.map((file) => (
        <FileBlock
          key={file.relativePath}
          path={destination(
            plan,
            file.contents,
            file.absolutePath ?? file.relativePath,
          )}
          contents={file.contents}
        />
      ))}
    </div>
  )
}

function StepBody({ step }: { step: ScheduleManualStep }) {
  const { t } = useI18n()
  const command = step.command?.join(' ')
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <span className="font-medium">
        {scheduleCopy(t, step.titleKey) ?? step.id}
      </span>
      {scheduleCopy(t, step.summaryKey) && (
        <span className="text-muted-foreground">
          {scheduleCopy(t, step.summaryKey)} {scheduleCopy(t, step.whyKey)}
        </span>
      )}
      {step.fileContents && step.filePath && (
        <FileBlock path={step.filePath} contents={step.fileContents} />
      )}
      {command && (
        <div className="flex items-start gap-1 rounded-lg bg-muted px-3 py-2">
          <code className="min-w-0 flex-1 font-mono text-[11px] leading-5 break-all">
            {command}
          </code>
          <CopyButton
            text={command}
            label={t('backupSchedule.verify.copyCommand')}
          />
        </div>
      )}
    </div>
  )
}

/** Numbered manual steps; falls back to the plain strings when there are no details. */
export function ManualSteps({
  details,
  steps,
}: {
  details?: ScheduleManualStep[]
  steps: string[]
}) {
  if (details && details.length > 0) {
    return (
      <ol className="flex list-decimal flex-col gap-3 pl-4 text-[13px]">
        {details.map((step) => (
          <li key={step.id}>
            <StepBody step={step} />
          </li>
        ))}
      </ol>
    )
  }
  return (
    <ol className="flex list-decimal flex-col gap-1 pl-4 font-mono text-xs break-all">
      {steps.map((step) => (
        <li key={step}>{step}</li>
      ))}
    </ol>
  )
}
