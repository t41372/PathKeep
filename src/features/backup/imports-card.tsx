/**
 * Recent imports (Takeout and browser files), each with undo or restore.
 * Both go through a preview dialog (`preview_import_batch`): what the import
 * added, from where, how many visits are shown now, and the audit record;
 * then the action. Undo hides the visits; restore shows them again.
 */
import { useMutation, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { refreshAfterArchiveChange } from '@/app/backup-runner'
import { SectionCard } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Skeleton } from '@/components/ui/skeleton'
import { Spinner } from '@/components/ui/spinner'
import { importClient } from '@/lib/backend-client/import'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { useSnapshot } from '@/lib/queries/app'
import type { ImportBatchDetail, ImportBatchOverview } from '@/lib/types'
import { PathLine } from './path-actions'

const baseName = (path: string) =>
  path.split(/[\\/]/).filter(Boolean).pop() ?? path

function useKindLabel() {
  const { t } = useI18n()
  return (batch: ImportBatchOverview) =>
    batch.sourceKind.includes('takeout')
      ? t('backup.import.batches.takeout')
      : t('backup.import.batches.browserFile')
}

export function ImportsCard() {
  const { t } = useI18n()
  const format = useFormat()
  const kind = useKindLabel()
  const batches = useSnapshot().recentImportBatches
  const [open, setOpen] = useState<ImportBatchOverview | null>(null)

  return (
    <SectionCard
      title={t('backup.import.batches.title')}
      subtitle={t('backupImport.batches.subtitle')}
    >
      {batches.length === 0 ? (
        <p className="text-[13px] text-muted-foreground">
          {t('backupImport.batches.empty')}
        </p>
      ) : (
        <ul className="-my-1.5 flex flex-col">
          {batches.map((batch) => {
            const undone = batch.status === 'reverted'
            return (
              <li
                key={batch.id}
                className="flex items-center gap-3 py-1.5 text-[13px]"
              >
                <div className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate" title={batch.sourcePath}>
                    {kind(batch)}
                    <span className="text-muted-foreground">
                      {' · '}
                      {baseName(batch.sourcePath)}
                    </span>
                  </span>
                  <span className="text-xs text-muted-foreground tabular">
                    {format.dayAndTime(batch.importedAt ?? batch.createdAt)}
                    {' · '}
                    {undone
                      ? t('backup.import.batches.undone')
                      : t('backup.import.batches.summary', {
                          imported: format.number(batch.importedItems),
                          duplicates: format.number(batch.duplicateItems),
                        })}
                  </span>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setOpen(batch)}
                >
                  {undone
                    ? t('backup.import.batches.restore')
                    : t('backup.import.batches.undo')}
                </Button>
              </li>
            )
          })}
        </ul>
      )}
      <BatchDialog batch={open} onClose={() => setOpen(null)} />
    </SectionCard>
  )
}

function BatchDialog({
  batch,
  onClose,
}: {
  batch: ImportBatchOverview | null
  onClose: () => void
}) {
  return (
    <Dialog open={batch !== null} onOpenChange={(next) => !next && onClose()}>
      {batch && <BatchReview batch={batch} onClose={onClose} />}
    </Dialog>
  )
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-0.5 rounded-lg bg-muted px-3 py-2">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="text-lg font-semibold tabular">{value}</span>
    </div>
  )
}

function BatchReview({
  batch,
  onClose,
}: {
  batch: ImportBatchOverview
  onClose: () => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const kind = useKindLabel()
  const undone = batch.status === 'reverted'
  const detail = useQuery({
    queryKey: [...queryKeys.archiveData, 'import-batch', batch.id],
    queryFn: () => importClient.previewBatch(batch.id),
    staleTime: 0,
  })
  const action = useMutation({
    mutationFn: async () => {
      if (undone) await importClient.restoreBatch(batch.id)
      else await importClient.revertBatch(batch.id)
      await refreshAfterArchiveChange()
    },
    onSuccess: () => {
      toast.success(
        undone
          ? t('backup.import.batches.restoreDone')
          : t('backup.import.batches.revertDone'),
      )
      onClose()
    },
  })
  const current: ImportBatchOverview = detail.data?.batch ?? batch

  return (
    <DialogContent
      className="sm:max-w-lg"
      showCloseButton={!action.isPending}
      onInteractOutside={(event) => action.isPending && event.preventDefault()}
      onEscapeKeyDown={(event) => action.isPending && event.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>
          {t('backupImport.batches.reviewTitle', {
            time: format.dayAndTime(batch.importedAt ?? batch.createdAt),
          })}
        </DialogTitle>
        <DialogDescription>
          {undone
            ? t('backupImport.batches.restoreNote', {
                count: current.importedItems,
              })
            : t('backupImport.batches.undoNote', {
                count: current.visibleItems,
              })}
        </DialogDescription>
      </DialogHeader>

      <div className="flex min-w-0 flex-col gap-4 text-[13px]">
        <div className="flex flex-col gap-1">
          <span className="text-xs text-muted-foreground">
            {t('backupImport.batches.source')} · {kind(current)}
          </span>
          <PathLine path={current.sourcePath} />
        </div>
        <div className="grid grid-cols-3 gap-2" data-testid="batch-counts">
          <Fact
            label={t('backupImport.batches.added')}
            value={format.number(current.importedItems)}
          />
          <Fact
            label={t('backupImport.batches.already')}
            value={format.number(current.duplicateItems)}
          />
          <Fact
            label={t('backupImport.batches.visible')}
            value={format.number(current.visibleItems)}
          />
        </div>
        {detail.isPending ? (
          <div className="flex flex-col gap-2">
            <Skeleton className="h-4 w-1/2" />
            <Skeleton className="h-16 w-full" />
          </div>
        ) : detail.isError ? (
          <p role="alert" className="text-destructive">
            {`${t('backupImport.batches.loadFailed')} ${describeError(detail.error, 'preview_import_batch')}`}
          </p>
        ) : (
          <BatchDetail detail={detail.data} />
        )}
        {action.error && (
          <p role="alert" className="[overflow-wrap:anywhere] text-destructive">
            {`${undone ? t('backup.import.batches.restoreFailed') : t('backup.import.batches.revertFailed')}: ${describeError(action.error, undone ? 'restore_import_batch' : 'revert_import_batch')}`}
          </p>
        )}
      </div>

      <DialogFooter>
        <Button variant="ghost" disabled={action.isPending} onClick={onClose}>
          {t('common.cancel')}
        </Button>
        <Button
          variant={undone ? 'default' : 'destructive'}
          disabled={action.isPending || !detail.isSuccess}
          onClick={() => action.mutate()}
        >
          {action.isPending && <Spinner />}
          {action.isPending
            ? t('backupImport.batches.working')
            : undone
              ? t('backupImport.batches.restoreConfirm')
              : t('backup.import.batches.undoConfirm')}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}

function BatchDetail({ detail }: { detail: ImportBatchDetail }) {
  const { t } = useI18n()
  const format = useFormat()
  const sample = detail.previewEntries.slice(0, 5)
  return (
    <>
      {detail.previewRangeStart && detail.previewRangeEnd && (
        <span className="text-muted-foreground">
          {t('backup.import.range', {
            start: format.date(detail.previewRangeStart),
            end: format.date(detail.previewRangeEnd),
          })}
        </span>
      )}
      <div className="flex flex-col gap-1">
        <span className="text-xs text-muted-foreground">
          {t('backupImport.batches.sample')}
        </span>
        {sample.length === 0 ? (
          <span className="text-muted-foreground">
            {t('backupImport.batches.noSample')}
          </span>
        ) : (
          <ul className="flex flex-col">
            {sample.map((entry) => (
              <li
                key={`${entry.sourceVisitId}-${entry.visitedAt}`}
                className="flex items-baseline gap-2 py-0.5"
              >
                <span className="min-w-0 flex-1 truncate" title={entry.url}>
                  {entry.title || entry.url}
                </span>
                <span className="shrink-0 font-mono text-xs text-muted-foreground tabular">
                  {format.dayAndTime(entry.visitedAt)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>
      {detail.batch.auditPath && (
        <div className="flex flex-col gap-0.5">
          <span className="text-xs text-muted-foreground">
            {t('backupImport.batches.record')}
          </span>
          <PathLine path={detail.batch.auditPath} />
        </div>
      )}
    </>
  )
}
