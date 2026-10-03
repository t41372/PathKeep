/**
 * Free up space: lists what can be deleted (`preview_retention_prune`), lets
 * the user tick what goes, then deletes it (`run_retention_prune`). Your
 * history itself is never in the list.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { archiveClient } from '@/lib/backend-client/archive'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import type { RetentionBucket } from '@/lib/types'
import { Notice } from './notice'

const bucketIds = ['snapshots', 'exports', 'staging', 'quarantine'] as const
type BucketId = (typeof bucketIds)[number]

const isKnown = (id: string): id is BucketId =>
  (bucketIds as readonly string[]).includes(id)

export function FreeSpaceDialog({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <FreeSpaceForm onClose={onClose} />}
    </Dialog>
  )
}

function FreeSpaceForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const format = useFormat()
  const client = useQueryClient()
  const preview = useQuery({
    queryKey: ['retention-preview'],
    queryFn: archiveClient.previewRetentionPrune,
    staleTime: 0,
    gcTime: 0,
  })
  // Snapshots are restore points, so they are opt-in; leftovers are opt-out.
  const [picked, setPicked] = useState<Set<string> | null>(null)
  const buckets = (preview.data?.buckets ?? []).filter(
    (bucket) => bucket.bytes > 0 && isKnown(bucket.id),
  )
  const selected =
    picked ??
    new Set(buckets.filter((b) => b.id !== 'snapshots').map((b) => b.id))
  const selectedBytes = buckets
    .filter((bucket) => selected.has(bucket.id))
    .reduce((sum, bucket) => sum + bucket.bytes, 0)

  const prune = useMutation({
    mutationFn: () =>
      archiveClient.runRetentionPrune({ bucketIds: [...selected] }),
    onSuccess: (result) => {
      toast.success(
        t('settingsStorage.freeSpace.done', {
          size: format.bytes(result.deletedBytes),
        }),
      )
      void client.invalidateQueries({ queryKey: queryKeys.dashboard })
      onClose()
    },
  })

  const toggle = (bucket: RetentionBucket, on: boolean) => {
    const next = new Set(selected)
    if (on) next.add(bucket.id)
    else next.delete(bucket.id)
    setPicked(next)
  }

  const showSnapshotWarning = selected.has('snapshots')

  return (
    <DialogContent className="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{t('settingsStorage.freeSpace.title')}</DialogTitle>
        <DialogDescription>
          {t('settingsStorage.freeSpace.intro')}
        </DialogDescription>
      </DialogHeader>

      {preview.isPending ? (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-11 w-full" />
          <Skeleton className="h-11 w-full" />
        </div>
      ) : preview.isError ? (
        <p role="alert" className="text-[13px] text-destructive">
          {t('settingsStorage.freeSpace.loadFailed', {
            message: describeError(preview.error, 'preview_retention_prune'),
          })}
        </p>
      ) : buckets.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">
          {t('settingsStorage.freeSpace.nothing')}
        </p>
      ) : (
        <ul className="flex flex-col gap-1">
          {buckets.map((bucket) => {
            const id = bucket.id as BucketId
            return (
              <li
                key={bucket.id}
                className="flex items-start gap-3 rounded-lg px-1 py-2"
              >
                <Checkbox
                  id={`prune-${id}`}
                  className="mt-0.5"
                  checked={selected.has(id)}
                  disabled={prune.isPending}
                  onCheckedChange={(value) => toggle(bucket, value === true)}
                />
                <Label
                  htmlFor={`prune-${id}`}
                  className="flex min-w-0 flex-1 flex-col items-start gap-0.5 font-normal"
                >
                  <span className="flex w-full justify-between gap-3 font-medium">
                    {t(`settingsStorage.freeSpace.bucket.${id}.title`)}
                    <span className="font-mono text-[13px] font-normal">
                      {format.bytes(bucket.bytes)}
                    </span>
                  </span>
                  <span className="text-[13px] text-muted-foreground">
                    {t(`settingsStorage.freeSpace.bucket.${id}.description`)}
                  </span>
                </Label>
              </li>
            )
          })}
        </ul>
      )}

      {showSnapshotWarning && (
        <Notice tone="warning">
          {t('settingsStorage.freeSpace.snapshotWarning')}
        </Notice>
      )}

      {prune.error && (
        <p role="alert" className="text-[13px] text-destructive">
          {t('settingsStorage.freeSpace.failed', {
            message: describeError(prune.error, 'run_retention_prune'),
          })}
        </p>
      )}

      <DialogFooter>
        <Button variant="ghost" disabled={prune.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          variant="destructive"
          disabled={prune.isPending || selected.size === 0 || !buckets.length}
          onClick={() => prune.mutate()}
        >
          {prune.isPending && <Spinner />}
          {t('settingsStorage.freeSpace.delete', {
            size: format.bytes(selectedBytes),
          })}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
