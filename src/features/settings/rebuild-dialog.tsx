/**
 * The confirmation before rebuilding the chosen model's search index from
 * nothing: how many visits it reads again, then the rebuild itself. Opened
 * from the Rebuild row and from the note on an index built from an older
 * version of the page text. The rebuild itself is `useRebuild`. Not
 * responsible for when a rebuild is offered.
 */
import { useQuery } from '@tanstack/react-query'
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
import { Skeleton } from '@/components/ui/skeleton'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { useI18n } from '@/lib/i18n'

export function RebuildDialog({
  open,
  onOpenChange,
  onConfirm,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onConfirm: () => void
}) {
  const { t } = useI18n()
  const estimate = useQuery({
    queryKey: ['ai', 'reembed-estimate', 'full'],
    queryFn: () => intelligenceClient.estimateReembed('full'),
    enabled: open,
    staleTime: 60_000,
  })

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {t('settingsAi.rebuild.dialogTitle')}
          </AlertDialogTitle>
          {estimate.isPending ? (
            <Skeleton className="h-10 w-full" />
          ) : (
            <AlertDialogDescription>
              {estimate.data && estimate.data.pageCount > 0
                ? t('settingsAi.rebuild.dialogBody', {
                    count: estimate.data.pageCount,
                  })
                : t('settingsAi.rebuild.dialogBodyUnknown')}
            </AlertDialogDescription>
          )}
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>
            {t('settingsAi.rebuild.confirm')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
