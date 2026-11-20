/**
 * The link preview images PathKeep keeps: how many, how much space, and
 * deleting them all after a confirmation. Browsing history's own data, so
 * the row sits in Settings › Browsing history with the Link previews switch
 * (`data-spaces.md` §8).
 *
 * Not responsible for: fetching, or the "Keep images" rule and "Clean up
 * now" (link previews' options and the background tasks page).
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
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
import { explorerClient } from '@/lib/backend-client/explorer'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'

const ogStatsKey = ['og-image-storage'] as const

export function PreviewImagesRow() {
  const { t } = useI18n()
  const format = useFormat()
  const client = useQueryClient()
  const [confirm, setConfirm] = useState(false)
  const stats = useQuery({
    queryKey: ogStatsKey,
    queryFn: explorerClient.getOgImageStorageStats,
  })
  const clear = useMutation({
    mutationFn: explorerClient.clearOgImageCache,
    onSuccess: (report) => {
      toast.success(
        t('settingsBrowsing.images.cleared', {
          size: format.bytes(report.reclaimedBytes),
        }),
      )
      void client.invalidateQueries({ queryKey: ogStatsKey })
    },
    onError: (error) =>
      toast.error(t('settingsBrowsing.images.clearFailed'), {
        description: describeError(error, 'clear_og_image_cache'),
      }),
  })

  const data = stats.data
  return (
    <SettingRow
      title={t('settingsBrowsing.images.title')}
      description={
        stats.isPending ? (
          <Skeleton className="mt-0.5 h-3.5 w-40" />
        ) : stats.isError ? (
          t('settingsBrowsing.images.statsFailed')
        ) : (
          t('settingsBrowsing.images.description', {
            count: data!.blobCount,
            size: format.bytes(data!.totalBytes),
          })
        )
      }
      control={
        <Button
          size="sm"
          variant="outline"
          disabled={!data || data.totalBytes === 0 || clear.isPending}
          onClick={() => setConfirm(true)}
        >
          {clear.isPending && <Spinner />}
          {t('settingsBrowsing.images.action')}
        </Button>
      }
    >
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('settingsBrowsing.images.confirmTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('settingsBrowsing.images.confirmBody', {
                count: data?.blobCount ?? 0,
                size: format.bytes(data?.totalBytes ?? 0),
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => clear.mutate()}>
              {t('settingsBrowsing.images.action')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}
