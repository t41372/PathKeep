/**
 * The two features that contact websites: link preview images and page
 * summaries. Everything else in PathKeep stays on this computer, so these
 * switches are where the user decides whether any site hears from it. Both
 * belong to browsing history, so they sit in Settings › Browsing history
 * (`data-spaces.md` §8), with link previews' two options under its switch.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { AnimatePresence, motion, useReducedMotion } from 'motion/react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import { contentEnrichmentClient } from '@/lib/backend-client/content-enrichment'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { useSnapshot } from '@/lib/queries/app'
import { useSaveSetting } from '../use-save-setting'
import { LinkPreviewOptions } from './link-preview-options'

export function OnlineRows() {
  return (
    <>
      <LinkPreviewsRow />
      <PageSummariesRow />
    </>
  )
}

function LinkPreviewsRow() {
  const { t } = useI18n()
  const snapshot = useSnapshot()
  const { save, saving } = useSaveSetting()
  const still = !!useReducedMotion()
  const og = snapshot.config.ogImage
  const on = Boolean(og?.fetchEnabled && og.fetchMode !== 'off')

  const toggle = (next: boolean) =>
    void save((config) => {
      if (!config.ogImage) return config
      config.ogImage.fetchEnabled = next
      if (next && config.ogImage.fetchMode === 'off') {
        config.ogImage.fetchMode = 'background'
      }
      return config
    })

  return (
    <SettingRow
      title={t('settingsBrowsing.previews.title')}
      description={t('settingsBrowsing.previews.description')}
      htmlFor="settings-link-previews"
      control={
        <Switch
          id="settings-link-previews"
          checked={on}
          disabled={!og || saving}
          onCheckedChange={toggle}
        />
      }
    >
      <AnimatePresence initial={false}>
        {on && (
          // The row's gap is taken back while the options open and close, so
          // they slide in from nothing instead of jumping by the gap.
          <motion.div
            key="options"
            initial={still ? { opacity: 0 } : { height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={still ? { opacity: 0 } : { height: 0, opacity: 0 }}
            transition={{ duration: 0.18, ease: 'easeOut' }}
            className="-mt-3 overflow-hidden"
          >
            <div
              role="group"
              aria-label={t('settingsBrowsing.previews.options')}
              className="mt-3 flex flex-col gap-2.5 border-t pt-3 text-[13px]"
            >
              <LinkPreviewOptions />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </SettingRow>
  )
}

const contentFetchKey = ['content-fetch-settings'] as const

function PageSummariesRow() {
  const { t } = useI18n()
  const client = useQueryClient()
  const query = useQuery({
    queryKey: contentFetchKey,
    queryFn: contentEnrichmentClient.getContentFetchSettings,
  })
  const change = useMutation({
    mutationFn: (enabled: boolean) =>
      contentEnrichmentClient.setContentFetchSettings({
        ...query.data!,
        enabled,
      }),
    onSuccess: (snapshot) => {
      client.setQueryData(queryKeys.snapshot, snapshot)
      void client.invalidateQueries({ queryKey: contentFetchKey })
    },
    onError: (error) =>
      toast.error(t('settings.saveFailed'), {
        description: describeError(error, 'set_content_fetch_settings'),
      }),
  })

  const pending = change.isPending ? change.variables : undefined
  return (
    <SettingRow
      title={t('settingsBrowsing.summaries.title')}
      description={
        query.isError ? (
          <span className="text-destructive">
            {t('settingsBrowsing.summaries.loadFailed')}
          </span>
        ) : (
          t('settingsBrowsing.summaries.description')
        )
      }
      htmlFor="settings-page-summaries"
      control={
        query.isPending ? (
          <Skeleton className="h-[1.15rem] w-8 rounded-full" />
        ) : (
          <Switch
            id="settings-page-summaries"
            checked={pending ?? query.data?.enabled ?? false}
            disabled={!query.data || change.isPending}
            onCheckedChange={(next) => change.mutate(next)}
          />
        )
      }
    />
  )
}
