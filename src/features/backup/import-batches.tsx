/**
 * Earlier imports with undo. Undoing hides the visits an import added; the
 * rows stay in the archive and can be restored, so the confirmation says so.
 */
import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { refreshAfterArchiveChange } from '@/app/backup-runner'
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { importClient } from '@/lib/backend-client/import'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { ImportBatchOverview } from '@/lib/types'

function useBatchAction(action: 'revert' | 'restore') {
  const { t } = useI18n()
  return useMutation({
    mutationFn: async (batchId: number) => {
      if (action === 'revert') await importClient.revertBatch(batchId)
      else await importClient.restoreBatch(batchId)
      await refreshAfterArchiveChange()
    },
    onSuccess: () => toast.success(t(`backup.import.batches.${action}Done`)),
    onError: (error) =>
      toast.error(t(`backup.import.batches.${action}Failed`), {
        description: describeError(error, `${action}_import_batch`),
      }),
  })
}

export function ImportBatches() {
  const { t } = useI18n()
  const format = useFormat()
  const batches = useSnapshot().recentImportBatches
  const [undoing, setUndoing] = useState<ImportBatchOverview | null>(null)
  const revert = useBatchAction('revert')
  const restore = useBatchAction('restore')
  if (batches.length === 0) return null

  return (
    <div className="flex flex-col gap-1 border-t pt-3.5">
      <h3 className="mb-1 text-[13px] font-medium">
        {t('backup.import.batches.title')}
      </h3>
      <ul className="flex flex-col">
        {batches.map((batch) => {
          const undone = batch.status === 'reverted'
          return (
            <li
              key={batch.id}
              className="flex items-center gap-3 py-1.5 text-[13px]"
            >
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="truncate">
                  {batch.sourceKind.includes('takeout')
                    ? t('backup.import.batches.takeout')
                    : t('backup.import.batches.browserFile')}
                  <span className="text-muted-foreground">
                    {' · '}
                    {format.dayAndTime(batch.importedAt ?? batch.createdAt)}
                  </span>
                </span>
                <span className="text-xs text-muted-foreground tabular">
                  {undone
                    ? t('backup.import.batches.undone')
                    : t('backup.import.batches.summary', {
                        imported: format.number(batch.importedItems),
                        duplicates: format.number(batch.duplicateItems),
                      })}
                </span>
              </div>
              {undone ? (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={restore.isPending}
                  onClick={() => restore.mutate(batch.id)}
                >
                  {t('backup.import.batches.restore')}
                </Button>
              ) : (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setUndoing(batch)}
                >
                  {t('backup.import.batches.undo')}
                </Button>
              )}
            </li>
          )
        })}
      </ul>
      <AlertDialog
        open={undoing !== null}
        onOpenChange={(open) => !open && setUndoing(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('backup.import.batches.undoTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('backup.import.batches.undoBody', {
                count: undoing?.visibleItems ?? 0,
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <Button
              disabled={revert.isPending}
              onClick={() =>
                undoing &&
                revert.mutate(undoing.id, { onSettled: () => setUndoing(null) })
              }
            >
              {t('backup.import.batches.undoConfirm')}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
