/**
 * 存在哪裡: today's step (src/features/onboarding/storage-step.tsx): where the
 * archive will be created, a button to show the folder, and the size tiles.
 * With only browsing chosen it is today's three tiles. With more types the
 * same three numbers become one row per type and a 合計.
 */
import { FolderOpen } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { bytes as formatBytes, num } from '~/data/format'
import { archiveBytes, sourceBytesOfKind } from '../backup/sources-data'
import type { StepKind } from './sources-data'
import { StepHeading } from './steps'

export const APP_ROOT = '~/Library/Application Support/PathKeep'

/** "78.0 MB" back to bytes: the 來源 step lists each profile's size as text. */
function parseAmount(amount: string) {
  const match = /([\d.]+)\s*(B|KB|MB|GB)/.exec(amount)
  if (!match) return 0
  return Number(match[1]) * { B: 1, KB: 1e3, MB: 1e6, GB: 1e9 }[match[2] as 'B' | 'KB' | 'MB' | 'GB']
}

/** The app's estimate (src/lib/onboarding-estimates.ts): archive ≈ 4/5 of the files, plus manifest and snapshots. */
function browsingEstimate(sourceBytes: number, profiles: number) {
  const manifest = profiles === 0 ? 0 : Math.max(256 * 1024, profiles * 64 * 1024)
  return Math.ceil((sourceBytes * 4) / 5) + manifest + sourceBytes
}

export function StorageStep({ list, selection, mac }: { list: StepKind[]; selection: Record<string, Set<string>>; mac: boolean }) {
  const chosen = (kind: StepKind) => kind.sources.filter((s) => selection[kind.id]?.has(s.id) && !s.blocked)
  const browsing = list.find((kind) => kind.id === 'browsing')
  const profiles = browsing ? chosen(browsing) : []
  const sourceBytes = profiles.reduce((sum, s) => sum + parseAmount(s.amount), 0)
  const estimate = browsingEstimate(sourceBytes, profiles.length)
  // Today's three numbers, one row per chosen type: what the sources hold now, about what the
  // first backup will take, and how many sources. Browsing from the app's estimate, Agent from its
  // sources, made-up types filler.
  const rows = list.map((kind) => {
    const n = chosen(kind).length
    const share = n / Math.max(1, kind.sources.length)
    const source = kind.id === 'browsing' ? sourceBytes : kind.id === 'agent' ? Math.round((sourceBytesOfKind.agent ?? 0) * share) : n * 52_000_000
    const after = kind.id === 'browsing' ? estimate : kind.id === 'agent' ? Math.round((archiveBytes.agent ?? 0) * share) : n * 41_000_000
    return { kind, source, after, count: n }
  })
  const total = { source: rows.reduce((sum, row) => sum + row.source, 0), after: rows.reduce((sum, row) => sum + row.after, 0), count: rows.reduce((sum, row) => sum + row.count, 0) }
  const onlyBrowsing = rows.length === 1 && rows[0].kind.id === 'browsing'
  const tiles = [
    { label: '瀏覽器歷史', value: formatBytes(sourceBytes) },
    { label: '預計大小', value: `≈ ${formatBytes(estimate)}` },
    { label: '瀏覽器', value: num(profiles.length) },
  ]

  return (
    <>
      <StepHeading title="存檔放在哪裡" body="存檔是一個本機資料庫，不會上傳到任何地方。" />
      <section className="flex flex-col gap-3.5 rounded-xl border bg-card p-[18px] shadow-card">
        <h2 className="text-[13px] font-medium">存檔位置</h2>
        <div className="flex gap-2">
          <div className="flex h-9 min-w-0 flex-1 items-center truncate rounded-lg border bg-popover px-3 font-mono text-[13px]" title={APP_ROOT}>
            <span className="truncate">{APP_ROOT}</span>
          </div>
          <Button variant="outline" className="h-9">
            <FolderOpen />
            {mac ? '在 Finder 中顯示' : '打開資料夾'}
          </Button>
        </div>
      </section>
      {onlyBrowsing ? (
        <dl className="grid grid-cols-3 gap-2.5">
          {tiles.map((tile) => (
            <div key={tile.label} className="flex flex-col gap-1 rounded-[10px] border bg-card p-3.5 shadow-card">
              <dt className="text-xs text-muted-foreground">{tile.label}</dt>
              <dd className="text-xl font-semibold tabular">{tile.value}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <section className="rounded-xl border bg-card p-[18px] shadow-card">
          <table className="w-full text-[13px]">
            <thead>
              <tr className="text-left text-xs text-muted-foreground">
                <th className="pb-2 font-normal">資料類型</th>
                <th className="pb-2 text-right font-normal">來源現在的大小</th>
                <th className="pb-2 text-right font-normal">第一次備份後大約</th>
                <th className="pb-2 text-right font-normal">來源</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(({ kind, source, after, count }) => (
                <tr key={kind.id} className="border-t">
                  <th scope="row" className="py-2 text-left font-normal">
                    <span className="flex items-center gap-2">
                      <kind.icon className="size-3.5 text-muted-foreground" aria-hidden />
                      {kind.label}
                    </span>
                  </th>
                  <td className="py-2 text-right font-mono tabular">{formatBytes(source)}</td>
                  <td className="py-2 text-right font-mono tabular">≈ {formatBytes(after)}</td>
                  <td className="py-2 text-right font-mono tabular">{num(count)}</td>
                </tr>
              ))}
              {rows.length > 1 && (
                <tr className="border-t font-medium">
                  <th scope="row" className="pt-2.5 text-left font-medium">
                    合計
                  </th>
                  <td className="pt-2.5 text-right font-mono tabular">{formatBytes(total.source)}</td>
                  <td className="pt-2.5 text-right font-mono tabular">≈ {formatBytes(total.after)}</td>
                  <td className="pt-2.5 text-right font-mono tabular">{num(total.count)}</td>
                </tr>
              )}
            </tbody>
          </table>
        </section>
      )}
      <p className="text-[13px] leading-normal text-muted-foreground">{onlyBrowsing ? '依你選的瀏覽器估算。之後每次備份，存檔會再大一點。' : '依你選的來源估算。之後每次備份，存檔會再大一點。'}</p>
    </>
  )
}
