/**
 * The notice beside search results while the search index hasn't caught up
 * with the archive (History, the command palette). Until then some pages can't
 * be found, so the results must not read as the whole answer: after an upgrade
 * or a reset the index is built from scratch (with its progress), and after a
 * write it lags a moment.
 */
import { Progress } from '@/components/ui/progress'
import type { IndexerState } from '@/lib/backend-client/search'
import { useFormat, useI18n } from '@/lib/i18n'
import { cn } from '@/lib/cn'

export interface CoverageStatus {
  coverageComplete: boolean
  indexer: IndexerState | null
}

export function SearchCoverageNotice({
  status,
  className,
}: {
  status: CoverageStatus | null
  className?: string
}) {
  const { t } = useI18n()
  const format = useFormat()
  if (!status || status.coverageComplete) return null
  const indexer = status.indexer
  const building =
    indexer !== null &&
    (indexer.phase === 'building' || indexer.phase === 'rebuilding') &&
    indexer.documentsTotal > 0

  return (
    <div role="status" className={cn('flex flex-col gap-1.5', className)}>
      <p className="text-[13px] text-muted-foreground">
        {indexer?.phase === 'failed'
          ? t('history.coverage.failed', { error: indexer.error ?? '' })
          : building
            ? t('history.coverage.building', {
                done: format.number(indexer.documentsProcessed),
                total: format.number(indexer.documentsTotal),
              })
            : t('history.coverage.updating')}
      </p>
      {building && (
        <Progress
          className="h-1 max-w-[320px]"
          value={Math.min(
            100,
            (indexer.documentsProcessed / indexer.documentsTotal) * 100,
          )}
          aria-label={t('history.coverage.progressLabel')}
        />
      )}
    </div>
  )
}
