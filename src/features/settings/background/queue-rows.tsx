/**
 * The queue controls and the two queue rows of Settings → Background work:
 * pause / resume and tasks-at-a-time (`config.ai.jobQueuePaused` /
 * `jobQueueConcurrency`, which govern both queues), the Insights queue with
 * its rebuild button, and the search-index / assistant queue.
 *
 * Not responsible for polling rules or job naming (see the hook and
 * `job-view`).
 */
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { Switch } from '@/components/ui/switch'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { RowSelect } from '../row-select'
import { useSaveSetting } from '../use-save-setting'
import { JobList } from './job-list'
import { useJobViews } from './job-view'
import {
  useAiJobActions,
  useAiQueue,
  useIntelligenceRuntime,
  useQueueRebuild,
  useRefreshQueues,
  useRuntimeJobActions,
} from './use-background-work'

const CONCURRENCY_CHOICES = [1, 2, 3, 4]

/** "2 running · 3 waiting · 1 failed", or "Nothing running." */
function useCountSummary() {
  const { t } = useI18n()
  return (counts: { running: number; queued: number; failed: number }) => {
    const parts = [
      counts.running > 0 &&
        t('settingsBackground.counts.running', { count: counts.running }),
      counts.queued > 0 &&
        t('settingsBackground.counts.queued', { count: counts.queued }),
      counts.failed > 0 &&
        t('settingsBackground.counts.failed', { count: counts.failed }),
    ].filter(Boolean)
    return parts.length > 0
      ? parts.join(' · ')
      : t('settingsBackground.counts.idle')
  }
}

export function QueueControls() {
  const { t } = useI18n()
  const ai = useSnapshot().config.ai
  const { save, saving } = useSaveSetting()
  const refresh = useRefreshQueues()

  const setPaused = async (paused: boolean) => {
    const saved = await save((config) => {
      config.ai.jobQueuePaused = paused
      return config
    })
    if (!saved) return
    toast.success(
      paused
        ? t('settingsBackground.run.pausedToast')
        : t('settingsBackground.run.resumedToast'),
    )
    await refresh()
  }

  const setConcurrency = async (value: string) => {
    const saved = await save((config) => {
      config.ai.jobQueueConcurrency = Number(value)
      return config
    })
    if (saved) await refresh()
  }

  const current = Math.max(1, ai.jobQueueConcurrency)
  const choices = CONCURRENCY_CHOICES.includes(current)
    ? CONCURRENCY_CHOICES
    : [...CONCURRENCY_CHOICES, current].sort((a, b) => a - b)

  return (
    <>
      <SettingRow
        title={t('settingsBackground.run.title')}
        description={
          ai.jobQueuePaused
            ? t('settingsBackground.run.paused')
            : t('settingsBackground.run.on')
        }
        htmlFor="settings-background-run"
        control={
          <Switch
            id="settings-background-run"
            checked={!ai.jobQueuePaused}
            disabled={saving}
            onCheckedChange={(on) => void setPaused(!on)}
          />
        }
      />
      <SettingRow
        title={t('settingsBackground.concurrency.title')}
        description={t('settingsBackground.concurrency.description')}
        htmlFor="settings-background-concurrency"
        control={
          <RowSelect
            id="settings-background-concurrency"
            value={String(current)}
            disabled={saving}
            onChange={(value) => void setConcurrency(value)}
            options={choices.map((count) => ({
              value: String(count),
              label: String(count),
            }))}
          />
        }
      />
    </>
  )
}

export function InsightsQueueRow() {
  const { t } = useI18n()
  const paused = useSnapshot().config.ai.jobQueuePaused
  const runtime = useIntelligenceRuntime(paused)
  const { fromRuntime } = useJobViews()
  const { retry, cancel } = useRuntimeJobActions()
  const rebuild = useQueueRebuild()
  const summarize = useCountSummary()
  const [confirm, setConfirm] = useState(false)

  const data = runtime.data
  return (
    <SettingRow
      title={t('settingsBackground.insights.title')}
      description={
        runtime.isPending ? (
          <Skeleton className="mt-0.5 h-3.5 w-40" />
        ) : runtime.isError ? (
          <span className="text-destructive">
            {t('settingsBackground.loadFailed', {
              message: describeError(
                runtime.error,
                'load_intelligence_runtime',
              ),
            })}
          </span>
        ) : (
          <span className="flex flex-col">
            <span>{t('settingsBackground.insights.description')}</span>
            <span aria-live="polite">{summarize(data!.queue)}</span>
          </span>
        )
      }
      control={
        <Button
          size="sm"
          variant="outline"
          disabled={rebuild.isPending}
          onClick={() => setConfirm(true)}
        >
          {rebuild.isPending && <Spinner />}
          {t('settingsBackground.insights.rebuild')}
        </Button>
      }
    >
      {data && (
        <JobList
          jobs={data.recentJobs.map(fromRuntime)}
          busy={retry.isPending || cancel.isPending}
          onRetry={(job) => retry.mutate(job.id)}
          onCancel={(job) => cancel.mutate(job.id)}
        />
      )}
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('settingsBackground.insights.confirmTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('settingsBackground.insights.confirmBody')}
              {paused && ` ${t('settingsBackground.insights.pausedNote')}`}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => rebuild.mutate()}>
              {t('settingsBackground.insights.rebuild')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}

export function AiQueueRow() {
  const { t } = useI18n()
  const ai = useSnapshot().config.ai
  const queue = useAiQueue()
  const { fromAiQueue } = useJobViews()
  const { replay, cancel } = useAiJobActions()
  const summarize = useCountSummary()

  const data = queue.data
  const aiOn = ai.enabled && (ai.semanticIndexEnabled || ai.assistantEnabled)
  const hasJobs = (data?.recentJobs.length ?? 0) > 0

  return (
    <SettingRow
      title={t('settingsBackground.ai.title')}
      description={
        queue.isPending ? (
          <Skeleton className="mt-0.5 h-3.5 w-40" />
        ) : queue.isError ? (
          <span className="text-destructive">
            {t('settingsBackground.loadFailed', {
              message: describeError(queue.error, 'load_ai_queue_status'),
            })}
          </span>
        ) : !aiOn && !hasJobs ? (
          t('settingsBackground.ai.off')
        ) : (
          <span className="flex flex-col">
            <span>{t('settingsBackground.ai.description')}</span>
            <span aria-live="polite">{summarize(data!)}</span>
          </span>
        )
      }
      control={
        !aiOn && (
          <Button size="sm" variant="outline" asChild>
            <Link to="/settings/ai">{t('settingsBackground.ai.openAi')}</Link>
          </Button>
        )
      }
    >
      {data && (aiOn || hasJobs) && (
        <JobList
          jobs={data.recentJobs.map(fromAiQueue)}
          busy={replay.isPending || cancel.isPending}
          retryLabel={{
            text: t('settingsBackground.ai.replay'),
            aria: (name) => t('settingsBackground.ai.replayLabel', { name }),
          }}
          onRetry={(job) => replay.mutate(job.id)}
          onCancel={(job) => cancel.mutate(job.id)}
        />
      )}
    </SettingRow>
  )
}
