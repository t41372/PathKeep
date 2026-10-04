/**
 * "Clear Insights data", Preview → Manual → Execute: the dialog counts the
 * rows with `preview_clear_derived_intelligence` (the same counts the clear
 * reports), says what is kept, and only then runs
 * `clear_derived_intelligence`. Afterwards it offers to rebuild.
 *
 * Not responsible for deciding when clearing is allowed (the caller hides
 * the entry while a rebuild runs).
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
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
import { Spinner } from '@/components/ui/spinner'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import type { ClearDerivedIntelligenceReport } from '@/lib/types'
import { Notice } from '../notice'
import { runtimeKey, useQueueRebuild } from './use-background-work'

function rowsOf(report: ClearDerivedIntelligenceReport) {
  return [
    ['visits', report.clearedVisitDerivedFactRows],
    ['daily', report.clearedDailyRollupRows],
    ['structural', report.clearedStructuralRows],
    ['runtime', report.clearedRuntimeRows],
  ] as const
}

export function ClearDerivedDialog({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const client = useQueryClient()
  const rebuild = useQueueRebuild()
  // Counted fresh every time the dialog opens: the numbers are the decision.
  const preview = useQuery({
    queryKey: ['intelligence', 'clear-preview'],
    queryFn: intelligenceClient.previewClearDerivedState,
    staleTime: 0,
    gcTime: 0,
  })
  const clear = useMutation({
    mutationFn: intelligenceClient.clearDerivedState,
    onSuccess: () => {
      onClose()
      toast.success(t('settingsBackground.data.cleared'), {
        action: {
          label: t('settingsBackground.data.rebuildNow'),
          onClick: () => rebuild.mutate(),
        },
      })
      void client.invalidateQueries({ queryKey: runtimeKey })
      void client.invalidateQueries({ queryKey: queryKeys.archiveData })
    },
    onError: (error) =>
      toast.error(t('settingsBackground.data.clearFailed'), {
        description: describeError(error, 'clear_derived_intelligence'),
      }),
  })

  const rows = preview.data ? rowsOf(preview.data) : []
  const total = rows.reduce((sum, [, count]) => sum + count, 0)

  return (
    <AlertDialog
      open
      onOpenChange={(next) => !next && !clear.isPending && onClose()}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {t('settingsBackground.data.clearTitle')}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {t('settingsBackground.data.clearBody')}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <div className="flex flex-col gap-3 text-[13px]">
          {preview.isPending ? (
            <div className="flex flex-col gap-1.5">
              <span className="text-muted-foreground">
                {t('settingsBackground.data.clearCounting')}
              </span>
              <Skeleton className="h-16 w-full" />
            </div>
          ) : preview.isError ? (
            <p className="text-destructive">
              {t('settingsBackground.data.clearCountFailed', {
                message: describeError(
                  preview.error,
                  'preview_clear_derived_intelligence',
                ),
              })}
            </p>
          ) : total === 0 ? (
            <p>{t('settingsBackground.data.clearNothing')}</p>
          ) : (
            <div className="flex flex-col gap-1">
              <span className="font-medium">
                {t('settingsBackground.data.clearWill')}
              </span>
              <ul
                aria-label={t('settingsBackground.data.clearWill')}
                className="flex list-disc flex-col gap-0.5 pl-5"
              >
                {rows
                  .filter(([, count]) => count > 0)
                  .map(([key, count]) => (
                    <li key={key} className="tabular-nums">
                      {t(`settingsBackground.data.clearRows.${key}`, {
                        count,
                      })}
                    </li>
                  ))}
              </ul>
            </div>
          )}
          <Notice tone="info">{t('settingsBackground.data.clearKeeps')}</Notice>
        </div>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={clear.isPending}>
            {t('common.cancel')}
          </AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            disabled={!preview.data || total === 0 || clear.isPending}
            onClick={(event) => {
              event.preventDefault()
              clear.mutate()
            }}
          >
            {clear.isPending && <Spinner />}
            {t('settingsBackground.data.clearConfirm')}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
