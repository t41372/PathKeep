/**
 * Disk usage: one bar split into the archive, the search indexes, saved
 * snapshots and everything else, from the dashboard's storage summary.
 */
import { Skeleton } from '@/components/ui/skeleton'
import { useFormat, useI18n } from '@/lib/i18n'
import { useDashboard } from '@/lib/queries/app'
import type { StorageSummary } from '@/lib/types'

type Part = 'archive' | 'indexes' | 'snapshots' | 'other'

const colors: Record<Part, string> = {
  archive: 'bg-brand',
  indexes: 'bg-blue',
  snapshots: 'bg-violet',
  other: 'bg-muted-foreground/40',
}

function split(storage: StorageSummary): Record<Part, number> {
  return {
    archive:
      storage.archiveDatabaseBytes +
      storage.sourceEvidenceDatabaseBytes +
      storage.manifestBytes,
    indexes:
      storage.searchDatabaseBytes +
      storage.intelligenceDatabaseBytes +
      storage.semanticSidecarBytes +
      storage.intelligenceBlobBytes,
    snapshots: storage.snapshotBytes,
    other: storage.exportBytes + storage.stagingBytes + storage.quarantineBytes,
  }
}

export function UsageCard() {
  const { t } = useI18n()
  const format = useFormat()
  const dashboard = useDashboard()

  if (dashboard.isError) {
    return (
      <div className="rounded-xl border bg-card px-[18px] py-4 text-[13px] text-muted-foreground shadow-card">
        {t('settingsStorage.usage.failed')}
      </div>
    )
  }

  const parts = dashboard.data ? split(dashboard.data.storage) : null
  const total = parts
    ? Object.values(parts).reduce((sum, bytes) => sum + bytes, 0)
    : 0

  return (
    <div className="flex flex-col gap-3 rounded-xl border bg-card p-[18px] shadow-card">
      <div className="flex justify-between">
        <span className="font-medium">{t('settingsStorage.usage.title')}</span>
        {parts ? (
          <span className="font-mono text-[13px]">{format.bytes(total)}</span>
        ) : (
          <Skeleton className="h-4 w-16" />
        )}
      </div>
      {parts ? (
        <>
          <div
            className="flex h-2.5 gap-0.5 overflow-hidden rounded-[5px] bg-muted"
            role="img"
            aria-label={t('settingsStorage.usage.title')}
          >
            {(Object.keys(parts) as Part[]).map(
              (part) =>
                parts[part] > 0 && (
                  <span
                    key={part}
                    className={colors[part]}
                    style={{ width: `${(parts[part] / total) * 100}%` }}
                  />
                ),
            )}
          </div>
          <div className="flex flex-wrap gap-4 text-xs text-muted-foreground">
            {(Object.keys(parts) as Part[]).map((part) => (
              <span key={part} className="flex items-center gap-1.5">
                <span className={`size-2 rounded-[2px] ${colors[part]}`} />
                {t(`settingsStorage.usage.${part}`)} {format.bytes(parts[part])}
              </span>
            ))}
          </div>
        </>
      ) : (
        <>
          <Skeleton className="h-2.5 w-full rounded-[5px]" />
          <Skeleton className="h-3.5 w-3/4" />
        </>
      )}
    </div>
  )
}
