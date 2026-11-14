/** Backup's smaller cards: auto-delete notice, size by type, recent runs. */
import { CircleCheck, CircleMinus, ExternalLink, TriangleAlert } from 'lucide-react'
import { SectionCard } from '@/components/app/section-card'
import { cn } from '@/lib/cn'
import { NOW } from '~/data/fixture'
import { bytes, num, when } from '~/data/format'
import { kinds, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'
import { AGENT_ARCHIVE_RATIO, archiveBytes, harnessSources, sourceBytesOfKind } from './sources-data'

/** A tool setting that deletes old conversations. PathKeep tells, and never edits it. */
export function AutoDeleteNotice() {
  const { enabledKinds } = useProto()
  if (!enabledKinds.includes('agent')) return null
  return (
    <>
      {harnessSources
        .filter((h) => h.autoDelete)
        .map((h) => (
          <div key={h.id} className="flex items-start gap-3 rounded-xl border border-amber-500/25 bg-amber-500/8 px-5 py-4">
            <TriangleAlert className="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-300" />
            <div className="flex min-w-0 flex-1 flex-col gap-1 text-[13px]">
              <span className="font-medium">
                {h.name} 的 <code className="font-mono text-[12px]">{h.autoDelete!.setting}</code> 設成 {h.autoDelete!.days} 天：它會刪掉 {h.autoDelete!.days} 天前的對話
              </span>
              <span className="leading-relaxed text-muted-foreground">
                PathKeep 已經存下的不受影響，現在有 {num(h.gone)} 段對話只剩 PathKeep 有。想讓 {h.name} 自己也留久一點，可以改{' '}
                <code className="font-mono text-[12px]">{h.autoDelete!.file}</code>。PathKeep 不會去動它的設定。
              </span>
              <a href={h.autoDelete!.docs} target="_blank" rel="noreferrer" className="flex w-fit items-center gap-1 text-brand hover:underline">
                {h.name} 設定說明
                <ExternalLink className="size-3" />
              </a>
            </div>
          </div>
        ))}
    </>
  )
}

/** The 自動備份 card moved to Settings › 排程與背景工作; re-exported for pages that still import it from here. */
export { ScheduleCard } from '../schedule/schedule-card'

export function StorageCard() {
  const { enabledKinds } = useProto()
  const shown = kinds.filter((k) => k.real && (enabledKinds.includes(k.id) || archiveBytes[k.id]))
  const max = Math.max(...shown.map((k) => archiveBytes[k.id] ?? 0))
  const total = shown.reduce((s, k) => s + (archiveBytes[k.id] ?? 0), 0)
  return (
    <SectionCard title="存檔大小" subtitle={`共 ${bytes(total)}，依資料類型分`}>
      <div className="flex flex-col gap-3.5">
        {shown.map((k) => {
          const size = archiveBytes[k.id] ?? 0
          const source = sourceBytesOfKind[k.id] ?? 0
          return (
            <div key={k.id} className="flex flex-col gap-1.5">
              <div className="flex items-baseline justify-between gap-3 text-[13px]">
                <span className="flex items-center gap-1.5 font-medium">
                  <k.icon className="size-3.5 text-muted-foreground" />
                  {k.label}
                  {!enabledKinds.includes(k.id) && <span className="font-normal text-muted-foreground">（沒有開啟，資料保留）</span>}
                </span>
                <span className="font-mono tabular">{bytes(size)}</span>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                <span className="block h-full rounded-full bg-foreground/60" style={{ width: `${(size / max) * 100}%` }} />
              </div>
              <span className="text-xs text-muted-foreground">
                來源 {bytes(source)}
                {k.id === 'agent' && `：存檔目前約是來源的 ${AGENT_ARCHIVE_RATIO} 倍（原始紀錄一份、加密的搜尋索引一份）`}
              </span>
            </div>
          )
        })}
      </div>
    </SectionCard>
  )
}

export interface Run {
  at: number
  ms: number
  result: 'ok' | 'warn' | 'skipped'
  parts: Partial<Record<KindId, string>>
  warning?: string
}

export const runs: Run[] = [
  { at: NOW - 78 * 60_000, ms: 161_000, result: 'ok', parts: { browsing: '+1,203 筆造訪', agent: '+14 段對話，9 段有新內容，+212 個 subagent' } },
  { at: NOW - 4 * 3600_000 + 20 * 60_000, ms: 118_000, result: 'warn', parts: { browsing: '+862 筆造訪', agent: '+6 段對話，4 段有新內容' }, warning: 'Codex：2 個檔案正在寫入，下次再讀' },
  { at: NOW - 6 * 3600_000 + 20 * 60_000, ms: 97_000, result: 'ok', parts: { browsing: '+410 筆造訪', agent: '+3 段對話' } },
  { at: NOW - 8 * 3600_000 + 20 * 60_000, ms: 74_000, result: 'warn', parts: { browsing: '+318 筆造訪' }, warning: 'Chrome 個人 正在使用中，因此 PathKeep 讀取的是歷史檔案的副本，而不是正在使用的檔案。' },
  { at: NOW - 22 * 3600_000, ms: 204_000, result: 'ok', parts: { browsing: '+2,118 筆造訪', agent: '+21 段對話，12 段有新內容，+340 個 subagent' } },
  { at: NOW - 24 * 3600_000, ms: 88_000, result: 'ok', parts: { browsing: '+77 筆造訪', agent: '沒有新內容' } },
]

export function seconds(ms: number) {
  const s = Math.round(ms / 1000)
  return s >= 60 ? `${Math.floor(s / 60)} 分 ${s % 60} 秒` : `${s} 秒`
}

export function RunsCard() {
  const { enabledKinds } = useProto()
  return (
    <SectionCard title="最近的備份">
      <ul className="-mx-5 -mb-5">
        {runs.map((run) => {
          const Icon = run.result === 'ok' ? CircleCheck : run.result === 'warn' ? TriangleAlert : CircleMinus
          return (
            <li key={run.at} className="flex items-start gap-3 border-t px-5 py-3">
              <Icon
                className={cn(
                  'mt-0.5 size-4 shrink-0',
                  run.result === 'ok' && 'text-green',
                  run.result === 'warn' && 'text-amber-600 dark:text-amber-300',
                  run.result === 'skipped' && 'text-muted-foreground',
                )}
              />
              <div className="flex min-w-0 flex-1 flex-col gap-1">
                <span className="flex items-baseline gap-2 text-[13px]">
                  <span className="font-medium">{when(run.at)}</span>
                  {run.ms > 0 && <span className="text-xs text-muted-foreground">花了 {seconds(run.ms)}</span>}
                </span>
                {Object.keys(run.parts).length > 0 && (
                  <span className="flex flex-col gap-0.5 text-xs text-muted-foreground">
                    {kinds
                      .filter((k) => enabledKinds.includes(k.id) && run.parts[k.id])
                      .map((k) => (
                        <span key={k.id} className="flex items-center gap-1.5">
                          <k.icon className="size-3" />
                          <span className="text-foreground/80">{k.label}</span>
                          {run.parts[k.id]}
                        </span>
                      ))}
                  </span>
                )}
                {run.warning && <span className="text-xs text-muted-foreground">{run.warning}</span>}
              </div>
            </li>
          )
        })}
      </ul>
    </SectionCard>
  )
}
