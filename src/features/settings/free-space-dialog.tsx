/**
 * Free up space: lists what can be deleted (`preview_retention_prune`), lets
 * the user tick what goes, then deletes exactly that (`run_retention_prune`).
 * Each safety copy is a row of its own, so the copy a restore or an import
 * just made (the replaced archive, as big as the archive) can go while older
 * ones stay. Your history itself is never in the list. A conversation file
 * PathKeep kept when it disappeared says what it is, and ticking it warns
 * that it may hold the only saved version of some conversations.
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
import type { RetentionCopy } from '@/lib/types'
import { Notice } from './notice'
import { copyLabelKey, HELD_COPY } from './copy-label'

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
  const buckets = (preview.data?.buckets ?? []).filter(
    (bucket) => bucket.bytes > 0 && isKnown(bucket.id),
  )
  const copies = preview.data?.copies ?? []
  // Leftovers are opt-out. Snapshots and safety copies are restore points, so
  // they are opt-in.
  const [pickedBuckets, setPickedBuckets] = useState<Set<string> | null>(null)
  const [pickedCopies, setPickedCopies] = useState<Set<string>>(new Set())
  const selectedBuckets =
    pickedBuckets ??
    new Set(buckets.filter((b) => b.id !== 'snapshots').map((b) => b.id))
  const selectedBytes =
    buckets
      .filter((bucket) => selectedBuckets.has(bucket.id))
      .reduce((sum, bucket) => sum + bucket.bytes, 0) +
    copies
      .filter((copy) => pickedCopies.has(copy.path))
      .reduce((sum, copy) => sum + copy.bytes, 0)
  const restorable = copies.filter((copy) => copy.restorable)
  const heldPicked = copies.some(
    (copy) => copy.sourceOp === HELD_COPY && pickedCopies.has(copy.path),
  )
  const noCopyLeft =
    restorable.length > 0 &&
    restorable.every((copy) => pickedCopies.has(copy.path))
  const nothingPicked = selectedBuckets.size === 0 && pickedCopies.size === 0

  const prune = useMutation({
    mutationFn: () =>
      archiveClient.runRetentionPrune({
        bucketIds: [...selectedBuckets],
        copies: [...pickedCopies],
      }),
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

  const toggleBucket = (id: string, on: boolean) => {
    const next = new Set(selectedBuckets)
    if (on) next.add(id)
    else next.delete(id)
    setPickedBuckets(next)
  }
  const toggleCopy = (path: string, on: boolean) => {
    const next = new Set(pickedCopies)
    if (on) next.add(path)
    else next.delete(path)
    setPickedCopies(next)
  }

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
        <p
          role="alert"
          className="text-[13px] [overflow-wrap:anywhere] text-destructive"
        >
          {t('settingsStorage.freeSpace.loadFailed', {
            message: describeError(preview.error, 'preview_retention_prune'),
          })}
        </p>
      ) : buckets.length === 0 && copies.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">
          {t('settingsStorage.freeSpace.nothing')}
        </p>
      ) : (
        <div className="flex max-h-[min(60vh,28rem)] flex-col gap-3 overflow-y-auto">
          {buckets.length > 0 && (
            <ul className="flex flex-col gap-1">
              {buckets.map((bucket) => {
                const id = bucket.id as BucketId
                return (
                  <Row
                    key={bucket.id}
                    id={`prune-${id}`}
                    checked={selectedBuckets.has(id)}
                    disabled={prune.isPending}
                    onChange={(on) => toggleBucket(id, on)}
                    title={t(`settingsStorage.freeSpace.bucket.${id}.title`)}
                    size={format.bytes(bucket.bytes)}
                    description={t(
                      `settingsStorage.freeSpace.bucket.${id}.description`,
                    )}
                  />
                )
              })}
            </ul>
          )}
          {copies.length > 0 && (
            <section
              aria-labelledby="prune-copies-heading"
              className="flex flex-col gap-1"
            >
              <h3
                id="prune-copies-heading"
                className="px-1 text-[13px] font-medium text-muted-foreground"
              >
                {t('settingsStorage.freeSpace.copies')}
              </h3>
              <ul className="flex flex-col gap-1">
                {copies.map((copy, index) => (
                  <Row
                    key={copy.path}
                    id={`prune-copy-${index}`}
                    checked={pickedCopies.has(copy.path)}
                    disabled={prune.isPending}
                    onChange={(on) => toggleCopy(copy.path, on)}
                    title={t(
                      `settingsStorage.restore.op.${copyLabelKey(copy.sourceOp)}`,
                    )}
                    size={format.bytes(copy.bytes)}
                    description={copyDescription(copy, t, format)}
                  />
                ))}
              </ul>
            </section>
          )}
        </div>
      )}

      {pickedCopies.size > 0 && (
        <Notice tone="warning">
          {t(
            noCopyLeft
              ? 'settingsStorage.freeSpace.noCopyLeft'
              : 'settingsStorage.freeSpace.copyWarning',
          )}
        </Notice>
      )}
      {heldPicked && (
        <Notice tone="warning">
          {t('settingsStorage.freeSpace.heldWarning')}
        </Notice>
      )}

      {prune.error && (
        <p
          role="alert"
          className="text-[13px] [overflow-wrap:anywhere] text-destructive"
        >
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
          disabled={prune.isPending || nothingPicked || preview.isPending}
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

function copyDescription(
  copy: RetentionCopy,
  t: ReturnType<typeof useI18n>['t'],
  format: ReturnType<typeof useFormat>,
) {
  const made = copy.createdAt
    ? t('settingsStorage.freeSpace.copyMade', {
        date: format.date(copy.createdAt, {
          dateStyle: 'medium',
          timeStyle: 'short',
        }),
      })
    : null
  // Kept when a conversation file disappeared: it was never meant to restore.
  if (copy.sourceOp === HELD_COPY)
    return [made, t('settingsStorage.freeSpace.conversationsOnly')]
      .filter(Boolean)
      .join(' · ')
  const state = !copy.restorable
    ? t('settingsStorage.freeSpace.notRestorable')
    : copy.unchecked
      ? t('settingsStorage.restore.unchecked')
      : null
  const conversations = copy.conversationsUnreadable
    ? t('settingsStorage.restore.conversationsUnreadable')
    : null
  return [made, state, conversations].filter(Boolean).join(' · ')
}

function Row({
  id,
  checked,
  disabled,
  onChange,
  title,
  size,
  description,
}: {
  id: string
  checked: boolean
  disabled: boolean
  onChange: (on: boolean) => void
  title: string
  size: string
  description: string
}) {
  return (
    <li className="flex items-start gap-3 rounded-lg px-1 py-2">
      <Checkbox
        id={id}
        className="mt-0.5"
        checked={checked}
        disabled={disabled}
        onCheckedChange={(value) => onChange(value === true)}
      />
      <Label
        htmlFor={id}
        className="flex min-w-0 flex-1 flex-col items-start gap-0.5 font-normal"
      >
        <span className="flex w-full justify-between gap-3 font-medium">
          {title}
          <span className="font-mono text-[13px] font-normal">{size}</span>
        </span>
        {description && (
          <span className="text-[13px] text-muted-foreground">
            {description}
          </span>
        )}
      </Label>
    </li>
  )
}
