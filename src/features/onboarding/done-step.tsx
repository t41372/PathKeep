/**
 * Step 7: a summary of every choice, then the stages that carry them out
 * with live backup progress, errors in place, and the result.
 *
 * Not responsible for running the stages (see `use-finish-setup.ts`).
 */
import {
  Check,
  Folder,
  Globe,
  Lock,
  RefreshCw,
  Sparkles,
  X,
  type LucideIcon,
} from 'lucide-react'
import { useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { ProgressCard } from '@/features/backup/progress-card'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { isFullDiskAccessIssueMessage } from '@/lib/ipc/command-error'
import { useSnapshot } from '@/lib/queries/app'
import type { Draft } from './draft'
import { InlineError } from './inline-error'
import { skippable, type FinishState, type Stage } from './use-finish-setup'

export function DoneStep({
  draft,
  stages,
  finish,
  onRetry,
  onSkip,
}: {
  draft: Draft
  stages: Stage[]
  finish: FinishState
  onRetry: () => void
  onSkip: () => void
}) {
  const { t } = useI18n()

  return (
    <>
      <Summary draft={draft} keychain={stages.includes('keychain')} />
      {finish.status !== 'idle' && (
        <ol
          className="flex animate-rise flex-col gap-2.5"
          aria-label={t('onboarding.progressLabel')}
        >
          {stages.map((stage) => (
            <StageRow key={stage} stage={stage} finish={finish} />
          ))}
        </ol>
      )}
      {finish.current === 'backup' && finish.status === 'running' && (
        <ProgressCard />
      )}
      {finish.status === 'failed' && finish.error && (
        <InlineError
          title={t(`onboarding.done.failed.${finish.error.stage}`)}
          detail={finish.error.message}
          actions={
            <>
              <Button size="sm" onClick={onRetry}>
                {t('onboarding.done.retry')}
              </Button>
              {skippable.has(finish.error.stage) && (
                <Button size="sm" variant="outline" onClick={onSkip}>
                  {t(
                    `onboarding.done.skip.${finish.error.stage as Exclude<Stage, 'archive'>}`,
                  )}
                </Button>
              )}
            </>
          }
        />
      )}
      {finish.status === 'done' && <Result draft={draft} finish={finish} />}
    </>
  )
}

function Summary({ draft, keychain }: { draft: Draft; keychain: boolean }) {
  const { t, locale } = useI18n()
  const snapshot = useSnapshot()
  const browsers = useMemo(() => {
    const chosen = new Set(draft.selectedProfileIds)
    const names = snapshot.browserProfiles
      .filter((profile) => chosen.has(profile.profileId))
      .map((profile) => `${profile.browserName} (${profile.profileName})`)
    return new Intl.ListFormat(locale, { type: 'conjunction' }).format(names)
  }, [draft.selectedProfileIds, locale, snapshot.browserProfiles])

  const rows: { icon: LucideIcon; label: string; value: string }[] = [
    { icon: Globe, label: t('onboarding.done.browsers'), value: browsers },
    {
      icon: Folder,
      label: t('onboarding.done.location'),
      value: snapshot.directories.appRoot,
    },
    {
      icon: Lock,
      label: t('onboarding.done.encryption'),
      value: draft.encrypt
        ? [
            t('onboarding.done.encrypted'),
            keychain && t('onboarding.done.inKeychain'),
          ]
            .filter(Boolean)
            .join(' · ')
        : t('onboarding.done.notEncrypted'),
    },
    {
      icon: RefreshCw,
      label: t('onboarding.done.schedule'),
      value: t(`onboarding.schedule.${draft.frequency}.title`),
    },
    {
      icon: Sparkles,
      label: t('onboarding.done.ai'),
      value: t(`onboarding.ai.${draft.ai}.title`),
    },
  ]

  return (
    <dl className="overflow-hidden rounded-xl border bg-card shadow-card">
      {rows.map(({ icon: Icon, label, value }) => (
        <div
          key={label}
          className="flex items-center gap-3 border-b px-4 py-3 text-[13px] last:border-b-0"
        >
          <Icon
            className="size-4 shrink-0 text-muted-foreground"
            strokeWidth={1.75}
            aria-hidden
          />
          <dt className="w-28 shrink-0 text-muted-foreground">{label}</dt>
          <dd className="min-w-0 flex-1 truncate font-medium" title={value}>
            {value}
          </dd>
        </div>
      ))}
    </dl>
  )
}

function StageRow({ stage, finish }: { stage: Stage; finish: FinishState }) {
  const { t } = useI18n()
  const state = finish.completed.includes(stage)
    ? 'done'
    : finish.current === stage
      ? finish.status === 'failed'
        ? 'failed'
        : 'running'
      : 'pending'

  return (
    <li
      className={cn(
        'flex items-center gap-2.5 text-[13px] transition-colors duration-200',
        state === 'pending' && 'text-muted-foreground',
      )}
    >
      <span className="flex size-[18px] shrink-0 items-center justify-center">
        {state === 'done' ? (
          <span className="flex size-[18px] animate-rise items-center justify-center rounded-full bg-green text-white">
            <Check className="size-3" strokeWidth={3} aria-hidden />
          </span>
        ) : state === 'running' ? (
          <Spinner aria-hidden role={undefined} className="text-brand" />
        ) : state === 'failed' ? (
          <span className="flex size-[18px] items-center justify-center rounded-full bg-destructive text-white">
            <X className="size-3" strokeWidth={3} aria-hidden />
          </span>
        ) : (
          <span className="size-3 rounded-full border-[1.5px] border-border" />
        )}
      </span>
      <span>{t(`onboarding.done.stages.${stage}`)}</span>
      <span className="sr-only">
        {t(`onboarding.done.stageState.${state}`)}
      </span>
    </li>
  )
}

function Result({ draft, finish }: { draft: Draft; finish: FinishState }) {
  const { t } = useI18n()
  const report = finish.report
  const safariSkipped = report?.warnings.some(isFullDiskAccessIssueMessage)
  return (
    <div
      role="status"
      className="flex animate-rise flex-col gap-1.5 rounded-xl bg-green/10 px-4 py-3.5 text-[13px]"
    >
      <span className="flex items-center gap-2 font-medium">
        <Check className="size-4 text-green" strokeWidth={2.5} aria-hidden />
        {report
          ? t('onboarding.done.saved', { count: report.run?.newVisits ?? 0 })
          : t('onboarding.done.skippedBackup')}
      </span>
      {safariSkipped && (
        <span className="text-muted-foreground">
          {t('onboarding.done.safariSkipped')}
        </span>
      )}
      {draft.ai === 'local' && finish.completed.includes('ai') && (
        <span className="text-muted-foreground">
          {t('onboarding.done.localAi')}
        </span>
      )}
    </div>
  )
}
