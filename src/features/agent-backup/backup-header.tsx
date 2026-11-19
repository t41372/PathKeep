/**
 * The Agent backup page's header: what the archive holds, the last finished
 * backup (its dot follows how it ended; one that failed or did not read
 * everything opens its details), "Back up now", or while this kind backs up
 * a line of progress in real units and a bar that slides (no total is
 * known). Under it, the states the spec asks for: paused, this computer not
 * backing up yet, the first backup not finished, the last backup's problem.
 *
 * Not responsible for: starting the backup (`use-start-backup.ts`).
 */
import { HardDriveDownload, Info, TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { PageHeader } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import type {
  AgentOverview,
  ArchiveRun,
  KindTask,
} from '@/lib/backend-client/agent-backup'
import { cn } from '@/lib/cn'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { wireNumber } from '@/lib/queries/agent-backup'
import { useOpenEnableAgent } from '@/features/enable-agent/links'
import { useRunParam } from './run-param'
import { useStartAgentBackup } from './use-start-backup'
import { taskProgressText } from './words'

const lastWords: Record<string, MessageKey> = {
  success: 'agentBackup.last.success',
  partial: 'agentBackup.last.partial',
  failed: 'agentBackup.last.failed',
  interrupted: 'agentBackup.last.interrupted',
}

const dotColour: Record<string, string> = {
  success: 'bg-green',
  partial: 'bg-brand',
  interrupted: 'bg-brand',
  failed: 'bg-red',
}

function LastBackup({ run }: { run: ArchiveRun | undefined }) {
  const { t } = useI18n()
  const format = useFormat()
  const { open } = useRunParam()
  const pill =
    'flex items-center gap-1.5 rounded-full border bg-card px-3 py-1 text-xs text-muted-foreground shadow-card'
  if (!run) {
    return (
      <span className={pill}>
        <span
          aria-hidden
          className="size-1.5 rounded-full bg-muted-foreground/50"
        />
        {t('agentBackup.last.never')}
      </span>
    )
  }
  const time = format.dayAndTime(run.finishedAt ?? run.startedAt ?? 0)
  const text = t(lastWords[run.result] ?? 'agentBackup.last.failed', { time })
  const dot = (
    <span
      aria-hidden
      className={cn('size-1.5 rounded-full', dotColour[run.result] ?? 'bg-red')}
    />
  )
  if (run.result === 'success') {
    return (
      <span className={pill}>
        {dot}
        {text}
      </span>
    )
  }
  return (
    <button
      type="button"
      onClick={() => open(run.runRef)}
      className={cn(pill, 'transition-colors hover:text-foreground')}
    >
      {dot}
      {text}
    </button>
  )
}

function Running({ task }: { task: KindTask }) {
  const { t } = useI18n()
  const format = useFormat()
  const detail = taskProgressText(t, format.bytes, task)
  return (
    <span className="flex min-w-[260px] flex-col gap-1.5 rounded-lg border bg-card px-3 py-1.5 text-[13px] shadow-card">
      <span aria-live="polite" className="truncate tabular">
        {t('agentBackup.running', { detail })}
      </span>
      <Progress
        aria-label={t('agentBackup.runningLabel', {
          name: t('spaces.agent.name'),
        })}
        className="h-1 bg-muted [&>[data-slot=progress-indicator]]:bg-brand"
      />
    </span>
  )
}

function Note({
  tone = 'info',
  children,
  action,
}: {
  tone?: 'info' | 'warn'
  children: ReactNode
  action?: ReactNode
}) {
  const Icon = tone === 'warn' ? TriangleAlert : Info
  return (
    <div
      role="status"
      className={cn(
        'flex flex-wrap items-start gap-3 rounded-xl border px-5 py-3.5 text-[13px]',
        tone === 'warn' ? 'border-brand/25 bg-brand-soft' : 'bg-card',
      )}
    >
      <Icon
        aria-hidden
        className={cn(
          'mt-0.5 size-4 shrink-0',
          tone === 'warn' ? 'text-brand' : 'text-muted-foreground',
        )}
      />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">{children}</div>
      {action}
    </div>
  )
}

export function BackupHeader({
  overview,
  lastFinished,
  running,
  paused,
  viewOnly,
}: {
  overview: AgentOverview | undefined
  lastFinished: ArchiveRun | undefined
  running: KindTask | undefined
  paused: boolean
  viewOnly: boolean
}) {
  const { t } = useI18n()
  const backup = useStartAgentBackup()
  const openEnable = useOpenEnableAgent()
  const { open } = useRunParam()
  const name = t('spaces.agent.name')
  const first = overview?.firstBackup

  return (
    <>
      <PageHeader
        title={t('agentBackup.title')}
        subtitle={
          overview ? (
            t('agentBackup.subtitle', {
              name,
              count: t('agentBackup.conversations', {
                count: wireNumber(overview.roots),
              }),
            })
          ) : (
            <>
              <span className="sr-only">{t('agentBackup.countLoading')}</span>
              <Skeleton aria-hidden className="mt-1 h-3.5 w-72" />
            </>
          )
        }
        actions={
          <>
            {overview && <LastBackup run={lastFinished} />}
            {running ? (
              <Running task={running} />
            ) : (
              !viewOnly && (
                <Button
                  className="h-9"
                  disabled={backup.starting}
                  onClick={() => void backup.start()}
                >
                  <HardDriveDownload />
                  {t('agentBackup.now')}
                </Button>
              )
            )}
          </>
        }
      />
      {viewOnly ? (
        <Note
          action={
            <Button size="sm" onClick={openEnable}>
              {t('agentBackup.viewOnly.setUp')}
            </Button>
          }
        >
          <span className="font-medium">{t('agentBackup.viewOnly.title')}</span>
          <span className="text-muted-foreground">
            {t('agentBackup.viewOnly.body')}
          </span>
        </Note>
      ) : (
        paused && (
          <Note
            action={
              <Button size="sm" variant="outline" asChild>
                <Link to="/settings/kinds">
                  {t('agentBackup.paused.manage')}
                </Link>
              </Button>
            }
          >
            <span className="font-medium">{t('agentBackup.paused.title')}</span>
            <span className="text-muted-foreground">
              {t('agentBackup.paused.body')}
            </span>
          </Note>
        )
      )}
      {first === 'running' && <Note>{t('agentBackup.first.running')}</Note>}
      {(first === 'interrupted' || first === 'failed') && (
        <Note tone="warn">{t('agentBackup.first.notFinished')}</Note>
      )}
      {lastFinished &&
        (lastFinished.result === 'failed' ||
          lastFinished.result === 'partial') && (
          <Note
            tone="warn"
            action={
              <Button
                size="sm"
                variant="outline"
                onClick={() => open(lastFinished.runRef)}
              >
                {t('agentBackup.problem.see')}
              </Button>
            }
          >
            {t(
              lastFinished.result === 'failed'
                ? 'agentBackup.problem.failed'
                : 'agentBackup.problem.partial',
            )}
          </Note>
        )}
    </>
  )
}
