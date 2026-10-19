/**
 * The two kinds of background work that contact websites, as Background work
 * shows them: page summaries (content fetch: status, "fetch now") and link
 * previews (coverage, when to fetch, how long to keep images, clean up now).
 *
 * The on/off switches stay in Settings → General (the one place that says
 * which features go online); these rows link there when a feature is off.
 * Not responsible for the fetching itself.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
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
import { contentEnrichmentClient } from '@/lib/backend-client/content-enrichment'
import { explorerClient } from '@/lib/backend-client/explorer'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { OgImageSettingsConfig } from '@/lib/types'
import { RowSelect } from '../row-select'
import { useSaveSetting } from '../use-save-setting'

/** Shared with the General → Page summaries switch. */
const contentFetchKey = ['content-fetch-settings'] as const
const coverageKey = ['og-image-coverage'] as const
const storageKey = ['og-image-storage'] as const

/** How many working-set pages one "Fetch now" queues. */
const FETCH_NOW_LIMIT = 500
const POLL_MS = 2000
const MB = 1024 * 1024

export function PageSummariesRow() {
  const { t } = useI18n()
  const paused = useSnapshot().config.ai.jobQueuePaused
  const client = useQueryClient()
  const status = useQuery({
    queryKey: contentFetchKey,
    queryFn: contentEnrichmentClient.getContentFetchSettings,
    staleTime: 0,
    refetchInterval: (query) => {
      const data = query.state.data
      if (!data?.enabled) return false
      return data.runningJobs > 0 || (!paused && data.queuedJobs > 0)
        ? POLL_MS
        : false
    },
  })
  const fetchNow = useMutation({
    mutationFn: () =>
      contentEnrichmentClient.enqueueContentFetchWorkingSet(FETCH_NOW_LIMIT),
    onSuccess: (count) => {
      if (count > 0)
        toast.success(t('settingsBackground.summaries.queued', { count }))
      else toast.info(t('settingsBackground.summaries.nothingToFetch'))
      void client.invalidateQueries({ queryKey: contentFetchKey })
    },
    onError: (error) =>
      toast.error(t('settingsBackground.summaries.fetchFailed'), {
        description: describeError(error, 'enqueue_content_fetch_working_set'),
      }),
  })

  const data = status.data
  const counts = data
    ? [
        data.runningJobs > 0 &&
          t('settingsBackground.counts.running', { count: data.runningJobs }),
        data.queuedJobs > 0 &&
          t('settingsBackground.counts.queued', { count: data.queuedJobs }),
        data.failedJobs > 0 &&
          t('settingsBackground.counts.failed', { count: data.failedJobs }),
      ].filter(Boolean)
    : []

  return (
    <SettingRow
      title={t('settingsBackground.summaries.title')}
      description={
        status.isPending ? (
          <Skeleton className="mt-0.5 h-3.5 w-40" />
        ) : status.isError ? (
          <span className="text-destructive">
            {t('settingsBackground.summaries.loadFailed')}
          </span>
        ) : !data!.enabled ? (
          t('settingsBackground.summaries.off')
        ) : (
          <span className="flex flex-col" aria-live="polite">
            <span>
              {t('settingsBackground.summaries.stored', {
                count: data!.storedRecords,
              })}
              {counts.length > 0 && ` ${counts.join(' · ')}`}
            </span>
            <span>{t('settingsBackground.summaries.fetchHint')}</span>
          </span>
        )
      }
      control={
        data && !data.enabled ? (
          <Button size="sm" variant="outline" asChild>
            <Link to="/settings/general">
              {t('settingsBackground.summaries.openGeneral')}
            </Link>
          </Button>
        ) : (
          <Button
            size="sm"
            variant="outline"
            disabled={!data || fetchNow.isPending}
            onClick={() => fetchNow.mutate()}
          >
            {fetchNow.isPending && <Spinner />}
            {t('settingsBackground.summaries.fetch')}
          </Button>
        )
      }
    />
  )
}

type KeepChoice = 'off' | 'days30' | 'days90' | 'size100' | 'size500' | 'custom'

const keepRules: Record<
  Exclude<KeepChoice, 'custom'>,
  OgImageSettingsConfig['cleanup']
> = {
  off: { mode: 'off' },
  days30: { mode: 'timeTtl', maxAgeDays: 30 },
  days90: { mode: 'timeTtl', maxAgeDays: 90 },
  size100: { mode: 'lru', maxBytes: 100 * MB },
  size500: { mode: 'lru', maxBytes: 500 * MB },
}

function keepChoiceOf(cleanup: OgImageSettingsConfig['cleanup']): KeepChoice {
  const match = (Object.keys(keepRules) as (keyof typeof keepRules)[]).find(
    (key) => JSON.stringify(keepRules[key]) === JSON.stringify(cleanup),
  )
  return match ?? 'custom'
}

export function LinkPreviewsRow() {
  const { t } = useI18n()
  const format = useFormat()
  const client = useQueryClient()
  const og = useSnapshot().config.ogImage
  const { save, saving } = useSaveSetting()
  const [confirm, setConfirm] = useState(false)
  const on = Boolean(og?.fetchEnabled && og.fetchMode !== 'off')
  // Counting is a full pass over the archive's pages: read once per visit,
  // never polled.
  const coverage = useQuery({
    queryKey: coverageKey,
    queryFn: explorerClient.getOgImageCoverageStats,
    enabled: on,
  })
  const clean = useMutation({
    mutationFn: explorerClient.runOgImageCleanup,
    onSuccess: (report) => {
      toast.success(
        t('settingsBackground.previews.cleaned', {
          count: report.deletedBlobs,
          size: format.bytes(report.reclaimedBytes),
        }),
      )
      void client.invalidateQueries({ queryKey: storageKey })
      void client.invalidateQueries({ queryKey: coverageKey })
    },
    onError: (error) =>
      toast.error(t('settingsBackground.previews.cleanFailed'), {
        description: describeError(error, 'run_og_image_cleanup'),
      }),
  })

  if (!og) return null
  const keep = keepChoiceOf(og.cleanup)

  const setMode = (mode: string) =>
    void save((config) => {
      if (config.ogImage)
        config.ogImage.fetchMode = mode as 'on_demand' | 'background'
      return config
    })
  const setKeep = (choice: string) =>
    void save((config) => {
      if (config.ogImage && choice in keepRules)
        config.ogImage.cleanup = keepRules[choice as keyof typeof keepRules]
      return config
    })

  const stats = coverage.data
  return (
    <SettingRow
      title={t('settingsBackground.previews.title')}
      description={
        !on
          ? t('settingsBackground.previews.off')
          : coverage.isPending
            ? t('settingsBackground.previews.coverageLoading')
            : coverage.isError
              ? t('settingsBackground.previews.coverageFailed')
              : t('settingsBackground.previews.coverage', {
                  withImage: format.number(stats!.pagesWithImage),
                  eligible: format.number(stats!.eligiblePages),
                  checked: format.number(stats!.attemptedPages),
                })
      }
      control={
        !on && (
          <Button size="sm" variant="outline" asChild>
            <Link to="/settings/general">
              {t('settingsBackground.summaries.openGeneral')}
            </Link>
          </Button>
        )
      }
    >
      {on && (
        <div className="flex flex-col gap-2.5 border-t pt-3 text-[13px]">
          <div className="flex items-center justify-between gap-4">
            <label htmlFor="settings-previews-mode">
              {t('settingsBackground.previews.mode.label')}
            </label>
            <RowSelect
              id="settings-previews-mode"
              value={og.fetchMode}
              disabled={saving}
              onChange={setMode}
              options={(['on_demand', 'background'] as const).map((mode) => ({
                value: mode,
                label: t(`settingsBackground.previews.mode.${mode}`),
              }))}
            />
          </div>
          <div className="flex items-center justify-between gap-4">
            <label htmlFor="settings-previews-keep">
              {t('settingsBackground.previews.keep.label')}
            </label>
            <RowSelect
              id="settings-previews-keep"
              value={keep}
              disabled={saving}
              onChange={setKeep}
              options={[
                ...(keep === 'custom' ? (['custom'] as const) : []),
                ...(Object.keys(keepRules) as (keyof typeof keepRules)[]),
              ].map((choice) => ({
                value: choice,
                label: t(`settingsBackground.previews.keep.${choice}`),
              }))}
            />
          </div>
          <div className="flex items-center justify-between gap-4">
            <span className="text-muted-foreground">
              {t('settingsBackground.previews.cleanHint')}
            </span>
            <Button
              size="sm"
              variant="outline"
              disabled={clean.isPending}
              onClick={() => setConfirm(true)}
            >
              {clean.isPending && <Spinner />}
              {t('settingsBackground.previews.clean')}
            </Button>
          </div>
        </div>
      )}
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('settingsBackground.previews.cleanTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('settingsBackground.previews.cleanBody', {
                rule: t(`settingsBackground.previews.keep.${keep}`),
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => clean.mutate()}>
              {t('settingsBackground.previews.clean')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}
