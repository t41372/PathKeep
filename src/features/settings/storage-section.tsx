/**
 * Settings → Storage: where the archive lives and how much space it takes,
 * freeing space, exporting, moving to another computer, restoring a safety
 * copy, and deleting everything. Each destructive action opens a dialog that
 * shows what will happen before it runs.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import {
  SettingRow,
  SettingsGroup,
  SettingsSection,
} from '@/components/app/setting-row'
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
import { archiveClient } from '@/lib/backend-client/archive'
import { explorerClient } from '@/lib/backend-client/explorer'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { useSnapshot } from '@/lib/queries/app'
import type { ExportFormat } from '@/lib/types'
import { FreeSpaceDialog } from './free-space-dialog'
import { MoveDialog } from './move-dialogs'
import { RestoreDialog } from './restore-dialog'
import { reveal, revealLabelKey } from './reveal'
import { RowSelect } from './row-select'
import { UsageCard } from './usage-card'
import { WipeDialog } from './wipe-dialog'

type Open = 'free' | 'export' | 'import' | 'restore' | 'wipe' | null

export function StorageSection() {
  const { t } = useI18n()
  const [open, setOpen] = useState<Open>(null)
  const close = () => setOpen(null)

  return (
    <SettingsSection title={t('settings.nav.storage')}>
      <LocationRow />
      <UsageCard />
      <SettingRow
        title={t('settingsStorage.freeSpace.rowTitle')}
        description={t('settingsStorage.freeSpace.rowDescription')}
        control={
          <Button size="sm" variant="outline" onClick={() => setOpen('free')}>
            {t('settingsStorage.freeSpace.action')}
          </Button>
        }
      />
      <PreviewCacheRow />
      <ExportRow />

      <SettingsGroup title={t('settingsStorage.moveGroup')} />
      <SettingRow
        title={t('settingsStorage.move.title')}
        description={t('settingsStorage.move.description')}
        control={
          <>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setOpen('import')}
            >
              {t('settingsStorage.move.importAction')}
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setOpen('export')}
            >
              {t('settingsStorage.move.exportAction')}
            </Button>
          </>
        }
      />
      <SettingRow
        title={t('settingsStorage.restore.rowTitle')}
        description={t('settingsStorage.restore.rowDescription')}
        control={
          <Button
            size="sm"
            variant="outline"
            onClick={() => setOpen('restore')}
          >
            {t('settingsStorage.restore.action')}
          </Button>
        }
      />
      <SettingRow
        danger
        title={t('settingsStorage.wipe.rowTitle')}
        description={t('settingsStorage.wipe.rowDescription')}
        control={
          <Button
            size="sm"
            variant="destructive"
            onClick={() => setOpen('wipe')}
          >
            {t('settingsStorage.wipe.action')}
          </Button>
        }
      />

      <FreeSpaceDialog open={open === 'free'} onClose={close} />
      <MoveDialog
        mode={open === 'export' || open === 'import' ? open : null}
        onClose={close}
      />
      <RestoreDialog open={open === 'restore'} onClose={close} />
      <WipeDialog open={open === 'wipe'} onClose={close} />
    </SettingsSection>
  )
}

function LocationRow() {
  const { t } = useI18n()
  const root = useSnapshot().directories.appRoot
  return (
    <SettingRow
      title={t('settingsStorage.location.title')}
      description={<span className="font-mono text-xs break-all">{root}</span>}
      control={
        <Button
          size="sm"
          variant="outline"
          onClick={() =>
            void reveal(root, t('settingsStorage.location.failed'))
          }
        >
          {t(revealLabelKey())}
        </Button>
      }
    />
  )
}

const ogStatsKey = ['og-image-storage'] as const

function PreviewCacheRow() {
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
        t('settingsStorage.previews.cleared', {
          size: format.bytes(report.reclaimedBytes),
        }),
      )
      void client.invalidateQueries({ queryKey: ogStatsKey })
    },
    onError: (error) =>
      toast.error(t('settingsStorage.previews.clearFailed'), {
        description: describeError(error, 'clear_og_image_cache'),
      }),
  })

  const data = stats.data
  return (
    <SettingRow
      title={t('settingsStorage.previews.title')}
      description={
        stats.isPending ? (
          <Skeleton className="mt-0.5 h-3.5 w-40" />
        ) : stats.isError ? (
          t('settingsStorage.previews.statsFailed')
        ) : (
          t('settingsStorage.previews.description', {
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
          {t('settingsStorage.previews.action')}
        </Button>
      }
    >
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('settingsStorage.previews.confirmTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('settingsStorage.previews.confirmBody', {
                count: data?.blobCount ?? 0,
                size: format.bytes(data?.totalBytes ?? 0),
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => clear.mutate()}>
              {t('settingsStorage.previews.action')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}

const exportFormats: ExportFormat[] = ['html', 'markdown', 'text', 'jsonl']

function ExportRow() {
  const { t } = useI18n()
  const client = useQueryClient()
  const [kind, setKind] = useState<ExportFormat>('html')
  const run = useMutation({
    mutationFn: () => archiveClient.exportHistory({ query: {}, format: kind }),
    onSuccess: (result) => {
      void client.invalidateQueries({ queryKey: queryKeys.dashboard })
      toast.success(t('settingsStorage.export.done', { count: result.count }), {
        description: result.path,
        action: {
          label: t(revealLabelKey()),
          onClick: () =>
            void reveal(result.path, t('settingsStorage.location.failed')),
        },
      })
    },
    onError: (error) =>
      toast.error(t('settingsStorage.export.failed'), {
        description: describeError(error, 'export_history'),
      }),
  })

  return (
    <SettingRow
      title={t('settingsStorage.export.title')}
      description={t('settingsStorage.export.description')}
      control={
        <>
          <RowSelect
            value={kind}
            label={t('settingsStorage.export.format')}
            disabled={run.isPending}
            onChange={(value) => setKind(value as ExportFormat)}
            options={exportFormats.map((value) => ({
              value,
              label: t(`settingsStorage.export.formats.${value}`),
            }))}
          />
          <Button
            size="sm"
            variant="outline"
            disabled={run.isPending}
            onClick={() => run.mutate()}
          >
            {run.isPending && <Spinner />}
            {run.isPending
              ? t('settingsStorage.export.running')
              : t('settingsStorage.export.action')}
          </Button>
        </>
      }
    >
      {run.isPending && (
        <p role="status" className="text-[13px] text-muted-foreground">
          {t('settingsStorage.export.wait')}
        </p>
      )}
    </SettingRow>
  )
}
