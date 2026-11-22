/**
 * Free up space: lists what can be deleted (`preview_retention_prune`), lets
 * the user tick what goes, then deletes exactly that (`run_retention_prune`).
 * Each safety copy is a row of its own, so the copy a restore or an import
 * just made (the replaced archive, as big as the archive) can go while older
 * ones stay. Your history itself is never in the list. A conversation file
 * PathKeep kept when it disappeared says what it is, and ticking it warns
 * that it may hold the only saved version of some conversations. Folders
 * kept aside that nothing restores from (an undone import's conversations,
 * what an undone "Delete all data" left, an archive an earlier version set
 * aside) are listed the same way, each with the way to it in the file
 * manager, so they can be looked at before they go. Browsing history's
 * semantic search index is a row too, opt-in like the copies: freed with the
 * preview read when the dialog opened, and only while semantic search is
 * off (`semantic-index-free.tsx`).
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { FolderOpen } from 'lucide-react'
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
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { queryKeys } from '@/lib/query'
import { useSnapshot } from '@/lib/queries/app'
import type { RetentionCopy } from '@/lib/types'
import { Notice } from './notice'
import {
  copyLabelKey,
  HELD_COPY,
  IMPORT_UNDO_COPY,
  SET_ASIDE_FOLDERS,
} from './copy-label'
import { reveal, revealLabelKey } from './reveal'
import { INDEX_REFUSED, refreshAfterIndexFreed } from './semantic-index-free'

const bucketIds = ['snapshots', 'exports', 'staging', 'quarantine'] as const
type BucketId = (typeof bucketIds)[number]

const isKnown = (id: string): id is BucketId =>
  (bucketIds as readonly string[]).includes(id)

/** The index wasn't freed, so nothing was deleted: the prune runs after it. */
class IndexNotFreed extends Error {
  readonly reason: unknown
  constructor(reason: unknown) {
    super('the semantic search index was not freed')
    this.name = 'IndexNotFreed'
    this.reason = reason
  }
}

/** The index was freed (`freed` bytes), then the prune failed. */
class RestNotDeleted extends Error {
  readonly reason: unknown
  readonly freed: number
  constructor(reason: unknown, freed: number) {
    super('the semantic search index was freed; the rest was not deleted')
    this.name = 'RestNotDeleted'
    this.reason = reason
    this.freed = freed
  }
}

/**
 * Browsing history's semantic search index as a row. While semantic search
 * is on, the backend refuses to free it, so its size comes from the AI status
 * and the row only says to turn it off. While it's off, the size and the
 * receipt the free uses come from a preview read when the dialog opens.
 */
function useIndexRow() {
  const snapshot = useSnapshot()
  const ai = snapshot.config.ai
  const on = ai.enabled && ai.semanticIndexEnabled
  const statusBytes = snapshot.aiStatus.semanticSidecarBytes
  const preview = useQuery({
    queryKey: ['archive', 'semantic-index', 'free-space-preview'],
    queryFn: intelligenceClient.previewFreeSemanticIndex,
    enabled: !on && statusBytes > 0,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  })
  const previewBytes = preview.data ? Number(preview.data.keptBytes) : null
  // Once a free went through, the status reads 0 and the last preview is spent.
  const bytes =
    on || statusBytes === 0 ? statusBytes : (previewBytes ?? statusBytes)
  const receipt =
    !on && !preview.isFetching && previewBytes !== null && previewBytes > 0
      ? preview.data
      : null
  return {
    on,
    bytes,
    shown: bytes > 0,
    receipt,
    reading: preview.isFetching,
    error: preview.error,
    readAgain: () => void preview.refetch(),
  }
}

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
  // The index takes long to build again, so it's opt-in like the copies.
  const kept = useIndexRow()
  const [pickedIndex, setPickedIndex] = useState(false)
  const freeIndex = pickedIndex && kept.receipt !== null
  const selectedBuckets =
    pickedBuckets ??
    new Set(buckets.filter((b) => b.id !== 'snapshots').map((b) => b.id))
  const selectedBytes =
    buckets
      .filter((bucket) => selectedBuckets.has(bucket.id))
      .reduce((sum, bucket) => sum + bucket.bytes, 0) +
    copies
      .filter((copy) => pickedCopies.has(copy.path))
      .reduce((sum, copy) => sum + copy.bytes, 0) +
    (freeIndex ? kept.bytes : 0)
  const restorable = copies.filter((copy) => copy.restorable)
  const heldPicked = copies.some(
    (copy) => copy.sourceOp === HELD_COPY && pickedCopies.has(copy.path),
  )
  const noCopyLeft =
    restorable.length > 0 &&
    restorable.every((copy) => pickedCopies.has(copy.path))
  const prunes = selectedBuckets.size > 0 || pickedCopies.size > 0
  const nothingPicked = !prunes && !freeIndex

  const run = useMutation({
    mutationFn: async () => {
      // The index first. Its receipt holds only while the archive is as it
      // was, and a prune records its run in the archive, so a free after it
      // would always be refused as changed. Refused, it leaves everything.
      let freed: number | null = null
      if (freeIndex && kept.receipt) {
        try {
          const result = await intelligenceClient.freeSemanticIndex(
            kept.receipt.previewId,
          )
          freed = Number(result.freedBytes)
        } catch (reason) {
          throw new IndexNotFreed(reason)
        }
      }
      const indexFreed = freed !== null
      if (!prunes) return { deleted: freed ?? 0, indexFreed }
      try {
        const result = await archiveClient.runRetentionPrune({
          bucketIds: [...selectedBuckets],
          copies: [...pickedCopies],
        })
        return { deleted: (freed ?? 0) + result.deletedBytes, indexFreed }
      } catch (reason) {
        if (freed !== null) throw new RestNotDeleted(reason, freed)
        throw reason
      }
    },
    onSuccess: ({ deleted, indexFreed }) => {
      toast.success(
        t('settingsStorage.freeSpace.done', { size: format.bytes(deleted) }),
      )
      if (indexFreed) void refreshAfterIndexFreed().catch(() => undefined)
      else void client.invalidateQueries({ queryKey: queryKeys.dashboard })
      onClose()
    },
    onError: (error) => {
      // Changed since its size was read: read it again and let the person
      // look at the new size before deleting again.
      if (
        error instanceof IndexNotFreed &&
        commandErrorCode(error.reason) === 'preview-expired'
      )
        kept.readAgain()
      // Gone, while the rest stays listed to try again.
      if (error instanceof RestNotDeleted) {
        setPickedIndex(false)
        void refreshAfterIndexFreed().catch(() => undefined)
      }
    },
  })
  const failure = run.error ? describeFailure(run.error, t, format) : null
  const indexNote = kept.on
    ? t('settingsStorage.freeSpace.index.searchOn')
    : kept.error
      ? refusalWords(kept.error, t)
      : t('settingsStorage.freeSpace.index.description')

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
      ) : buckets.length === 0 && copies.length === 0 && !kept.shown ? (
        <p className="text-[13px] text-muted-foreground">
          {t('settingsStorage.freeSpace.nothing')}
        </p>
      ) : (
        <div className="flex max-h-[min(60vh,28rem)] flex-col gap-3 overflow-y-auto">
          {(buckets.length > 0 || kept.shown) && (
            <ul className="flex flex-col gap-1">
              {buckets.map((bucket) => {
                const id = bucket.id as BucketId
                return (
                  <Row
                    key={bucket.id}
                    id={`prune-${id}`}
                    checked={selectedBuckets.has(id)}
                    disabled={run.isPending}
                    onChange={(on) => toggleBucket(id, on)}
                    title={t(`settingsStorage.freeSpace.bucket.${id}.title`)}
                    size={format.bytes(bucket.bytes)}
                    description={t(
                      `settingsStorage.freeSpace.bucket.${id}.description`,
                    )}
                  />
                )
              })}
              {kept.shown && (
                <Row
                  id="prune-semantic-index"
                  checked={freeIndex}
                  disabled={run.isPending || kept.receipt === null}
                  onChange={setPickedIndex}
                  title={t('settingsStorage.freeSpace.index.title')}
                  size={format.bytes(kept.bytes)}
                  description={indexNote}
                />
              )}
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
                    disabled={run.isPending}
                    onChange={(on) => toggleCopy(copy.path, on)}
                    title={t(
                      `settingsStorage.restore.op.${copyLabelKey(copy.sourceOp)}`,
                    )}
                    size={format.bytes(copy.bytes)}
                    description={copyDescription(copy, t, format)}
                    revealPath={setAside(copy) ? copy.path : undefined}
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

      {failure && (
        <p
          role="alert"
          className="text-[13px] [overflow-wrap:anywhere] text-destructive"
        >
          {failure}
        </p>
      )}

      <DialogFooter>
        <Button variant="ghost" disabled={run.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          variant="destructive"
          disabled={
            run.isPending ||
            nothingPicked ||
            preview.isPending ||
            // The index's size is being read again: delete once it's shown.
            (pickedIndex && kept.reading)
          }
          onClick={() => run.mutate()}
        >
          {run.isPending && <Spinner />}
          {t('settingsStorage.freeSpace.delete', {
            size: format.bytes(selectedBytes),
          })}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}

type Translate = ReturnType<typeof useI18n>['t']

/** Why the index can't be read or freed, in the words Settings › AI uses. */
function indexRefusal(
  error: unknown,
  t: Translate,
  otherwise: (message: string) => string,
) {
  const code = commandErrorCode(error)
  const known = code ? INDEX_REFUSED[code] : undefined
  return known
    ? t(known)
    : otherwise(describeError(error, 'free_semantic_index'))
}

function refusalWords(error: unknown, t: Translate) {
  return indexRefusal(error, t, (message) =>
    t('settingsAi.semantic.kept.readFailed', { message }),
  )
}

/** Which part failed, and so what was deleted: nothing, or only the index. */
function describeFailure(
  error: Error,
  t: Translate,
  format: ReturnType<typeof useFormat>,
) {
  if (error instanceof IndexNotFreed) {
    return commandErrorCode(error.reason) === 'preview-expired'
      ? t('settingsStorage.freeSpace.indexChanged')
      : t('settingsStorage.freeSpace.indexFailed', {
          reason: indexRefusal(error.reason, t, (message) => message),
        })
  }
  if (error instanceof RestNotDeleted) {
    return t('settingsStorage.freeSpace.restFailed', {
      size: format.bytes(error.freed),
      message: describeError(error.reason, 'run_retention_prune'),
    })
  }
  return t('settingsStorage.freeSpace.failed', {
    message: describeError(error, 'run_retention_prune'),
  })
}

/** Kept aside rather than a restore point: shown with the way to it. */
function setAside(copy: RetentionCopy) {
  return (
    copy.sourceOp === IMPORT_UNDO_COPY ||
    SET_ASIDE_FOLDERS.includes(copy.sourceOp)
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
  if (copy.sourceOp === IMPORT_UNDO_COPY)
    return [made, t('settingsStorage.freeSpace.undoneImport')]
      .filter(Boolean)
      .join(' · ')
  if (SET_ASIDE_FOLDERS.includes(copy.sourceOp))
    return [made, t('settingsStorage.freeSpace.setAside')]
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
  revealPath,
}: {
  id: string
  checked: boolean
  disabled: boolean
  onChange: (on: boolean) => void
  title: string
  size: string
  description: string
  /** A folder to show in the file manager before deciding. */
  revealPath?: string
}) {
  const { t } = useI18n()
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
      {revealPath && (
        <Button
          type="button"
          size="icon-xs"
          variant="ghost"
          className="text-muted-foreground"
          aria-label={t(revealLabelKey())}
          title={t(revealLabelKey())}
          onClick={() =>
            void reveal(revealPath, t('settingsStorage.location.failed'))
          }
        >
          <FolderOpen aria-hidden />
        </Button>
      )}
    </li>
  )
}
