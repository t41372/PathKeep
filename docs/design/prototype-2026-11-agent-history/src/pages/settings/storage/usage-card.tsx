/**
 * 磁碟用量 and, under it, 存檔大小. The card mirrors
 * `src/features/settings/usage-card.tsx`: one bar split into the archive,
 * search indexes, snapshots and everything else, with skeletons while the
 * numbers load.
 *
 * Design change: 存檔大小 is new. One line per data type; a type that is
 * off says 「（沒有開啟，資料保留）」.
 */
import { useEffect, useState } from 'react'
import { SettingRow } from '@/components/app/setting-row'
import { Skeleton } from '@/components/ui/skeleton'
import { kinds } from '~/kinds'
import { useProto } from '~/proto-state'
import { archiveBytes } from '../../backup/sources-data'
import { size } from '../pathkeep-kit/format'
import { archiveTotal, INDEX_BYTES, type BucketId } from './data'

type Part = 'archive' | 'indexes' | 'snapshots' | 'other'

const colors: Record<Part, string> = {
  archive: 'bg-brand',
  indexes: 'bg-blue',
  snapshots: 'bg-violet',
  other: 'bg-muted-foreground/40',
}

const partLabels: Record<Part, string> = { archive: '存檔', indexes: '搜尋索引', snapshots: '快照', other: '其他' }

export function UsageCard({ buckets }: { buckets: Record<BucketId, number> }) {
  const [loading, setLoading] = useState(true)
  useEffect(() => {
    const timer = setTimeout(() => setLoading(false), 500)
    return () => clearTimeout(timer)
  }, [])

  const parts: Record<Part, number> = {
    archive: archiveTotal,
    indexes: INDEX_BYTES,
    snapshots: buckets.snapshots,
    other: buckets.exports + buckets.staging + buckets.quarantine,
  }
  const total = Object.values(parts).reduce((sum, bytes) => sum + bytes, 0)

  return (
    <div className="flex flex-col gap-3 rounded-xl border bg-card p-[18px] shadow-card">
      <div className="flex justify-between">
        <span className="font-medium">磁碟用量</span>
        {loading ? <Skeleton className="h-4 w-16" /> : <span className="font-mono text-[13px]">{size(total)}</span>}
      </div>
      {loading ? (
        <>
          <Skeleton className="h-2.5 w-full rounded-[5px]" />
          <Skeleton className="h-3.5 w-3/4" />
        </>
      ) : (
        <>
          <div className="flex h-2.5 gap-0.5 overflow-hidden rounded-[5px] bg-muted" role="img" aria-label="磁碟用量">
            {(Object.keys(parts) as Part[]).map((part) => parts[part] > 0 && <span key={part} className={colors[part]} style={{ width: `${(parts[part] / total) * 100}%` }} />)}
          </div>
          <div className="flex flex-wrap gap-4 text-xs text-muted-foreground">
            {(Object.keys(parts) as Part[]).map((part) => (
              <span key={part} className="flex items-center gap-1.5">
                <span className={`size-2 rounded-[2px] ${colors[part]}`} />
                {partLabels[part]} {size(parts[part])}
              </span>
            ))}
          </div>
        </>
      )}
    </div>
  )
}

/** The archive's size by data type (design change). */
export function ArchiveSizeRow() {
  const { enabledKinds } = useProto()
  const shown = kinds.filter((k) => archiveBytes[k.id])
  return (
    <SettingRow title="存檔大小" description={`共 ${size(archiveTotal)}`}>
      <div className="flex flex-col gap-2.5">
        {shown.map((k) => (
          <div key={k.id} className="flex flex-col gap-1">
            <div className="flex justify-between text-[13px]">
              <span className="flex items-center gap-1.5">
                <k.icon className="size-3.5 text-muted-foreground" />
                {k.label}
                {!enabledKinds.includes(k.id) && <span className="text-muted-foreground">（沒有開啟，資料保留）</span>}
              </span>
              <span className="font-mono tabular">{size(archiveBytes[k.id]!)}</span>
            </div>
            <div className="h-1.5 overflow-hidden rounded-full bg-muted">
              <span className="block h-full rounded-full bg-foreground/60" style={{ width: `${(archiveBytes[k.id]! / archiveTotal) * 100}%` }} />
            </div>
          </div>
        ))}
      </div>
    </SettingRow>
  )
}
