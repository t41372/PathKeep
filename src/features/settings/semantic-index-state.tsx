/**
 * The chosen model's semantic index under the semantic search switch: its
 * build (progress, pace, pause, continue, cancel), what it still needs and
 * what search reads meanwhile, how complete it is, whether it was made from
 * an older version of the page text, and whether backups keep it up to date.
 * Behavior comes from `useSemanticIndex`; this file only renders it.
 */
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Progress } from '@/components/ui/progress'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { RebuildDialog } from './rebuild-dialog'
import type { SemanticIndex } from './semantic-row'
import { useRebuild } from './use-rebuild'

/** The chosen model's index: its build, or what it needs, or how complete it is. */
export function IndexState({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const format = useFormat()
  const { build, progress, semantic, status } = index

  if (build && progress) {
    const backgroundLink = (
      <Button size="xs" variant="ghost" asChild>
        <Link to="/settings/background">
          {t('settingsAi.semantic.inBackground')}
        </Link>
      </Button>
    )
    const cancel = (
      <Button
        size="xs"
        variant="ghost"
        disabled={index.acting}
        onClick={() => index.cancel(build.jobId)}
      >
        {t('settingsAi.semantic.cancelBuild')}
      </Button>
    )
    const counts =
      progress.total > 0
        ? t('settingsAi.semantic.indexProgress', {
            done: format.number(progress.done),
            total: format.number(progress.total),
          })
        : undefined

    if (build.state === 'running') {
      return (
        <ProgressLine
          label={t('settingsAi.semantic.indexing')}
          detail={[counts, pace(t, format, progress)]
            .filter(Boolean)
            .join(' · ')}
          value={
            progress.total > 0 ? progress.done / progress.total : undefined
          }
          action={
            <>
              <Button
                size="xs"
                variant="outline"
                disabled={index.acting}
                onClick={() => index.pause(build.jobId)}
              >
                {t('settingsAi.semantic.pause')}
              </Button>
              {cancel}
              {backgroundLink}
            </>
          }
        />
      )
    }
    if (build.state === 'queued') {
      return (
        <ProgressLine
          label={t('settingsAi.semantic.indexWaiting')}
          action={
            <>
              {cancel}
              {backgroundLink}
            </>
          }
        />
      )
    }
    // Paused: by the user, by the service, or with all background work.
    const byService = build.pausedBy === 'service'
    const byQueue = build.pausedBy === 'queue'
    return (
      <div role="status" className="flex flex-col gap-2 text-[13px]">
        <p
          className={cn(
            '[overflow-wrap:anywhere]',
            byService ? 'text-destructive' : 'text-muted-foreground',
          )}
        >
          {byService
            ? t('settingsAi.semantic.pausedByService', {
                name: index.service?.name ?? chosenName(index),
                message: build.pauseMessage ?? '',
              })
            : byQueue
              ? t('settingsAi.semantic.pausedByQueue')
              : t('settingsAi.semantic.pausedByUser', {
                  done: format.number(progress.done),
                  total: format.number(progress.total),
                })}
        </p>
        <div className="flex justify-end gap-2">
          {byQueue ? (
            backgroundLink
          ) : (
            <>
              {cancel}
              <Button
                size="xs"
                variant="outline"
                disabled={index.acting}
                onClick={() => index.resume(build.jobId)}
              >
                {t('settingsAi.semantic.continue')}
              </Button>
            </>
          )}
        </div>
      </div>
    )
  }

  if (index.setupRunning) {
    return <ProgressLine label={t('settingsAi.semantic.indexWaiting')} />
  }

  // The queue has not answered yet: what the index needs is not known.
  if (!semantic) return <Skeleton className="h-4 w-56" />

  if (!semantic.chosenComplete) {
    return <NeedsBuild index={index} />
  }

  if (status.state === 'failed') {
    return (
      <p className="text-[13px] text-destructive">
        {t('settingsAi.semantic.indexFailed')}
      </p>
    )
  }
  if (status.state === 'degraded') {
    return (
      <p className="text-[13px] text-destructive">
        {t('settingsAi.semantic.indexIncomplete')}
      </p>
    )
  }

  const summary = (
    <p className="text-[13px] text-muted-foreground">
      {status.indexedItems > 0
        ? [
            t('settingsAi.semantic.indexed', { count: status.indexedItems }),
            status.lastIndexedAt &&
              t('settingsAi.semantic.lastIndexed', {
                when: format.relative(status.lastIndexedAt),
              }),
          ]
            .filter(Boolean)
            .join(' · ')
        : t('settingsAi.semantic.nothingIndexed')}
    </p>
  )
  if (index.staleReason !== 'textFormat') return summary
  return (
    <div className="flex flex-col gap-2 text-[13px]">
      {summary}
      <p className="text-foreground">{t('settingsAi.semantic.olderText')}</p>
      <div className="flex justify-end">
        <StartOver
          index={index}
          label={t('settingsAi.semantic.rebuildIndex')}
        />
      </div>
    </div>
  )
}

/**
 * Rebuilds the chosen model's index from nothing, after the same confirmation
 * as Rebuild search index. Offered when the index was made from an older
 * version of the page text, which updating it never changes.
 */
function StartOver({ index, label }: { index: SemanticIndex; label: string }) {
  const [open, setOpen] = useState(false)
  const rebuild = useRebuild(index)
  return (
    <>
      <Button
        size="sm"
        variant="outline"
        disabled={index.acting || rebuild.isPending}
        onClick={() => setOpen(true)}
      >
        {label}
      </Button>
      <RebuildDialog
        open={open}
        onOpenChange={setOpen}
        onConfirm={() => rebuild.mutate()}
      />
    </>
  )
}

/**
 * The chosen model has no complete index and nothing is building it: what
 * building it means, what search uses meanwhile, and the button to start.
 */
function NeedsBuild({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const format = useFormat()
  const started = index.status.indexedItems > 0
  const estimate = useQuery({
    queryKey: ['ai', 'reembed-estimate', 'full', index.modelId],
    queryFn: () => intelligenceClient.estimateReembed('full'),
    enabled: !started,
    staleTime: 60_000,
  })
  const model = chosenName(index)
  const olderText = started && index.staleReason === 'textFormat'
  const serving = index.semantic?.serving
  const servingOther =
    serving &&
    !(
      serving.providerId === index.semantic?.chosen?.providerId &&
      serving.model === index.semantic?.chosen?.model
    )
      ? serving
      : null

  return (
    <div className="flex flex-col gap-2 text-[13px]">
      <p className="text-foreground">
        {started
          ? t('settingsAi.semantic.unfinished', {
              model,
              count: index.status.indexedItems,
            })
          : estimate.data && estimate.data.pageCount > 0
            ? t('settingsAi.semantic.needsBuild', {
                model,
                count: format.number(estimate.data.pageCount),
              })
            : t('settingsAi.semantic.needsBuildCounting', { model })}
      </p>
      {olderText && (
        <p className="text-foreground">
          {t('settingsAi.semantic.olderTextUnfinished')}
        </p>
      )}
      <p className="text-muted-foreground">
        {servingOther
          ? t('settingsAi.semantic.meanwhilePrevious', {
              model: shortModel(servingOther.model),
            })
          : t('settingsAi.semantic.meanwhilePartial')}
      </p>
      <div className="flex justify-end gap-2">
        {olderText && (
          <StartOver index={index} label={t('settingsAi.semantic.startOver')} />
        )}
        <Button
          size="sm"
          variant="outline"
          disabled={index.acting}
          onClick={index.start}
        >
          {started
            ? t('settingsAi.semantic.continue')
            : t('settingsAi.semantic.build')}
        </Button>
      </div>
    </div>
  )
}

/** Whether each backup's new visits are indexed; it starts once the index is complete. */
export function AutoUpdate({ index }: { index: SemanticIndex }) {
  const { t } = useI18n()
  const on = useSnapshot().config.ai.autoIndexAfterBackup
  return (
    <div className="flex items-center justify-between gap-3 border-t pt-3 text-[13px]">
      <div className="flex flex-col gap-0.5">
        <Label htmlFor="settings-semantic-auto" className="font-normal">
          {t('settingsAi.semantic.autoUpdate')}
        </Label>
        <span className="text-xs text-muted-foreground">
          {t('settingsAi.semantic.autoUpdateNote')}
        </span>
      </div>
      <Switch
        id="settings-semantic-auto"
        checked={on}
        disabled={index.saving}
        onCheckedChange={(next) =>
          void index.setAutoUpdate(next).catch((reason: unknown) =>
            toast.error(
              t('settingsAi.semantic.failed', {
                message: describeError(reason),
              }),
            ),
          )
        }
      />
    </div>
  )
}

/** The chosen model, by its short name. */
function chosenName(index: SemanticIndex) {
  const chosen = index.semantic?.chosen
  if (chosen) return shortModel(chosen.model)
  return index.service?.defaultModel ?? ''
}

/** `minishlab/potion-multilingual-128M` → `potion-multilingual-128M`. */
function shortModel(model: string) {
  return model.split('/').at(-1) ?? model
}

/** Speed and time left, once there is enough to measure them. */
function pace(
  t: ReturnType<typeof useI18n>['t'],
  format: ReturnType<typeof useFormat>,
  progress: NonNullable<SemanticIndex['progress']>,
) {
  if (progress.perSecond === null || progress.secondsLeft === null) return null
  const speed = t('settingsAi.semantic.speed', {
    rate: format.number(Math.round(progress.perSecond)),
  })
  const minutes = Math.round(progress.secondsLeft / 60)
  const left =
    progress.secondsLeft < 60
      ? t('settingsAi.semantic.etaSoon')
      : minutes < 90
        ? t('settingsAi.semantic.etaMinutes', { count: minutes })
        : t('settingsAi.semantic.etaHours', {
            hours: format.number(Math.floor(minutes / 60)),
            minutes: format.number(minutes % 60),
          })
  return `${speed} · ${left}`
}

export function ProgressLine({
  label,
  detail,
  value,
  action,
}: {
  label: string
  detail?: string
  /** 0–1; leave out while the total is not known yet. */
  value?: number
  action?: React.ReactNode
}) {
  return (
    <div role="status" className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3 text-[13px]">
        <span>{label}</span>
        <span className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
          {detail}
          {action}
        </span>
      </div>
      <Progress
        value={value === undefined ? undefined : Math.round(value * 100)}
        aria-label={label}
      />
    </div>
  )
}
