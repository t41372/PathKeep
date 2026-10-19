/**
 * Verify: everything PathKeep knows about the automatic backup, in one
 * sheet. The checks the backend ran, problems with what they mean, the
 * files found, the last install / remove and its record, recent runs the
 * scheduler started, and the manual steps for doing it by hand.
 *
 * Reads `schedule_status` only; changes go through the card's dialog.
 */
import {
  CircleAlert,
  CircleCheck,
  CircleDashed,
  RefreshCw,
  TriangleAlert,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import type {
  ScheduleIssue,
  ScheduleStatus,
  ScheduleVerificationCheck,
} from '@/lib/types'
import { PathLine } from './path-actions'
import { scheduleCopy } from './schedule-copy'
import { ManualSteps } from './schedule-plan-view'

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex min-w-0 flex-col gap-2">
      <h3 className="text-[13px] font-medium">{title}</h3>
      {children}
    </section>
  )
}

function Empty({ children }: { children: ReactNode }) {
  return <p className="text-[13px] text-muted-foreground">{children}</p>
}

const checkIcons = {
  ok: <CircleCheck className="size-4 text-green" />,
  warning: <TriangleAlert className="size-4 text-brand" />,
  error: <CircleAlert className="size-4 text-destructive" />,
  pending: <CircleDashed className="size-4 text-muted-foreground" />,
}

function CheckRow({ check }: { check: ScheduleVerificationCheck }) {
  const { t } = useI18n()
  const status = check.status in checkIcons ? check.status : 'pending'
  return (
    <li className="flex items-start gap-2.5 py-1.5">
      <span className="mt-0.5" aria-hidden>
        {checkIcons[status]}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5 text-[13px]">
        <span className="flex items-baseline justify-between gap-3">
          <span className="font-medium">
            {scheduleCopy(t, check.labelKey) ?? check.key}
          </span>
          <span className="shrink-0 text-xs text-muted-foreground">
            {t(`backupSchedule.verify.status.${status}`)}
          </span>
        </span>
        {scheduleCopy(t, check.detailKey) && (
          <span className="text-muted-foreground">
            {scheduleCopy(t, check.detailKey)}
          </span>
        )}
        {check.evidence.length > 0 && (
          <span className="font-mono text-[11px] break-all text-muted-foreground">
            {check.evidence.join(' · ')}
          </span>
        )}
      </div>
    </li>
  )
}

function IssueRow({ issue }: { issue: ScheduleIssue }) {
  const { t } = useI18n()
  return (
    <li
      className={cn(
        'flex flex-col gap-0.5 rounded-lg p-3 text-[13px]',
        issue.severity === 'error' ? 'bg-destructive/10' : 'bg-brand-soft',
      )}
    >
      <span className="font-medium">
        {scheduleCopy(t, issue.titleKey) ?? issue.code}
      </span>
      {[issue.detailKey, issue.consequenceKey].map((key) => {
        const text = scheduleCopy(t, key)
        return text ? (
          <span key={key} className="text-muted-foreground">
            {text}
          </span>
        ) : null
      })}
      {issue.evidence.length > 0 && (
        <span className="font-mono text-[11px] break-all text-muted-foreground">
          {issue.evidence.join(' · ')}
        </span>
      )}
    </li>
  )
}

const attemptOutcomes = [
  'running',
  'success',
  'skipped',
  'deferred',
  'failed',
  'interrupted',
] as const

function LastChange({ status }: { status: ScheduleStatus }) {
  const { t } = useI18n()
  const format = useFormat()
  const last = status.lastAction
  if (!last) return <Empty>{t('backupSchedule.verify.noLastChange')}</Empty>
  const action = (['apply', 'remove', 'repair'] as const).find(
    (value) => value === last.action,
  )
  const outcome = (['ok', 'failed', 'unknown'] as const).find(
    (value) => value === last.status,
  )
  return (
    <div className="flex flex-col gap-1.5 text-[13px]">
      <span>
        <span className="font-medium">
          {action ? t(`backupSchedule.verify.action.${action}`) : last.action}
        </span>
        <span className="text-muted-foreground">
          {' · '}
          {t(`backupSchedule.verify.outcome.${outcome ?? 'unknown'}`)}
          {' · '}
          {format.dayAndTime(last.at)}
        </span>
      </span>
      {(last.files ?? []).map((file) => (
        <PathLine key={file} path={file} />
      ))}
      {last.auditPath && (
        <div className="flex flex-col gap-0.5">
          <span className="text-xs text-muted-foreground">
            {t('backupSchedule.verify.record')}
          </span>
          <PathLine path={last.auditPath} />
        </div>
      )}
    </div>
  )
}

export function ScheduleDetailsSheet({
  open,
  status,
  checking,
  onCheck,
  onClose,
}: {
  open: boolean
  status: ScheduleStatus
  checking: boolean
  onCheck: () => void
  onClose: () => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const checks = status.verificationChecks ?? []
  const issues = status.issues ?? []
  const attempts = status.recentAttempts ?? []

  return (
    <Sheet open={open} onOpenChange={(next) => !next && onClose()}>
      <SheetContent className="w-[460px] overflow-y-auto sm:max-w-[460px]">
        <SheetHeader>
          <SheetTitle>{t('backupSchedule.verify.title')}</SheetTitle>
          <SheetDescription>
            {t('backupSchedule.verify.description')}
          </SheetDescription>
        </SheetHeader>
        <div className="flex flex-col gap-5 px-4 pb-6">
          <div className="flex items-center justify-between gap-3 text-[13px] text-muted-foreground">
            <span>
              {status.checkedAt
                ? t('backupSchedule.verify.checkedAt', {
                    time: format.dayAndTime(status.checkedAt),
                  })
                : null}
            </span>
            <Button
              size="sm"
              variant="outline"
              disabled={checking}
              onClick={onCheck}
            >
              <RefreshCw className={cn(checking && 'animate-spin')} />
              {checking
                ? t('backupSchedule.verify.checking')
                : t('backupSchedule.verify.checkAgain')}
            </Button>
          </div>

          {issues.length > 0 && (
            <Section title={t('backupSchedule.verify.problems')}>
              <ul className="flex flex-col gap-2">
                {issues.map((issue) => (
                  <IssueRow
                    key={`${issue.code}-${issue.titleKey}`}
                    issue={issue}
                  />
                ))}
              </ul>
            </Section>
          )}

          <Section title={t('backupSchedule.verify.checks')}>
            {checks.length === 0 ? (
              <Empty>{t('backupSchedule.verify.noChecks')}</Empty>
            ) : (
              <ul className="flex flex-col">
                {checks.map((check) => (
                  <CheckRow key={check.key} check={check} />
                ))}
              </ul>
            )}
          </Section>

          <Section title={t('backupSchedule.verify.files')}>
            {status.detectedFiles.length === 0 ? (
              <Empty>{t('backupSchedule.verify.noFiles')}</Empty>
            ) : (
              <div className="flex flex-col gap-1">
                {status.detectedFiles.map((file) => (
                  <PathLine key={file} path={file} />
                ))}
              </div>
            )}
          </Section>

          <Section title={t('backupSchedule.verify.lastChange')}>
            <LastChange status={status} />
          </Section>

          <Section title={t('backupSchedule.verify.attempts')}>
            {attempts.length === 0 ? (
              <Empty>{t('backupSchedule.verify.noAttempts')}</Empty>
            ) : (
              <ul className="flex flex-col text-[13px]">
                {attempts.slice(0, 8).map((attempt) => {
                  const outcome = attemptOutcomes.find(
                    (value) => value === attempt.outcome,
                  )
                  return (
                    <li
                      key={attempt.id}
                      className="flex justify-between gap-3 py-1"
                    >
                      <span
                        className={cn(
                          (outcome === 'failed' || outcome === 'interrupted') &&
                            'text-destructive',
                        )}
                      >
                        {outcome
                          ? t(`backupSchedule.verify.attempt.${outcome}`)
                          : attempt.outcome}
                      </span>
                      <span className="font-mono text-xs text-muted-foreground tabular">
                        {format.dayAndTime(attempt.startedAt)}
                      </span>
                    </li>
                  )
                })}
              </ul>
            )}
          </Section>

          <Section title={t('backupSchedule.verify.manual')}>
            <p className="text-[13px] text-muted-foreground">
              {t('backupSchedule.verify.manualBody')}
            </p>
            <ManualSteps
              details={status.manualStepDetails}
              steps={status.manualSteps}
            />
          </Section>
        </div>
      </SheetContent>
    </Sheet>
  )
}
