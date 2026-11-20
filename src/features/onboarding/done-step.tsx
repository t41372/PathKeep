/**
 * Step 8: a summary of every choice, then the stages that carry them out
 * with live backup progress, errors in place, and the result.
 *
 * With browsing history the only kind chosen, the first backup shows as
 * it always has (the shared progress card, "Done. N visits saved."). With
 * Agent conversations chosen, it shows one line per kind instead, each in
 * its own unit (`first-backups.tsx`).
 * Not responsible for running the stages (see `use-finish-setup.ts`).
 */
import {
  Bot,
  Check,
  Earth,
  Folder,
  Globe,
  Layers,
  Lock,
  Minus,
  RefreshCw,
  Sparkles,
  X,
  type LucideIcon,
} from 'lucide-react'
import { useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { ProgressCard } from '@/features/backup/progress-card'
import {
  leftOutOtherThanSafari,
  skippedSafariForFullDiskAccess,
} from '@/features/backup/run-codes'
import { toolName } from '@/features/data-kinds/tools'
import type { AgentToolId, DataKindId } from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { spaceOfKind } from '@/lib/spaces'
import { readableSelection, type Draft } from './draft'
import { KindProgress, KindResults } from './first-backups'
import { InlineError } from './inline-error'
import { skippable, type FinishState, type Stage } from './use-finish-setup'

export function DoneStep({
  draft,
  kinds,
  agentTools,
  stages,
  finish,
  onRetry,
  onSkip,
  onOpenBackup,
  onKeepGoing,
}: {
  draft: Draft
  kinds: DataKindId[]
  /** The ticked Agent tools PathKeep can read. */
  agentTools: AgentToolId[]
  stages: Stage[]
  finish: FinishState
  onRetry: () => void
  onSkip: () => void
  onOpenBackup: (kind: DataKindId) => void
  /** Lets the Agent save go on in the background; setup opens the app once the rest is done. */
  onKeepGoing: () => void
}) {
  const { t } = useI18n()
  const onlyBrowsing = kinds.length === 1 && kinds[0] === 'browsing'
  const backingUp = finish.current === 'backup' && finish.status === 'running'

  return (
    <>
      <Summary
        draft={draft}
        kinds={kinds}
        agentTools={agentTools}
        keychain={stages.includes('keychain')}
      />
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
      {backingUp &&
        (onlyBrowsing ? (
          <ProgressCard />
        ) : (
          <KindProgress
            kinds={kinds}
            runs={finish.runs}
            onKeepGoing={onKeepGoing}
          />
        ))}
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
      {finish.status === 'done' && (
        <Result
          draft={draft}
          kinds={kinds}
          finish={finish}
          onOpenBackup={onOpenBackup}
        />
      )}
    </>
  )
}

function Summary({
  draft,
  kinds,
  agentTools,
  keychain,
}: {
  draft: Draft
  kinds: DataKindId[]
  agentTools: AgentToolId[]
  keychain: boolean
}) {
  const { t, locale } = useI18n()
  const snapshot = useSnapshot()
  const list = useMemo(
    () => new Intl.ListFormat(locale, { type: 'conjunction' }),
    [locale],
  )
  const browsers = useMemo(() => {
    const names = readableSelection(draft, snapshot).map(
      (profile) => `${profile.browserName} (${profile.profileName})`,
    )
    return t('onboarding.done.browsersValue', {
      count: names.length,
      names: list.format(names),
    })
  }, [draft, list, snapshot, t])
  const tools = t('onboarding.done.agentValue', {
    count: agentTools.length,
    names: list.format(agentTools.map((tool) => t(toolName(tool)))),
  })
  const browsing = kinds.includes('browsing')

  const rows: { icon: LucideIcon; label: string; value: string }[] = [
    {
      icon: Layers,
      label: t('onboarding.done.kinds'),
      value: list.format(kinds.map((kind) => t(spaceOfKind(kind)!.name))),
    },
    ...(browsing
      ? [{ icon: Globe, label: t('onboarding.done.browsers'), value: browsers }]
      : []),
    ...(kinds.includes('agent-history')
      ? [{ icon: Bot, label: t('onboarding.done.agentTools'), value: tools }]
      : []),
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
    // Link previews are fetched from each site by default; say so before the
    // first backup starts them, not only in Settings. Only browsing goes online.
    ...(browsing
      ? [
          {
            icon: Earth,
            label: t('onboarding.done.online'),
            value: t('onboarding.done.onlineValue'),
          },
        ]
      : []),
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
  const state = finish.skipped.includes(stage)
    ? 'skipped'
    : finish.completed.includes(stage)
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
        (state === 'pending' || state === 'skipped') && 'text-muted-foreground',
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
        ) : state === 'skipped' ? (
          <Minus className="size-3.5" strokeWidth={2.5} aria-hidden />
        ) : (
          <span className="size-3 rounded-full border-[1.5px] border-border" />
        )}
      </span>
      <span className={cn(state === 'skipped' && 'line-through')}>
        {t(`onboarding.done.stages.${stage}`)}
      </span>
      <span className="sr-only">
        {t(`onboarding.done.stageState.${state}`)}
      </span>
    </li>
  )
}

function Result({
  draft,
  kinds,
  finish,
  onOpenBackup,
}: {
  draft: Draft
  kinds: DataKindId[]
  finish: FinishState
  onOpenBackup: (kind: DataKindId) => void
}) {
  const { t } = useI18n()
  const skipped = finish.skipped.includes('backup')
  const browsing = finish.runs.browsing
  const report = browsing?.status === 'ended' ? browsing.report : null
  const onlyBrowsing = kinds.length === 1 && kinds[0] === 'browsing'
  const allEnded = kinds.every(
    (kind) =>
      (kind === 'browsing' ? finish.runs.browsing : finish.runs.agent)
        ?.status === 'ended',
  )
  const safariSkipped = report ? skippedSafariForFullDiskAccess(report) : false
  // A first backup that saved some browsers and not others finishes setup, and says so.
  const leftOut = report ? leftOutOtherThanSafari(report) : false

  return (
    <div
      role="status"
      className="flex animate-rise flex-col gap-1.5 rounded-xl bg-green/10 px-4 py-3.5 text-[13px]"
    >
      <span className="flex items-center gap-2 font-medium">
        <Check className="size-4 text-green" strokeWidth={2.5} aria-hidden />
        {skipped
          ? t('onboarding.done.skippedBackup')
          : onlyBrowsing
            ? t('onboarding.done.saved', { count: report?.run?.newVisits ?? 0 })
            : t(
                allEnded
                  ? 'onboarding.done.allDone'
                  : 'onboarding.done.finished',
              )}
      </span>
      {!skipped && !onlyBrowsing && (
        <KindResults
          kinds={kinds}
          runs={finish.runs}
          onOpenBackup={onOpenBackup}
        />
      )}
      {safariSkipped && (
        <span className="text-muted-foreground">
          {t('onboarding.done.safariSkipped')}
        </span>
      )}
      {leftOut && (
        <span className="text-muted-foreground">
          {t('onboarding.done.partial')}
        </span>
      )}
      {finish.browsingStillOn && (
        <span className="text-muted-foreground">
          {t('onboarding.done.browsingStillOn')}
        </span>
      )}
      {draft.ai === 'local' &&
        finish.completed.includes('ai') &&
        !finish.skipped.includes('ai') && (
          <span className="text-muted-foreground">
            {t('onboarding.done.localAi')}
          </span>
        )}
    </div>
  )
}
